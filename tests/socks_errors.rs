use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use scarif_database_mcp::composition::logging::log_layer;
use scarif_database_mcp::contexts::connections::domain::errors::{ConnectionError, SessionError};
use scarif_database_mcp::contexts::connections::domain::types::{
    ExecutionLimits, FindRequest, MongoConnectParams, MongoTransport, Secret,
};
use scarif_database_mcp::contexts::connections::infrastructure::data_sources::mongodb::{
    MongoSession, ProxyEndpoint, client_options,
};
use scarif_database_mcp::contexts::connections::infrastructure::data_sources::socks::{
    AllowedTarget, ChannelOpener, ProxyCredentials, SocksFront, TargetPolicy,
};
use scarif_database_mcp::contexts::connections::infrastructure::mappers::{
    MongoRedaction, PROXY_CHANNEL_REFUSED, connection_error_from_mongo,
};
use tokio::io::DuplexStream;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const MEMBER: &str = "pl-0.atlas.scarif.test";
const MEMBER_PORT: u16 = 1024;
const CONNECTION: &str = "atlas";
const DB_USER: &str = "atlas-reader";
const DB_PASSWORD: &str = "atlas-reader-test-pw";

static CAPTURED: Mutex<Vec<u8>> = Mutex::new(Vec::new());

struct Capture;

impl io::Write for Capture {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        CAPTURED
            .lock()
            .expect("capture lock")
            .extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn captured() -> String {
    String::from_utf8_lossy(&CAPTURED.lock().expect("capture lock")).into_owned()
}

struct RefusingOpener;

impl ChannelOpener for RefusingOpener {
    type Stream = DuplexStream;

    async fn open_channel(&self, target: &AllowedTarget) -> Result<DuplexStream, SessionError> {
        Err(SessionError::Connect(format!(
            "the bastion refused {}:{}",
            target.host(),
            target.port()
        )))
    }
}

fn params() -> MongoConnectParams {
    MongoConnectParams {
        transport: MongoTransport::Seed {
            host: MEMBER.into(),
            port: MEMBER_PORT,
            tls: false,
        },
        replica_set: Some("rs0".into()),
        database: Some("shop".into()),
        user: DB_USER.into(),
        password_command: vec!["/bin/echo".into(), "pw".into()],
        ssh_hops: None,
    }
}

fn find() -> FindRequest {
    FindRequest {
        database: None,
        collection: "customers".into(),
        filter: serde_json::json!({}),
        projection: None,
        sort: None,
        skip: None,
        limit: Some(1),
        hint: None,
        collation: None,
    }
}

async fn refused_through(front: &SocksFront, sent: &ProxyCredentials) -> ConnectionError {
    let options = client_options(
        &params(),
        &Secret::new(DB_PASSWORD),
        Some(ProxyEndpoint {
            address: front.address(),
            credentials: sent,
        }),
        Duration::from_secs(1),
    )
    .await
    .expect("seed options need no DNS");
    let session = MongoSession::open(options).expect("client");
    let failure = match session
        .find(
            "shop",
            &find(),
            ExecutionLimits {
                max_rows: 1,
                timeout_ms: 1_000,
            },
        )
        .await
    {
        Ok(_) => panic!("no member is reachable"),
        Err(failure) => failure,
    };
    let unproxied = MongoRedaction::new(&params(), None, None);
    connection_error_from_mongo(CONNECTION, failure, &unproxied)
}

fn exposures(secret: &str) -> [String; 2] {
    [secret.to_string(), format!("{:?}", secret.as_bytes())]
}

#[tokio::test]
async fn un_refus_du_front_socks_rend_un_motif_fixe_sans_identifiants_ni_cible_meme_en_trace() {
    tracing_subscriber::registry()
        .with(log_layer(
            EnvFilter::new(
                "trace,fast_socks5=trace,fast_socks5::client=trace,fast_socks5::server=trace",
            ),
            || Capture,
        ))
        .try_init()
        .expect("install the capturing subscriber and the log bridge");
    let policy = TargetPolicy::from_seed_host(MEMBER).expect("valid seed host");
    let front = SocksFront::start(Arc::new(RefusingOpener), policy)
        .await
        .expect("start the SOCKS5 front");
    let foreign = ProxyCredentials::generate().expect("credentials");

    let rejected = refused_through(&front, &foreign).await;
    let channel_refused = refused_through(&front, front.credentials()).await;

    let expected = ConnectionError::ConnectFailed {
        connection: CONNECTION.into(),
        source: SessionError::Connect(PROXY_CHANNEL_REFUSED.into()),
    };
    assert_eq!(rejected, expected);
    assert_eq!(channel_refused, expected);
    tracing::trace!(target: "scarif_probe", "trace probe");
    let logs = captured();
    assert!(
        logs.contains("trace probe"),
        "the capture records at trace level"
    );
    let texts = [rejected.to_string(), channel_refused.to_string(), logs];
    let target = format!("{MEMBER}:{MEMBER_PORT}");
    let mut canaries = vec![MEMBER.to_string(), target];
    canaries.extend(exposures(DB_USER));
    canaries.extend(exposures(DB_PASSWORD));
    for credentials in [&foreign, front.credentials()] {
        for secret in [credentials.username(), credentials.password()] {
            canaries.extend(exposures(secret));
        }
    }
    for (place, text) in ["rejection", "channel refusal", "logs"].iter().zip(&texts) {
        for (index, canary) in canaries.iter().enumerate() {
            assert!(
                !text.contains(canary.as_str()),
                "canary {index} reached the {place}"
            );
        }
    }
}
