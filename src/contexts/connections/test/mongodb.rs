use crate::contexts::connections::domain::types::HeavyQueryLimits;
use std::path::Path;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use ::mongodb::Client;
use ::mongodb::bson::{Bson, Document, doc};
use ::mongodb::error::{ErrorKind, WriteFailure};
use ::mongodb::options::{ReadPreference, SelectionCriteria};
use regex::Regex;
use serde_json::json;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::RwLock;

use super::mongodb_replica::{MEMBER_HOST, require_gate};
use super::tunnel::{
    KnownHosts, SSH_HOST, SSH_PASSWORD, SSH_USER, listing_container_key, ssh_port,
};
use crate::contexts::connections::domain::constants::AGGREGATE_WRITE_STAGE_REFUSED;
use crate::contexts::connections::domain::errors::{
    ConnectionError, PasswordCommandError, SessionError,
};
use crate::contexts::connections::domain::repositories::ConnectionCatalog;
use crate::contexts::connections::domain::types::{
    AggregateRequest, CollectionKind, ConnectionEntry, ConnectionSource, ConnectionStatus,
    Discovery, DocumentNode, ExecutionLimits, FindRequest, Hop, HostKeyPolicy, MongoConnectParams,
    MongoReadPreference, MongoTransport, Secret, SshAuth, SshHop, Tunnel, TunnelAuth, UuidSubtype,
};
use crate::contexts::connections::infrastructure::data_sources::mongodb::{
    CHANNELS_PER_MEMBER, MongoSession, ProxyEndpoint, SELECTION_TIMEOUT, client_options,
};
use crate::contexts::connections::infrastructure::data_sources::password_command::{
    PasswordCommandRunner, ProcessPasswordCommand,
};
use crate::contexts::connections::infrastructure::data_sources::socks::{
    AllowedTarget, ChannelOpener, SocksFront, TargetPolicy,
};
use crate::contexts::connections::infrastructure::data_sources::ssh::{SshLink, TunnelStream};
use crate::contexts::connections::infrastructure::mappers::{
    AUTHENTICATION_REFUSED, PROXY_CHANNEL_REFUSED,
};
use crate::contexts::connections::infrastructure::repositories::{
    CachedCommandPasswords, MongoSessionPool,
};
use crate::contexts::connections::use_cases::{
    ListDocumentDatabases, ReadCollections, RunAggregate, RunFind, SampleFields,
};

const CONNECTION: &str = "atlas-test";
const READER: (&str, &str) = ("atlas-reader", "atlas-reader-test-pw");
const WRITER: (&str, &str) = ("atlas-writer", "atlas-writer-test-pw");
const ADMIN: (&str, &str) = ("atlas-admin", "atlas-admin-test-pw");
const MEMBER_PORTS: [u16; 3] = [1024, 1025, 1026];
const UNPERMITTED_PORT: u16 = 1027;
const UNAUTHORIZED: i32 = 13;

static PRIMARY: RwLock<()> = RwLock::const_new(());
const ELECTION_WAIT: Duration = Duration::from_secs(60);

struct FixedCatalog(MongoConnectParams);

impl ConnectionCatalog for FixedCatalog {
    fn discover_in(&self, _: Option<&str>) -> Discovery {
        Discovery {
            connections: vec![ConnectionEntry {
                id: CONNECTION.into(),
                name: CONNECTION.into(),
                declared_name: CONNECTION.into(),
                source: ConnectionSource::File,
                folder: None,
                project: "General".into(),
                engine: "mongodb".into(),
                connection_type: "test".into(),
                heavy_query: HeavyQueryLimits::default(),
                database: self.0.database.clone(),
                status: ConnectionStatus::Available,
                params: None,
                mongo: Some(self.0.clone()),
            }],
            hint: None,
        }
    }
}

struct CountingRunner {
    runner: ProcessPasswordCommand,
    runs: AtomicUsize,
}

impl PasswordCommandRunner for CountingRunner {
    fn run(&self, argv: &[String]) -> Result<Secret, PasswordCommandError> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        self.runner.run(argv)
    }
}

struct Harness {
    pool: Arc<MongoSessionPool>,
    runner: Arc<CountingRunner>,
    _known_hosts: KnownHosts,
}

fn bastion_hop(known_hosts: &Path) -> SshHop {
    SshHop {
        label: SSH_HOST.to_string(),
        host: SSH_HOST.to_string(),
        port: ssh_port(),
        user: SSH_USER.to_string(),
        auth: SshAuth::Password(Secret::new(SSH_PASSWORD)),
        host_key: HostKeyPolicy::Verify {
            known_hosts: vec![known_hosts.to_path_buf()],
            alias: None,
        },
    }
}

fn params((user, password): (&str, &str), port: u16, known_hosts: &Path) -> MongoConnectParams {
    MongoConnectParams {
        read_preference: MongoReadPreference::default(),
        transport: MongoTransport::Seed {
            host: MEMBER_HOST.to_string(),
            port,
            tls: false,
        },
        replica_set: Some("rs0".to_string()),
        database: Some("shop".to_string()),
        user: user.to_string(),
        password_command: vec!["/bin/echo".to_string(), password.to_string()],
        ssh_hops: Some(vec![bastion_hop(known_hosts)]),
    }
}

fn harness(account: (&str, &str), port: u16, selection_timeout: Duration) -> Harness {
    require_gate();
    let known_hosts = listing_container_key();
    let runner = Arc::new(CountingRunner {
        runner: ProcessPasswordCommand::new(),
        runs: AtomicUsize::new(0),
    });
    let pool = MongoSessionPool::new(
        Arc::new(FixedCatalog(params(account, port, &known_hosts.path))),
        None,
        Arc::new(CachedCommandPasswords::new(runner.clone())),
    )
    .with_selection_timeout(selection_timeout);
    Harness {
        pool: Arc::new(pool),
        runner,
        _known_hosts: known_hosts,
    }
}

fn reader() -> Harness {
    harness(READER, MEMBER_PORTS[0], SELECTION_TIMEOUT)
}

fn limits(max_rows: usize) -> ExecutionLimits {
    ExecutionLimits {
        max_rows,
        timeout_ms: 10_000,
    }
}

fn find(collection: &str, filter: serde_json::Value, limit: Option<u64>) -> FindRequest {
    FindRequest {
        database: None,
        collection: collection.to_string(),
        filter,
        projection: None,
        sort: Some(json!({"_id": 1})),
        skip: None,
        limit,
        hint: None,
        collation: None,
    }
}

fn field<'a>(node: &'a DocumentNode, key: &str) -> &'a DocumentNode {
    let DocumentNode::Object(fields) = node else {
        panic!("not an object: {node:?}");
    };
    &fields
        .iter()
        .find(|(name, _)| name == key)
        .unwrap_or_else(|| panic!("no field {key} in {node:?}"))
        .1
}

struct RecordingLink {
    link: SshLink,
    opened: Mutex<Vec<(String, u16)>>,
    open_now: Arc<AtomicUsize>,
    peak: AtomicUsize,
}

impl RecordingLink {
    fn over(link: SshLink) -> Self {
        Self {
            link,
            opened: Mutex::new(Vec::new()),
            open_now: Arc::new(AtomicUsize::new(0)),
            peak: AtomicUsize::new(0),
        }
    }

    fn opened(&self) -> Vec<(String, u16)> {
        self.opened.lock().expect("opened lock").clone()
    }

    fn peak(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }
}

struct CountedStream {
    inner: TunnelStream,
    open_now: Arc<AtomicUsize>,
}

impl Drop for CountedStream {
    fn drop(&mut self) {
        self.open_now.fetch_sub(1, Ordering::SeqCst);
    }
}

impl AsyncRead for CountedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(context, buffer)
    }
}

impl AsyncWrite for CountedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(context, buffer)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(context)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(context)
    }
}

impl ChannelOpener for RecordingLink {
    type Stream = CountedStream;

    async fn open_channel(&self, target: &AllowedTarget) -> Result<CountedStream, SessionError> {
        self.opened
            .lock()
            .expect("opened lock")
            .push((target.host().to_string(), target.port()));
        let inner = self.link.open_channel(target).await?;
        let now = self.open_now.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(now, Ordering::SeqCst);
        Ok(CountedStream {
            inner,
            open_now: self.open_now.clone(),
        })
    }
}

async fn bastion_link(known_hosts: &Path) -> SshLink {
    let tunnel = Tunnel {
        hops: vec![Hop {
            label: SSH_HOST.to_string(),
            host: SSH_HOST.to_string(),
            port: ssh_port(),
            user: SSH_USER.to_string(),
            auth: TunnelAuth::Password(SSH_PASSWORD.to_string()),
            verify_host_key: true,
            known_hosts: vec![known_hosts.to_path_buf()],
            host_key_alias: None,
        }],
        remote_host: MEMBER_HOST.to_string(),
        remote_port: MEMBER_PORTS[0],
    };
    SshLink::open(&tunnel).await.expect("open the SSH link")
}

async fn front_over<C: ChannelOpener>(opener: Arc<C>) -> SocksFront {
    let policy = TargetPolicy::from_seed_host(MEMBER_HOST).expect("valid seed host");
    SocksFront::start(opener, policy, CHANNELS_PER_MEMBER)
        .await
        .expect("start the SOCKS5 front")
}

async fn admin_client(known_hosts: &Path, front: &SocksFront) -> Client {
    let options = client_options(
        &params(ADMIN, MEMBER_PORTS[0], known_hosts),
        &Secret::new(ADMIN.1),
        Some(ProxyEndpoint {
            address: front.address(),
            credentials: front.credentials(),
        }),
        SELECTION_TIMEOUT,
    )
    .await
    .expect("admin options");
    Client::with_options(options).expect("admin client")
}

async fn primary(admin: &Client) -> Option<String> {
    let hello = admin
        .database("admin")
        .run_command(doc! {"hello": 1})
        .await
        .ok()?;
    hello.get_str("primary").ok().map(str::to_string)
}

async fn step_down(admin: &Client) -> String {
    let before = loop_until(async || primary(admin).await).await;
    let _ = admin
        .database("admin")
        .run_command(doc! {"replSetStepDown": 30, "secondaryCatchUpPeriodSecs": 10})
        .selection_criteria(SelectionCriteria::ReadPreference(ReadPreference::Primary))
        .await;
    let after = loop_until(async || primary(admin).await.filter(|now| *now != before)).await;
    assert_ne!(before, after);
    after
}

async fn loop_until<T>(probe: impl AsyncFn() -> Option<T>) -> T {
    let started = Instant::now();
    loop {
        if let Some(value) = probe().await {
            return value;
        }
        assert!(
            started.elapsed() < ELECTION_WAIT,
            "the replica set did not settle in time"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

mod integration {
    use super::*;

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and mongodb services"]
    async fn un_find_par_le_bastion_decouvre_les_trois_membres_et_survit_a_une_bascule_du_primaire()
    {
        let _election = PRIMARY.write().await;
        let harness = reader();
        let use_case = RunFind::new(harness.pool.clone());
        let first = use_case
            .execute(
                CONNECTION,
                &find("customers", json!({}), Some(1)),
                limits(10),
            )
            .await
            .expect("find through the pool");
        assert_eq!(first.documents.len(), 1);
        assert!(!first.more_documents);

        let known_hosts = listing_container_key();
        let recording = Arc::new(RecordingLink::over(bastion_link(&known_hosts.path).await));
        let front = front_over(recording.clone()).await;
        let options = client_options(
            &params(READER, MEMBER_PORTS[0], &known_hosts.path),
            &Secret::new(READER.1),
            Some(ProxyEndpoint {
                address: front.address(),
                credentials: front.credentials(),
            }),
            SELECTION_TIMEOUT,
        )
        .await
        .expect("reader options");
        let session = MongoSession::open(options).expect("reader client");
        let measured = session
            .find("shop", &find("customers", json!({}), Some(1)), limits(10))
            .await
            .expect("find through the recording front");
        assert_eq!(measured.documents.len(), 1);

        let admin_front = front_over(Arc::new(bastion_link(&known_hosts.path).await)).await;
        let admin = admin_client(&known_hosts.path, &admin_front).await;
        let new_primary = step_down(&admin).await;

        let after = use_case
            .execute(
                CONNECTION,
                &find("customers", json!({}), Some(1)),
                limits(10),
            )
            .await
            .expect("a secondaryPreferred find after the step-down");
        assert_eq!(after.documents.len(), 1);
        let measured_after = session
            .find("shop", &find("orders", json!({}), Some(2)), limits(10))
            .await
            .expect("find through the recording front after the step-down");
        assert_eq!(measured_after.documents.len(), 2);

        let opened = recording.opened();
        for port in MEMBER_PORTS {
            assert!(
                opened.contains(&(MEMBER_HOST.to_string(), port)),
                "member {port} discovered through the front: {opened:?}"
            );
        }
        println!(
            "SSH channels opened by one connection: {} in total, {} at most at once ({opened:?}); new primary {new_primary}",
            opened.len(),
            recording.peak()
        );
        assert!(
            recording.peak() <= MEMBER_PORTS.len() * CHANNELS_PER_MEMBER,
            "{} channels open at once: {opened:?}",
            recording.peak()
        );
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and mongodb services"]
    async fn un_mauvais_mot_de_passe_rend_un_motif_fixe_apres_exactement_deux_executions_de_la_commande()
     {
        let harness = harness(
            ("atlas-reader", "wrong-test-pw"),
            MEMBER_PORTS[0],
            SELECTION_TIMEOUT,
        );
        let error = RunFind::new(harness.pool.clone())
            .execute(
                CONNECTION,
                &find("customers", json!({}), Some(1)),
                limits(10),
            )
            .await
            .expect_err("the password is refused");
        assert_eq!(
            error,
            ConnectionError::ConnectFailed {
                connection: CONNECTION.to_string(),
                source: SessionError::Authentication(AUTHENTICATION_REFUSED.to_string()),
            }
        );
        assert_eq!(harness.runner.runs.load(Ordering::SeqCst), 2);
        let text = error.to_string();
        assert!(
            !text.contains("wrong-test-pw") && !text.contains("atlas-reader"),
            "{text}"
        );
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and mongodb services"]
    async fn une_erreur_de_selection_ne_rend_ni_le_membre_ni_l_utilisateur_ni_les_identifiants_du_proxy()
     {
        let harness = harness(READER, UNPERMITTED_PORT, Duration::from_secs(2));
        let error = RunFind::new(harness.pool.clone())
            .execute(
                CONNECTION,
                &find("customers", json!({}), Some(1)),
                limits(10),
            )
            .await
            .expect_err("no member is reachable");
        assert!(
            matches!(
                &error,
                ConnectionError::ConnectFailed {
                    source: SessionError::Connect(_),
                    ..
                }
            ),
            "{error:?}"
        );
        assert_eq!(
            error,
            ConnectionError::ConnectFailed {
                connection: CONNECTION.to_string(),
                source: SessionError::Connect(PROXY_CHANNEL_REFUSED.to_string()),
            }
        );
        let text = error.to_string();
        for secret in [MEMBER_HOST, "scarif.test", READER.0, SSH_USER] {
            assert!(!text.contains(secret), "{secret} in {text}");
        }
        let credential = Regex::new("[0-9a-f]{64}").expect("regex");
        assert!(!credential.is_match(&text), "{text}");
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and mongodb services"]
    async fn le_lecteur_ne_voit_que_ses_bases_autorisees_sans_les_bases_systeme() {
        let _primary = PRIMARY.read().await;
        let harness = reader();
        let databases = ListDocumentDatabases::new(harness.pool.clone())
            .execute(CONNECTION)
            .await
            .expect("listDatabases");
        assert_eq!(databases, vec!["shop".to_string()]);
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and mongodb services"]
    async fn les_types_bson_de_la_collection_events_arrivent_en_noeuds() {
        let harness = reader();
        let output = RunFind::new(harness.pool.clone())
            .execute(
                CONNECTION,
                &find("events", json!({"_id": 1}), None),
                limits(10),
            )
            .await
            .expect("events");
        let event = &output.documents[0];
        let expected = [
            ("_id", DocumentNode::Int(1)),
            ("kind", DocumentNode::Text("payment".into())),
            ("amount", DocumentNode::Decimal("12345.6700".into())),
            ("telephone", DocumentNode::Int(33_641_278_395)),
            ("quantity", DocumentNode::Int(2)),
            ("payload", DocumentNode::Binary { subtype: 0, len: 6 }),
            (
                "token",
                DocumentNode::Uuid {
                    subtype: UuidSubtype::Standard,
                    value: "0b1f2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d".into(),
                },
            ),
            (
                "pattern",
                DocumentNode::Opaque {
                    type_name: "regex".into(),
                    len: 10,
                },
            ),
            ("seenAt", DocumentNode::Timestamp("1767225600:1".into())),
            ("at", DocumentNode::Date("2026-03-01T09:30:00.000Z".into())),
            ("ratio", DocumentNode::Double(0.25)),
            ("flag", DocumentNode::Bool(true)),
            ("missing", DocumentNode::Null),
        ];
        for (key, node) in expected {
            assert_eq!(field(event, key), &node, "{key}");
        }
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and mongodb services"]
    async fn un_resultat_plus_long_que_la_limite_est_coupe_et_signale() {
        let harness = reader();
        let find_output = RunFind::new(harness.pool.clone())
            .execute(CONNECTION, &find("customers", json!({}), None), limits(2))
            .await
            .expect("find");
        assert_eq!(find_output.documents.len(), 2);
        assert!(find_output.more_documents);
        let aggregate_output = RunAggregate::new(harness.pool.clone())
            .execute(
                CONNECTION,
                &AggregateRequest {
                    database: Some("shop".into()),
                    collection: "orders".into(),
                    pipeline: vec![
                        json!({"$match": {"placedAt": {"$gte": {"$date": "2026-03-01T00:00:00Z"}}}}),
                        json!({"$sort": {"_id": 1}}),
                        json!({"$project": {"status": 1}}),
                    ],
                    let_vars: None,
                },
                limits(2),
            )
            .await
            .expect("aggregate");
        assert_eq!(aggregate_output.documents.len(), 2);
        assert!(aggregate_output.more_documents);
        assert_eq!(
            field(&aggregate_output.documents[0], "status"),
            &DocumentNode::Text("paid".into())
        );
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and mongodb services"]
    async fn les_collections_et_vues_sont_listees_avec_leur_genre_et_leur_compte_estime() {
        let _primary = PRIMARY.read().await;
        let harness = reader();
        let use_case = ReadCollections::new(harness.pool.clone());
        let collections = use_case
            .list(CONNECTION, Some("shop"))
            .await
            .expect("collections");
        let names: Vec<&str> = collections.iter().map(|info| info.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "attributes",
                "customer_contacts",
                "customers",
                "events",
                "orders",
                "products"
            ]
        );
        let view = &collections[1];
        assert_eq!(view.kind, CollectionKind::View);
        assert_eq!(view.estimated_count, None);
        assert_eq!(collections[2].kind, CollectionKind::Collection);
        assert_eq!(collections[2].estimated_count, Some(3));
        assert_eq!(collections[0].kind, CollectionKind::Collection);

        let customers = use_case
            .describe(CONNECTION, None, "customers")
            .await
            .expect("describe customers");
        assert_eq!(customers.estimated_count, Some(3));
        let keys: Vec<Vec<(String, String)>> = customers
            .indexes
            .into_iter()
            .map(|index| index.keys)
            .collect();
        assert_eq!(
            keys,
            vec![
                vec![("_id".to_string(), "1".to_string())],
                vec![("contact.email".to_string(), "1".to_string())],
            ]
        );
        let contacts = use_case
            .describe(CONNECTION, None, "customer_contacts")
            .await
            .expect("describe the view");
        assert!(contacts.indexes.is_empty());
        assert_eq!(contacts.estimated_count, None);
        assert_eq!(
            use_case.describe(CONNECTION, None, "ghosts").await,
            Err(ConnectionError::UnknownCollection {
                requested: "ghosts".into(),
            })
        );
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and mongodb services"]
    async fn l_echantillon_rend_les_chemins_types_et_presences_sans_aucune_valeur() {
        let harness = reader();
        let fields = SampleFields::new(harness.pool.clone())
            .execute(CONNECTION, None, "customers")
            .await
            .expect("sample");
        let sampled = |path: &str| {
            fields
                .iter()
                .find(|field| field.path.join(".") == path)
                .unwrap_or_else(|| panic!("{path} sampled"))
        };
        assert_eq!(sampled("contact.email").types, vec!["string".to_string()]);
        assert_eq!(sampled("contact.email").presence, 1.0);
        let birth = sampled("birthDate");
        assert!((birth.presence - 2.0 / 3.0).abs() < 1e-9, "{birth:?}");
        let dynamic_key = vec![
            "preferences".to_string(),
            "kervadec.ylwenna@canary-mail.fr".to_string(),
        ];
        assert!(
            fields.iter().any(|field| field.path == dynamic_key),
            "an object key is one raw segment, left to the document anonymizer"
        );
        let shown = format!("{fields:?}");
        for value in [
            "Ylwenna",
            "Kervadec",
            "ylwenna.kervadec@",
            "odrigan",
            "FR76",
            "Brizolier",
            "06 41 27",
            "33641278395",
        ] {
            assert!(!shown.contains(value), "{value} in {shown}");
        }
        assert_eq!(harness.runner.runs.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and mongodb services"]
    async fn le_lecteur_passe_par_le_bastion_et_toute_ecriture_est_refusee() {
        let harness = reader();
        let refused = RunAggregate::new(harness.pool.clone())
            .execute(
                CONNECTION,
                &AggregateRequest {
                    database: None,
                    collection: "orders".into(),
                    pipeline: vec![json!({"$out": "orders_copy"})],
                    let_vars: None,
                },
                limits(10),
            )
            .await;
        assert_eq!(
            refused,
            Err(ConnectionError::Unsupported {
                reason: AGGREGATE_WRITE_STAGE_REFUSED.to_string(),
            })
        );
        assert_eq!(harness.runner.runs.load(Ordering::SeqCst), 0);

        let known_hosts = listing_container_key();
        let front = front_over(Arc::new(bastion_link(&known_hosts.path).await)).await;
        let options = client_options(
            &params(READER, MEMBER_PORTS[0], &known_hosts.path),
            &Secret::new(READER.1),
            Some(ProxyEndpoint {
                address: front.address(),
                credentials: front.credentials(),
            }),
            SELECTION_TIMEOUT,
        )
        .await
        .expect("reader options");
        let reader = Client::with_options(options).expect("reader client");
        let orders = reader.database("shop").collection::<Document>("orders");
        assert!(orders_count(&reader).await > 0);
        let written = orders
            .insert_one(doc! {"_id": "scarif-write-probe", "status": "forged"})
            .await
            .expect_err("the reader may not write");
        assert!(
            matches!(
                written.kind.as_ref(),
                ErrorKind::Write(WriteFailure::WriteError(error)) if error.code == UNAUTHORIZED
            ) || matches!(
                written.kind.as_ref(),
                ErrorKind::Command(error) if error.code == UNAUTHORIZED
            ),
            "{written:?}"
        );
        assert_eq!(
            orders
                .count_documents(doc! {"_id": "scarif-write-probe"})
                .await
                .expect("count after the refused write"),
            0
        );
    }

    async fn orders_count(reader: &Client) -> u64 {
        reader
            .database("shop")
            .collection::<Document>("orders")
            .count_documents(doc! {})
            .await
            .expect("count orders")
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and mongodb services"]
    async fn le_redacteur_accepte_une_ecriture_mais_les_cas_d_usage_n_envoient_que_des_lectures() {
        let harness = harness(WRITER, MEMBER_PORTS[0], SELECTION_TIMEOUT);
        let output = RunFind::new(harness.pool.clone())
            .execute(CONNECTION, &find("products", json!({}), None), limits(10))
            .await
            .expect("the writer reads");
        assert_eq!(output.documents.len(), 2);
        let known_hosts = listing_container_key();
        let front = front_over(Arc::new(bastion_link(&known_hosts.path).await)).await;
        let options = client_options(
            &params(WRITER, MEMBER_PORTS[0], &known_hosts.path),
            &Secret::new(WRITER.1),
            Some(ProxyEndpoint {
                address: front.address(),
                credentials: front.credentials(),
            }),
            SELECTION_TIMEOUT,
        )
        .await
        .expect("writer options");
        let writer = Client::with_options(options).expect("writer client");
        let privileges = writer
            .database("admin")
            .run_command(doc! {"connectionStatus": 1, "showPrivileges": true})
            .await
            .expect("connectionStatus");
        let shown = Bson::Document(privileges)
            .into_relaxed_extjson()
            .to_string();
        assert!(
            shown.contains("\"insert\""),
            "the writer may write: {shown}"
        );
    }
}

const DRIVER_SOURCES: &str = "infrastructure/data_sources/mongodb";

const POOL_FILE: &str = "infrastructure/repositories/mongo_session_pool.rs";

const EXPLAIN_FILE: &str = "explain.rs";

const DRIVER_WRITES_AND_COMMANDS: &str = r"\.(?:run_command|run_cursor_command|run_raw_command|insert_one|insert_many|update_one|update_many|replace_one|delete_one|delete_many|find_one_and_\w+|bulk_write|drop|drop_index(?:es)?|create_collection|create_index(?:es)?|rename|watch|gridfs_bucket|start_session|count_documents|distinct|find_one|create_search_index(?:es)?|drop_search_index|update_search_index|warm_connection_pool|shutdown)\b";

#[test]
fn les_sources_mongodb_n_appellent_que_les_actions_de_lecture_du_pilote() {
    let context = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/contexts/connections");
    let forbidden = Regex::new(DRIVER_WRITES_AND_COMMANDS).expect("regex");
    let reads = Regex::new(r"\.(find|aggregate|list_database_names|list_collections|list_indexes|estimated_document_count)\(").expect("regex");
    let mut seen = Vec::new();
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(context.join(DRIVER_SOURCES))
        .expect("driver sources")
        .map(|entry| entry.expect("source entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect();
    assert!(files.len() >= 8, "{files:?}");
    files.push(context.join(POOL_FILE));
    for file in &files {
        let source = std::fs::read_to_string(file).expect("source file");
        let production = source.split("#[cfg(test)]").next().unwrap_or_default();
        let found: Vec<&str> = forbidden
            .find_iter(production)
            .map(|found| found.as_str())
            .collect();
        if file.ends_with(EXPLAIN_FILE) {
            assert_eq!(found, vec![".run_command"], "{}", file.display());
            assert!(
                production.contains(
                    r#".run_command(doc! {"explain": command, "verbosity": QUERY_PLANNER})"#
                ) && production.contains(r#"QUERY_PLANNER: &str = "queryPlanner""#),
                "{} may only run explain with the queryPlanner verbosity",
                file.display()
            );
            continue;
        }
        assert!(found.is_empty(), "{} calls {found:?}", file.display());
        seen.extend(
            reads
                .captures_iter(production)
                .map(|capture| capture[1].to_string()),
        );
    }
    seen.sort();
    seen.dedup();
    assert_eq!(
        seen,
        vec![
            "aggregate",
            "estimated_document_count",
            "find",
            "list_collections",
            "list_database_names",
            "list_indexes",
        ]
    );
}
