use std::process::ExitCode;

use crate::common::AppConfig;
use crate::contexts::connections::injector::import_dbeaver_handler;

pub fn import_dbeaver(force: bool) -> ExitCode {
    import_dbeaver_handler(&AppConfig::from_env()).run(force)
}
