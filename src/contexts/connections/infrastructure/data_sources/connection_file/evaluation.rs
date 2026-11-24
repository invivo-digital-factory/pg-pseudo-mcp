use super::schema::RawEntry;
use crate::contexts::connections::domain::repositories::AzureSignInTokenProvider;
use crate::contexts::connections::domain::services::is_azure_postgres_host;
use crate::contexts::connections::domain::types::{
    AuthMode, ConnectParams, ConnectionDefinition, ConnectionEntry, ConnectionSource,
    ConnectionStatus, EngineSettings, MongoConnectParams, MongoSettings, PasswordSource, SslMode,
    TunnelParams,
};
use crate::contexts::connections::infrastructure::data_sources::openssh::OpenSshResolver;
use crate::contexts::connections::infrastructure::data_sources::password_resolution::{
    SignIn, pgpass_or_azure_token,
};
use crate::contexts::connections::infrastructure::data_sources::pgpass;

const MONGODB_AUTH_REASON: &str = "`auth` must be \"command\" when `engine` is \"mongodb\"";
const AZURE_HOST_REASON: &str = "auth = \"azure\" needs an Azure PostgreSQL host (ending in .postgres.database.azure.com): the Azure token is only sent to Azure PostgreSQL hosts";

struct Evaluation {
    connection_type: String,
    database: Option<String>,
    status: ConnectionStatus,
    params: Option<ConnectParams>,
    mongo: Option<MongoConnectParams>,
}

pub struct FileEvaluator<'a> {
    sign_in: SignIn<'a>,
    resolver: &'a OpenSshResolver,
    database: Option<&'a str>,
}

impl<'a> FileEvaluator<'a> {
    pub fn new(
        tokens: &'a dyn AzureSignInTokenProvider,
        resolver: &'a OpenSshResolver,
        database: Option<&'a str>,
    ) -> Self {
        Self {
            sign_in: SignIn::new(tokens),
            resolver,
            database,
        }
    }

    pub fn entry(&self, raw: &RawEntry) -> ConnectionEntry {
        let evaluation = match &raw.definition {
            Err(reason) => Evaluation {
                connection_type: "unknown".to_string(),
                database: None,
                status: ConnectionStatus::Unavailable {
                    reason: reason.clone(),
                },
                params: None,
                mongo: None,
            },
            Ok(definition) => self.evaluation(definition),
        };
        ConnectionEntry {
            id: raw.name.clone(),
            name: raw.name.clone(),
            declared_name: raw.name.clone(),
            source: ConnectionSource::File,
            folder: None,
            project: "file".to_string(),
            engine: raw.engine.to_string(),
            connection_type: evaluation.connection_type,
            database: evaluation.database,
            status: evaluation.status,
            params: evaluation.params,
            mongo: evaluation.mongo,
        }
    }

    fn evaluation(&self, definition: &ConnectionDefinition) -> Evaluation {
        let declared = match &definition.settings {
            EngineSettings::Postgresql { database, .. } => Some(database.clone()),
            EngineSettings::Mongodb(settings) => settings.database.clone(),
        };
        let database = self.database.map(str::to_string).or(declared);
        let outcome = match &definition.settings {
            EngineSettings::Postgresql {
                database: declared,
                sslmode,
            } => {
                let database = database.as_deref().unwrap_or(declared);
                self.params(definition, database, *sslmode)
                    .map(|params| (Some(params), None))
            }
            EngineSettings::Mongodb(settings) => self
                .mongo_params(definition, settings, database.clone())
                .map(|mongo| (None, Some(mongo))),
        };
        let (status, params, mongo) = match outcome {
            Ok((params, mongo)) => (ConnectionStatus::Available, params, mongo),
            Err(status) => (status, None, None),
        };
        Evaluation {
            connection_type: definition.connection_type.as_str().to_string(),
            database,
            status,
            params,
            mongo,
        }
    }

    fn mongo_params(
        &self,
        definition: &ConnectionDefinition,
        settings: &MongoSettings,
        database: Option<String>,
    ) -> Result<MongoConnectParams, ConnectionStatus> {
        let AuthMode::Command(argv) = &definition.auth else {
            return Err(ConnectionStatus::Unavailable {
                reason: MONGODB_AUTH_REASON.to_string(),
            });
        };
        let ssh_hops = definition
            .ssh_alias
            .as_deref()
            .map(|alias| self.resolver.resolve(alias))
            .transpose()?;
        Ok(MongoConnectParams {
            host: definition.host.clone(),
            port: definition.port,
            srv: settings.srv,
            tls: settings.tls,
            replica_set: settings.replica_set.clone(),
            database,
            user: definition.user.clone(),
            password: PasswordSource::Command(argv.clone()),
            ssh_hops,
        })
    }

    fn params(
        &self,
        definition: &ConnectionDefinition,
        database: &str,
        sslmode: SslMode,
    ) -> Result<ConnectParams, ConnectionStatus> {
        let target = pgpass::Target {
            host: &definition.host,
            port: definition.port,
            database,
            user: &definition.user,
        };
        let password = match &definition.auth {
            AuthMode::Pgpass => pgpass_or_azure_token(&target, false, &self.sign_in)?,
            AuthMode::Azure => {
                if !is_azure_postgres_host(&definition.host) {
                    return Err(ConnectionStatus::Unavailable {
                        reason: AZURE_HOST_REASON.to_string(),
                    });
                }
                pgpass_or_azure_token(&target, true, &self.sign_in)?
            }
            AuthMode::Command(argv) => PasswordSource::Command(argv.clone()),
        };
        let tunnel = definition
            .ssh_alias
            .as_deref()
            .map(|alias| {
                self.resolver.resolve(alias).map(|hops| TunnelParams {
                    hops,
                    remote_host: definition.host.clone(),
                    remote_port: definition.port,
                })
            })
            .transpose()?;
        let sslmode = password.effective_sslmode(sslmode);
        Ok(ConnectParams {
            host: definition.host.clone(),
            port: definition.port,
            database: database.to_string(),
            user: definition.user.clone(),
            password,
            sslmode,
            tunnel,
        })
    }
}
