use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

struct McpProcess {
    child: std::process::Child,
    stdout: BufReader<std::process::ChildStdout>,
}

impl McpProcess {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pg-pseudo-mcp"))
            .env("RUST_LOG", "debug")
            .env("PG_PSEUDO_DBEAVER_HOME", env!("CARGO_MANIFEST_DIR"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("binary starts");
        let stdout = BufReader::new(child.stdout.take().expect("stdout piped"));
        Self { child, stdout }
    }

    fn send(&mut self, message: Value) {
        let stdin = self.child.stdin.as_mut().expect("stdin piped");
        writeln!(stdin, "{message}").expect("write request");
        stdin.flush().expect("flush request");
    }

    fn receive(&mut self, id: u64) -> Value {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            assert!(Instant::now() < deadline, "no response for request {id}");
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).expect("read stdout");
            assert!(read > 0, "server closed stdout before answering {id}");
            let message: Value = serde_json::from_str(&line)
                .unwrap_or_else(|_| panic!("stdout carried a non-protocol line: {line}"));
            if message["id"] == json!(id) {
                return message;
            }
        }
    }

    fn initialize(&mut self) {
        self.send(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "smoke-test", "version": "0.0.0" }
            }
        }));
        let response = self.receive(1);
        assert!(
            response["result"]["instructions"]
                .as_str()
                .unwrap_or_default()
                .contains("pseudonym")
        );
        self.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    }
}

impl Drop for McpProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

mod e2e {
    use super::*;

    #[test]
    fn le_serveur_expose_sept_outils_en_lecture_seule_sur_stdio() {
        let mut server = McpProcess::start();
        server.initialize();
        server.send(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }));
        let response = server.receive(2);
        let tools = response["result"]["tools"].as_array().expect("tools array");
        let mut names: Vec<&str> = tools
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "describe_table",
                "explain",
                "list_connections",
                "list_tables",
                "query",
                "search_schema",
                "server_info"
            ]
        );
        for tool in tools {
            assert_eq!(
                tool["annotations"]["readOnlyHint"],
                json!(true),
                "{}",
                tool["name"]
            );
            assert_eq!(
                tool["annotations"]["openWorldHint"],
                json!(false),
                "{}",
                tool["name"]
            );
        }
    }

    #[test]
    fn server_info_annonce_les_limites_du_serveur() {
        let mut server = McpProcess::start();
        server.initialize();
        server.send(json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": { "name": "server_info", "arguments": {} }
        }));
        let response = server.receive(3);
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .expect("text content");
        let info: Value = serde_json::from_str(text).expect("json body");
        assert_eq!(info["defaults"]["max_rows"], json!(50));
        assert_eq!(info["maxima"]["max_rows"], json!(500));
        assert_eq!(info["maxima"]["max_bytes"], json!(65536));
    }

    fn tools_by_name(result: &Value) -> std::collections::BTreeMap<String, Value> {
        result["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .map(|tool| {
                (
                    tool["name"].as_str().expect("tool name").to_owned(),
                    tool.clone(),
                )
            })
            .collect()
    }

    #[test]
    fn la_liste_des_outils_correspond_au_contrat_de_reference() {
        let expected: Value =
            serde_json::from_str(include_str!("fixtures/tools_list.json")).expect("fixture json");
        let mut server = McpProcess::start();
        server.initialize();
        server.send(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }));
        let live = server.receive(2)["result"].clone();
        let expected_tools = tools_by_name(&expected);
        let live_tools = tools_by_name(&live);
        let differing: std::collections::BTreeSet<&String> = expected_tools
            .keys()
            .chain(live_tools.keys())
            .filter(|name| expected_tools.get(*name) != live_tools.get(*name))
            .collect();
        assert!(
            differing.is_empty(),
            "tools differing from the reference: {differing:?}\nexpected: {}\nlive: {}",
            serde_json::to_string_pretty(&expected).unwrap(),
            serde_json::to_string_pretty(&live).unwrap()
        );
        assert_eq!(live, expected);
    }
}
