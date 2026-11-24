use std::process::ExitCode;

use super::notices::{data_directory_notices, print_notices};
use crate::common::AppConfig;
use crate::contexts::anonymization::injector::mapping_purge_handler;

pub fn mapping_purge(confirmed: bool) -> ExitCode {
    let config = AppConfig::from_env();
    print_notices(&data_directory_notices(&config));
    match mapping_purge_handler(&config) {
        Ok(handler) => handler.run(confirmed),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
