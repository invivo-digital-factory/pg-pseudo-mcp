use std::process::ExitCode;

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
        ["--version"] => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: pg-pseudo-mcp [serve | --version]");
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
