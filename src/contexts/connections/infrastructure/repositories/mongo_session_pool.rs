use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::contexts::connections::domain::constants::{
    CONNECT_DEADLINE, MONGODB_DATABASE_REQUIRED,
};
use crate::contexts::connections::domain::errors::{ConnectionError, SessionError};
use crate::contexts::connections::domain::repositories::{
    CommandPasswordProvider, ConnectionCatalog, DocumentSessionPool,
};
use crate::contexts::connections::domain::services::{mongo_tunnel, resolve_mongo_connection};
use crate::contexts::connections::domain::types::{
    AggregateRequest, CollectionDescription, CollectionInfo, DatabaseNames, DocumentOutput,
    ExecutionLimits, FindRequest, MongoConnectParams, MongoTransport, SampledField, Secret,
};
use crate::contexts::connections::infrastructure::data_sources::mongodb::{
    MongoFailure, MongoSession, ProxyEndpoint, SELECTION_TIMEOUT, client_options,
};
use crate::contexts::connections::infrastructure::data_sources::socks::{SocksFront, TargetPolicy};
use crate::contexts::connections::infrastructure::data_sources::ssh::SshLink;
use crate::contexts::connections::infrastructure::mappers::{
    MongoRedaction, connection_error_from_mongo, is_authentication_failure,
    redacted_connect_failure,
};
use crate::contexts::connections::infrastructure::repositories::session_cache::{
    SessionCache, Slot,
};

const SESSION_KEY: &str = "";
const NO_OTHER_SESSIONS: usize = 0;

pub struct MongoSessionPool {
    catalog: Arc<dyn ConnectionCatalog>,
    known_hosts: Option<PathBuf>,
    commands: Arc<dyn CommandPasswordProvider>,
    sessions: Mutex<SessionCache<OpenMongo>>,
    connect_deadline: Duration,
    selection_timeout: Duration,
}

struct OpenMongo {
    session: MongoSession,
    front: Option<SocksFront>,
    link: Option<Arc<SshLink>>,
    params: MongoConnectParams,
    redaction: MongoRedaction,
    samples: HashMap<(String, String), Vec<SampledField>>,
}

struct Lease {
    guard: OwnedMutexGuard<Option<OpenMongo>>,
    slot: Slot<OpenMongo>,
    open: OpenMongo,
}

impl OpenMongo {
    fn reusable(&self, params: &MongoConnectParams) -> bool {
        self.params == *params && self.link.as_ref().is_none_or(|link| !link.is_closed())
    }

    fn database(&self, requested: Option<&str>) -> Result<String, MongoFailure> {
        requested
            .filter(|name| !name.trim().is_empty())
            .or(self.params.database.as_deref())
            .map(str::to_string)
            .ok_or_else(|| MongoFailure::InvalidRequest(MONGODB_DATABASE_REQUIRED.to_string()))
    }
}

impl MongoSessionPool {
    pub fn new(
        catalog: Arc<dyn ConnectionCatalog>,
        known_hosts: Option<PathBuf>,
        commands: Arc<dyn CommandPasswordProvider>,
    ) -> Self {
        Self {
            catalog,
            known_hosts,
            commands,
            sessions: Mutex::new(SessionCache::new(NO_OTHER_SESSIONS)),
            connect_deadline: CONNECT_DEADLINE,
            selection_timeout: SELECTION_TIMEOUT,
        }
    }

    pub fn with_selection_timeout(mut self, selection_timeout: Duration) -> Self {
        self.selection_timeout = selection_timeout;
        self
    }

    async fn run<R: MongoRead>(
        &self,
        connection: &str,
        read: R,
    ) -> Result<R::Output, ConnectionError> {
        let mut lease = self.lease(connection).await?;
        let mut outcome = read.read(&mut lease.open).await;
        if matches!(&outcome, Err(failure) if is_authentication_failure(failure)) {
            if let Err(error) = self.renew(connection, &mut lease.open).await {
                self.release(connection, lease, false).await;
                return Err(error);
            }
            outcome = read.read(&mut lease.open).await;
        }
        let result = outcome.map_err(|failure| {
            connection_error_from_mongo(connection, failure, &lease.open.redaction)
        });
        let keep = !matches!(result, Err(ConnectionError::ConnectFailed { .. }));
        self.release(connection, lease, keep).await;
        result
    }

    async fn lease(&self, connection: &str) -> Result<Lease, ConnectionError> {
        let catalog = self.catalog.clone();
        let discovery = tokio::task::spawn_blocking(move || catalog.discover())
            .await
            .map_err(|_| ConnectionError::discovery_interrupted())?;
        let params = resolve_mongo_connection(&discovery, connection)?;
        let slot = self
            .sessions
            .lock()
            .await
            .slot(connection, SESSION_KEY, true);
        let mut guard = slot.clone().lock_owned().await;
        let open = match guard.take() {
            Some(open) if open.reusable(&params) => open,
            _ => match self.open(connection, params).await {
                Ok(open) => open,
                Err(error) => {
                    drop(guard);
                    self.forget(connection, &slot).await;
                    return Err(error);
                }
            },
        };
        Ok(Lease { guard, slot, open })
    }

    async fn release(&self, connection: &str, lease: Lease, keep: bool) {
        let Lease {
            mut guard,
            slot,
            open,
        } = lease;
        if keep {
            *guard = Some(open);
            return;
        }
        drop(open);
        drop(guard);
        self.forget(connection, &slot).await;
    }

    async fn forget(&self, connection: &str, slot: &Slot<OpenMongo>) {
        self.sessions
            .lock()
            .await
            .forget(connection, SESSION_KEY, slot);
    }

    async fn open(
        &self,
        connection: &str,
        params: MongoConnectParams,
    ) -> Result<OpenMongo, ConnectionError> {
        let policy = target_policy(&params.transport)?;
        let unproxied = MongoRedaction::new(&params, Some(policy.clone()), None);
        let (link, front) = match mongo_tunnel(&params, self.known_hosts.as_deref()) {
            Some(tunnel) => {
                let link = tokio::time::timeout(self.connect_deadline, SshLink::open(&tunnel))
                    .await
                    .unwrap_or_else(|_| {
                        Err(SessionError::Connect(
                            "opening the SSH link timed out".to_string(),
                        ))
                    })
                    .map(Arc::new)
                    .map_err(|error| redacted_connect_failure(connection, error, &unproxied))?;
                let front = SocksFront::start(link.clone(), policy.clone())
                    .await
                    .map_err(|error| ConnectionError::ConnectFailed {
                        connection: connection.to_string(),
                        source: SessionError::Connect(format!(
                            "the local SOCKS5 front could not start: {error}"
                        )),
                    })?;
                (Some(link), Some(front))
            }
            None => (None, None),
        };
        let redaction = MongoRedaction::new(
            &params,
            Some(policy),
            front.as_ref().map(SocksFront::credentials),
        );
        let password = self
            .command_password(connection, &params.password_command, false)
            .await?;
        let session = self
            .session(connection, &params, &password, front.as_ref(), &redaction)
            .await?;
        Ok(OpenMongo {
            session,
            front,
            link,
            params,
            redaction,
            samples: HashMap::new(),
        })
    }

    async fn renew(&self, connection: &str, open: &mut OpenMongo) -> Result<(), ConnectionError> {
        let password = self
            .command_password(connection, &open.params.password_command, true)
            .await?;
        open.session = self
            .session(
                connection,
                &open.params,
                &password,
                open.front.as_ref(),
                &open.redaction,
            )
            .await?;
        open.samples.clear();
        Ok(())
    }

    async fn session(
        &self,
        connection: &str,
        params: &MongoConnectParams,
        password: &Secret,
        front: Option<&SocksFront>,
        redaction: &MongoRedaction,
    ) -> Result<MongoSession, ConnectionError> {
        let proxy = front.map(|front| ProxyEndpoint {
            address: front.address(),
            credentials: front.credentials(),
        });
        client_options(params, password, proxy, self.selection_timeout)
            .await
            .map_err(MongoFailure::from)
            .and_then(MongoSession::open)
            .map_err(|failure| connection_error_from_mongo(connection, failure, redaction))
    }

    async fn command_password(
        &self,
        connection: &str,
        argv: &[String],
        renew: bool,
    ) -> Result<Secret, ConnectionError> {
        let commands = self.commands.clone();
        let connection = connection.to_string();
        let argv = argv.to_vec();
        tokio::task::spawn_blocking(move || {
            if renew {
                commands.renewed_password(&connection, &argv)
            } else {
                commands.password(&connection, &argv)
            }
        })
        .await
        .map_err(|_| ConnectionError::discovery_interrupted())?
        .map_err(|error| ConnectionError::Unavailable {
            reason: error.to_string(),
        })
    }
}

fn target_policy(transport: &MongoTransport) -> Result<TargetPolicy, ConnectionError> {
    match transport {
        MongoTransport::Srv { host } => TargetPolicy::from_srv_name(host),
        MongoTransport::Seed { host, .. } => TargetPolicy::from_seed_host(host),
    }
    .map_err(|error| ConnectionError::Unavailable {
        reason: error.to_string(),
    })
}

trait MongoRead: Sync {
    type Output: Send;

    fn read<'a>(
        &'a self,
        open: &'a mut OpenMongo,
    ) -> impl Future<Output = Result<Self::Output, MongoFailure>> + Send + 'a;
}

struct FindRead<'r> {
    request: &'r FindRequest,
    limits: ExecutionLimits,
}

impl MongoRead for FindRead<'_> {
    type Output = DocumentOutput;

    async fn read(&self, open: &mut OpenMongo) -> Result<DocumentOutput, MongoFailure> {
        let database = open.database(self.request.database.as_deref())?;
        open.session
            .find(&database, self.request, self.limits)
            .await
    }
}

struct AggregateRead<'r> {
    request: &'r AggregateRequest,
    limits: ExecutionLimits,
}

impl MongoRead for AggregateRead<'_> {
    type Output = DocumentOutput;

    async fn read(&self, open: &mut OpenMongo) -> Result<DocumentOutput, MongoFailure> {
        let database = open.database(self.request.database.as_deref())?;
        open.session
            .aggregate(&database, self.request, self.limits)
            .await
    }
}

struct DatabaseNamesRead;

impl MongoRead for DatabaseNamesRead {
    type Output = DatabaseNames;

    async fn read(&self, open: &mut OpenMongo) -> Result<DatabaseNames, MongoFailure> {
        Ok(DatabaseNames {
            authorized: open.session.database_names().await?,
            default: open.params.database.clone(),
        })
    }
}

struct CollectionsRead<'r> {
    database: Option<&'r str>,
}

impl MongoRead for CollectionsRead<'_> {
    type Output = Vec<CollectionInfo>;

    async fn read(&self, open: &mut OpenMongo) -> Result<Vec<CollectionInfo>, MongoFailure> {
        let database = open.database(self.database)?;
        open.session.collections(&database).await
    }
}

struct DescribeRead<'r> {
    database: Option<&'r str>,
    collection: &'r str,
}

impl MongoRead for DescribeRead<'_> {
    type Output = CollectionDescription;

    async fn read(&self, open: &mut OpenMongo) -> Result<CollectionDescription, MongoFailure> {
        let database = open.database(self.database)?;
        open.session.describe(&database, self.collection).await
    }
}

struct SampleRead<'r> {
    database: Option<&'r str>,
    collection: &'r str,
}

impl MongoRead for SampleRead<'_> {
    type Output = Vec<SampledField>;

    async fn read(&self, open: &mut OpenMongo) -> Result<Vec<SampledField>, MongoFailure> {
        let key = (open.database(self.database)?, self.collection.to_string());
        if let Some(fields) = open.samples.get(&key) {
            return Ok(fields.clone());
        }
        let fields = open.session.sample(&key.0, &key.1).await?;
        open.samples.insert(key, fields.clone());
        Ok(fields)
    }
}

impl DocumentSessionPool for MongoSessionPool {
    async fn find(
        &self,
        connection: &str,
        request: &FindRequest,
        limits: ExecutionLimits,
    ) -> Result<DocumentOutput, ConnectionError> {
        self.run(connection, FindRead { request, limits }).await
    }

    async fn aggregate(
        &self,
        connection: &str,
        request: &AggregateRequest,
        limits: ExecutionLimits,
    ) -> Result<DocumentOutput, ConnectionError> {
        self.run(connection, AggregateRead { request, limits })
            .await
    }

    async fn database_names(&self, connection: &str) -> Result<DatabaseNames, ConnectionError> {
        self.run(connection, DatabaseNamesRead).await
    }

    async fn collections(
        &self,
        connection: &str,
        database: Option<&str>,
    ) -> Result<Vec<CollectionInfo>, ConnectionError> {
        self.run(connection, CollectionsRead { database }).await
    }

    async fn describe_collection(
        &self,
        connection: &str,
        database: Option<&str>,
        collection: &str,
    ) -> Result<CollectionDescription, ConnectionError> {
        self.run(
            connection,
            DescribeRead {
                database,
                collection,
            },
        )
        .await
    }

    async fn sample_fields(
        &self,
        connection: &str,
        database: Option<&str>,
        collection: &str,
    ) -> Result<Vec<SampledField>, ConnectionError> {
        self.run(
            connection,
            SampleRead {
                database,
                collection,
            },
        )
        .await
    }
}
