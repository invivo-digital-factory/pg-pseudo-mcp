use std::io;
use std::sync::{Arc, Mutex};

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore};
use scarif_database_mcp::composition::logging::log_layer;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const MEMBER: &str = "pl-1.tls-canary.scarif.test";
const CANARY: &str = "tls-canary";

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

#[test]
fn le_nom_tls_d_un_membre_n_atteint_aucun_journal_meme_en_trace() {
    tracing_subscriber::registry()
        .with(log_layer(
            EnvFilter::new("trace,rustls=trace,rustls::client=trace,rustls::client::hs=trace"),
            || Capture,
        ))
        .try_init()
        .expect("install the capturing subscriber and the log bridge");
    let config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .expect("protocol versions")
            .with_root_certificates(RootCertStore::empty())
            .with_no_client_auth();
    let name = ServerName::try_from(MEMBER.to_string()).expect("member name");
    let mut connection = ClientConnection::new(Arc::new(config), name).expect("client hello");
    let mut hello = Vec::new();
    connection
        .write_tls(&mut hello)
        .expect("serialize the client hello");
    assert!(
        String::from_utf8_lossy(&hello).contains(MEMBER),
        "the handshake carries the member name as SNI"
    );

    tracing::trace!(target: "scarif_probe", "trace probe");
    tracing::warn!(target: "rustls::probe", "rustls warning probe");
    let logs = captured();
    assert!(
        logs.contains("trace probe"),
        "the capture records at trace level"
    );
    assert!(
        logs.contains("rustls warning probe"),
        "warnings of the TLS library still reach the logs"
    );
    assert!(!logs.contains(CANARY), "a member TLS name reached the logs");
}
