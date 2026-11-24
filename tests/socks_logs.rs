use std::io;
use std::sync::{Arc, Mutex};

use fast_socks5::client::{Config, Socks5Stream};
use scarif_database_mcp::contexts::connections::domain::errors::SessionError;
use scarif_database_mcp::contexts::connections::infrastructure::data_sources::mongodb::CHANNELS_PER_MEMBER;
use scarif_database_mcp::contexts::connections::infrastructure::data_sources::socks::{
    AllowedTarget, ChannelOpener, SocksFront, TargetPolicy,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

const MEMBER: &str = "pl-0.atlas.scarif.test";
const REQUEST_LOG: &str = "Request target is";

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

struct EchoOpener;

impl ChannelOpener for EchoOpener {
    type Stream = DuplexStream;

    async fn open_channel(&self, _: &AllowedTarget) -> Result<DuplexStream, SessionError> {
        let (near, far) = tokio::io::duplex(1024);
        tokio::spawn(async move {
            let (mut reader, mut writer) = tokio::io::split(far);
            let _ = tokio::io::copy(&mut reader, &mut writer).await;
        });
        Ok(near)
    }
}

fn exposures(secret: &str) -> [String; 2] {
    [secret.to_string(), format!("{:?}", secret.as_bytes())]
}

#[tokio::test]
async fn les_identifiants_de_session_n_apparaissent_dans_aucun_journal_meme_en_trace() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(|| Capture)
        .with_ansi(false)
        .try_init()
        .expect("install the capturing subscriber and the log bridge");
    let policy = TargetPolicy::from_seed_host(MEMBER).expect("valid seed host");
    let front = SocksFront::start(Arc::new(EchoOpener), policy, CHANNELS_PER_MEMBER)
        .await
        .expect("start the SOCKS5 front");
    let credentials = front.credentials();
    let mut stream = Socks5Stream::connect_with_password(
        front.address(),
        MEMBER.to_string(),
        1024,
        credentials.username().to_string(),
        credentials.password().to_string(),
        Config::default(),
    )
    .await
    .expect("CONNECT through the front");
    stream.write_all(b"ping").await.expect("write through");
    let mut echoed = [0u8; 4];
    stream.read_exact(&mut echoed).await.expect("read the echo");
    assert_eq!(&echoed, b"ping");
    let logs = captured();
    assert!(
        logs.contains(REQUEST_LOG),
        "the debug records of fast-socks5 reach the capture"
    );
    for secret in [credentials.username(), credentials.password()] {
        for exposure in exposures(secret) {
            assert!(
                !logs.contains(&exposure),
                "a session credential reached the logs"
            );
        }
    }
}
