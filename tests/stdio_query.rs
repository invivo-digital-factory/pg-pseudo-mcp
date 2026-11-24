mod common;

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use common::{CONNECTION, assert_no_canary, write_workspace_for_test_database};

struct McpProcess {
    child: std::process::Child,
    stdout: BufReader<std::process::ChildStdout>,
    lines: Vec<String>,
    _dbeaver: tempfile::TempDir,
    _data: tempfile::TempDir,
}

impl McpProcess {
    fn start() -> Self {
        let dbeaver = tempfile::tempdir().expect("dbeaver dir");
        write_workspace_for_test_database(dbeaver.path());
        let data = tempfile::tempdir().expect("data dir");
        let mut child = Command::new(env!("CARGO_BIN_EXE_scarif-database-mcp"))
            .env("SCARIF_DBEAVER_HOME", dbeaver.path())
            .env("SCARIF_DATA_DIR", data.path())
            .env(
                "SCARIF_CONNECTIONS_FILE",
                data.path().join("absent-connections.toml"),
            )
            .env(
                "SCARIF_SSH_CONFIG",
                common::isolated_ssh_config(data.path()),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("binary starts");
        let stdout = BufReader::new(child.stdout.take().expect("stdout piped"));
        Self {
            child,
            stdout,
            lines: Vec::new(),
            _dbeaver: dbeaver,
            _data: data,
        }
    }

    fn send(&mut self, message: Value) {
        let stdin = self.child.stdin.as_mut().expect("stdin piped");
        writeln!(stdin, "{message}").expect("write request");
        stdin.flush().expect("flush request");
    }

    fn receive(&mut self, id: u64) -> Value {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            assert!(Instant::now() < deadline, "no response for request {id}");
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).expect("read stdout");
            assert!(read > 0, "server closed stdout before answering {id}");
            assert_no_canary(&line);
            let message: Value = serde_json::from_str(&line)
                .unwrap_or_else(|_| panic!("stdout carried a non-protocol line: {line}"));
            assert_eq!(message["jsonrpc"], json!("2.0"), "{line}");
            self.lines.push(line);
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
                "clientInfo": { "name": "stdio-query-test", "version": "0.0.0" }
            }
        }));
        self.receive(1);
        self.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    }

    fn call(&mut self, id: u64, tool: &str, arguments: Value) -> Value {
        self.send(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": tool, "arguments": arguments }
        }));
        self.receive(id)
    }
}

impl Drop for McpProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn body_of(response: &Value) -> Value {
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    serde_json::from_str(text).expect("json body")
}

mod e2e {
    use super::*;

    #[test]
    #[ignore = "needs the docker compose database"]
    fn une_requete_pseudonymisee_fait_l_aller_retour_sur_un_vrai_stdio() {
        let mut server = McpProcess::start();
        server.initialize();

        let connections = server.call(2, "list_connections", json!({}));
        let listed = body_of(&connections);
        let entry = listed["connections"]
            .as_array()
            .expect("connections array")
            .iter()
            .find(|entry| entry["name"] == json!(CONNECTION))
            .expect("test connection listed");
        assert_eq!(entry["status"], json!("available"));

        let customer = server.call(
            3,
            "query",
            json!({
                "connection": CONNECTION,
                "sql": "SELECT email FROM customers WHERE id = 1"
            }),
        );
        assert_ne!(customer["result"]["isError"], json!(true), "{customer}");
        let customer = body_of(&customer);
        assert_eq!(customer["row_count"], json!(1));
        assert!(
            customer["pseudonymized"].get("email").is_some(),
            "{customer}"
        );
        let email = customer["rows"][0][0].as_str().expect("email cell");
        assert!(email.contains('@'), "{email}");

        let orders = server.call(
            4,
            "query",
            json!({
                "connection": CONNECTION,
                "sql": format!(
                    "SELECT o.id FROM orders o JOIN customers c ON c.id = o.customer_id WHERE c.email = '{email}' ORDER BY o.id"
                )
            }),
        );
        assert_ne!(orders["result"]["isError"], json!(true), "{orders}");
        let orders = body_of(&orders);
        assert_eq!(orders["translated_literals"], json!(1), "{orders}");
        assert_eq!(orders["row_count"], json!(2), "{orders}");

        let refused = server.call(
            5,
            "query",
            json!({ "connection": CONNECTION, "sql": "DELETE FROM orders" }),
        );
        assert_eq!(refused["result"]["isError"], json!(true), "{refused}");
        let error = body_of(&refused);
        assert_eq!(error["error"]["category"], json!("refused_read_only"));

        assert_eq!(server.lines.len(), 5);
    }
}
