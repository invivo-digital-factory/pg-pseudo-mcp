use tracing::{Level, Metadata};
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, Layer};

const SILENCED_CRATES: [&str; 1] = ["fast_socks5"];
const WARNINGS_ONLY_PREFIX: &str = "hickory_";

pub fn log_layer<S, W>(filter: EnvFilter, writer: W) -> impl Layer<S>
where
    S: tracing::Subscriber + for<'span> LookupSpan<'span>,
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
{
    tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_ansi(false)
        .with_filter(filter)
        .with_filter(filter_fn(|metadata| !silenced(metadata)))
}

fn silenced(metadata: &Metadata<'_>) -> bool {
    let target = metadata.target();
    let crate_name = target.split("::").next().unwrap_or(target);
    SILENCED_CRATES.contains(&crate_name)
        || (crate_name.starts_with(WARNINGS_ONLY_PREFIX) && *metadata.level() > Level::WARN)
}
