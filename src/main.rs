use std::process::ExitCode;

use pg_pseudo_mcp::contexts::anonymization::injector::mapping_purge_handler;
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
    match mapping_purge_handler(&AppConfig::from_env()) {
        Ok(handler) => handler.run(confirmed),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
