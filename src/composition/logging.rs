use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::Directive;

const SILENCED: [&str; 1] = ["fast_socks5=off"];

pub fn log_filter(base: EnvFilter) -> EnvFilter {
    SILENCED
        .iter()
        .filter_map(|directive| directive.parse::<Directive>().ok())
        .fold(base, EnvFilter::add_directive)
}
