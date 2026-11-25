use std::io;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Mutex;

use hickory_resolver::TokioResolver;
use hickory_resolver::config::{ConnectionConfig, NameServerConfig, ResolverConfig};
use hickory_resolver::net::runtime::TokioRuntimeProvider;
use hickory_resolver::proto::rr::RecordType;
use scarif_database_mcp::composition::logging::log_layer;
use tokio::net::UdpSocket;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const SRV_NAME: &str = "_mongodb._tcp.cluster0.dns-canary.scarif.test.";
const MEMBERS: [&str; 3] = [
    "pl-0.dns-canary.scarif.test",
    "pl-1.dns-canary.scarif.test",
    "pl-2.dns-canary.scarif.test",
];
const CANARY: &str = "dns-canary";
const HEADER_LEN: usize = 12;
const SRV_TYPE: u16 = 33;
const IN_CLASS: u16 = 1;
const QUESTION_POINTER: u16 = 0xC00C;

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

fn encoded_name(name: &str) -> Vec<u8> {
    let mut encoded = Vec::new();
    for label in name.split('.').filter(|label| !label.is_empty()) {
        encoded.push(u8::try_from(label.len()).expect("short label"));
        encoded.extend_from_slice(label.as_bytes());
    }
    encoded.push(0);
    encoded
}

fn question_end(query: &[u8]) -> usize {
    let mut at = HEADER_LEN;
    while query[at] != 0 {
        at += usize::from(query[at]) + 1;
    }
    at + 1 + 4
}

fn srv_answer(query: &[u8]) -> Vec<u8> {
    let end = question_end(query);
    let mut answer = query[..end].to_vec();
    answer[2] = 0x84 | (query[2] & 0x01);
    answer[3] = 0x00;
    let count = u16::try_from(MEMBERS.len()).expect("few members");
    answer[6..8].copy_from_slice(&count.to_be_bytes());
    answer[8..12].copy_from_slice(&[0, 0, 0, 0]);
    for (index, member) in MEMBERS.iter().enumerate() {
        let target = encoded_name(member);
        let port = 27017 + u16::try_from(index).expect("few members");
        let mut data = Vec::new();
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&port.to_be_bytes());
        data.extend_from_slice(&target);
        answer.extend_from_slice(&QUESTION_POINTER.to_be_bytes());
        answer.extend_from_slice(&SRV_TYPE.to_be_bytes());
        answer.extend_from_slice(&IN_CLASS.to_be_bytes());
        answer.extend_from_slice(&60u32.to_be_bytes());
        answer.extend_from_slice(&u16::try_from(data.len()).expect("short").to_be_bytes());
        answer.extend_from_slice(&data);
    }
    answer
}

async fn serve_srv(socket: UdpSocket) {
    let mut buffer = [0u8; 512];
    loop {
        let Ok((length, peer)) = socket.recv_from(&mut buffer).await else {
            return;
        };
        let _ = socket.send_to(&srv_answer(&buffer[..length]), peer).await;
    }
}

#[tokio::test]
async fn les_reponses_srv_et_les_noms_des_membres_n_atteignent_aucun_journal_meme_en_trace() {
    tracing_subscriber::registry()
        .with(log_layer(
            EnvFilter::new(
                "trace,hickory_resolver=trace,hickory_proto=trace,hickory_net=trace,hickory_net::udp=trace,hickory_resolver::name_server_pool=trace",
            ),
            || Capture,
        ))
        .try_init()
        .expect("install the capturing subscriber and the log bridge");
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind the fake DNS server");
    let port = socket.local_addr().expect("server address").port();
    tokio::spawn(serve_srv(socket));
    let mut udp = ConnectionConfig::udp();
    udp.port = port;
    let config = ResolverConfig::from_parts(
        None,
        Vec::new(),
        vec![NameServerConfig::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            true,
            vec![udp],
        )],
    );
    let resolver = TokioResolver::builder_with_config(config, TokioRuntimeProvider::default())
        .build()
        .expect("resolver");
    let lookup = resolver
        .lookup(SRV_NAME, RecordType::SRV)
        .await
        .expect("SRV answers from the fake server");
    assert_eq!(lookup.answers().len(), MEMBERS.len());

    tracing::trace!(target: "scarif_probe", "trace probe");
    tracing::warn!(target: "hickory_net::probe", "hickory warning probe");
    let logs = captured();
    assert!(
        logs.contains("trace probe"),
        "the capture records at trace level"
    );
    assert!(
        logs.contains("hickory warning probe"),
        "warnings of the resolver still reach the logs"
    );
    for member in MEMBERS {
        assert!(!logs.contains(member), "an SRV answer reached the logs");
    }
    assert!(!logs.contains(CANARY), "a DNS name reached the logs");
}
