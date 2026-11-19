use std::io::{BufRead, Write};
use std::process::ExitCode;

use pg_pseudo_mcp::pseudo;
use pg_pseudo_mcp::state::AppConfig;
use pg_pseudo_mcp::tools::server::PgPseudoServer;
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] | ["serve"] => serve().await,
        ["mapping", "purge"] => mapping_purge(false),
        ["mapping", "purge", "--yes"] => mapping_purge(true),
        ["--version"] => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: pg-pseudo-mcp [serve | mapping purge [--yes] | --version]");
            ExitCode::from(2)
        }
    }
}

async fn serve() -> ExitCode {
    let server = PgPseudoServer::new(AppConfig::from_env());
    let running = match server.serve(stdio()).await {
        Ok(running) => running,
        Err(error) => {
            tracing::error!(%error, "failed to start the MCP server");
            return ExitCode::FAILURE;
        }
    };
    match running.waiting().await {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "MCP server stopped with an error");
            ExitCode::FAILURE
        }
    }
}

fn mapping_purge(confirmed: bool) -> ExitCode {
    let data_dir = match pseudo::data_dir(&AppConfig::from_env()) {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    if !confirmed {
        eprint!(
            "This deletes the pseudonym mapping and its secret in {}.\nPseudonyms already handed to agents will no longer translate back.\nType yes to confirm: ",
            data_dir.display()
        );
        let _ = std::io::stderr().flush();
        let mut answer = String::new();
        if std::io::stdin().lock().read_line(&mut answer).is_err() || answer.trim() != "yes" {
            eprintln!("Aborted, nothing deleted.");
            return ExitCode::FAILURE;
        }
    }
    match pseudo::purge(&data_dir) {
        Ok(deleted) if deleted.is_empty() => {
            eprintln!("Nothing to delete in {}.", data_dir.display());
            ExitCode::SUCCESS
        }
        Ok(deleted) => {
            for path in deleted {
                eprintln!("deleted {}", path.display());
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
