use tracing::Metadata;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, Layer};

const SILENCED_CRATES: [&str; 1] = ["fast_socks5"];

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
    SILENCED_CRATES.iter().any(|name| {
        target
            .strip_prefix(name)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with("::"))
    })
}
