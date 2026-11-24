use std::process::ExitCode;

use scarif_database_mcp::composition::connections_import::import_dbeaver;
use scarif_database_mcp::composition::logging::log_filter;
use scarif_database_mcp::composition::mapping_purge::mapping_purge;
use scarif_database_mcp::composition::server::serve;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(log_filter(EnvFilter::from_default_env()))
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
        ["connections", "import-dbeaver"] => import_dbeaver(false),
        ["connections", "import-dbeaver", "--force"] => import_dbeaver(true),
        ["--version"] => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!(
                "usage: scarif-database-mcp [serve | mapping purge [--yes] | connections import-dbeaver [--force] | --version]"
            );
            ExitCode::from(2)
        }
    }
}
