use std::process::ExitCode;

use super::notices::{deprecation_notices, print_notices};
use super::server_info::DATA_DIRECTORY_DEPRECATED;
use crate::common::AppConfig;
use crate::contexts::anonymization::injector::mapping_purge_handler;

pub fn mapping_purge(confirmed: bool) -> ExitCode {
    let config = AppConfig::from_env();
    let data_notices: Vec<_> = deprecation_notices(&config)
        .into_iter()
        .filter(|notice| notice.code == DATA_DIRECTORY_DEPRECATED)
        .collect();
    print_notices(&data_notices);
    match mapping_purge_handler(&config) {
        Ok(handler) => handler.run(confirmed),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
