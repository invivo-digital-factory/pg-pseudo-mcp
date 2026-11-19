use std::process::ExitCode;

use crate::common::AppConfig;
use crate::contexts::anonymization::injector::mapping_purge_handler;

pub fn mapping_purge(confirmed: bool) -> ExitCode {
    match mapping_purge_handler(&AppConfig::from_env()) {
        Ok(handler) => handler.run(confirmed),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
