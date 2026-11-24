use crate::common::AppConfig;
use crate::composition::server_info::{
    CONFIG_LOCATION_DEPRECATED, DATA_DIRECTORY_DEPRECATED, InfoWarning,
};
use crate::contexts::anonymization::infrastructure::data_sources::data_directory::resolve_data_dir;
use crate::contexts::connections::infrastructure::data_sources::connection_file::{
    resolve_user_config_file, user_config_file,
};

const CONFIG_FILES: [&str; 2] = ["connections.toml", "columns.toml"];

pub fn deprecation_notices(config: &AppConfig) -> Vec<InfoWarning> {
    let mut notices = Vec::new();
    for file in CONFIG_FILES {
        if file == "connections.toml" && config.connections_file.is_some() {
            continue;
        }
        let resolved = resolve_user_config_file(
            file,
            config.xdg_config_home.clone(),
            config.home_dir.as_deref(),
        );
        let preferred = user_config_file(
            file,
            config.xdg_config_home.clone(),
            config.home_dir.as_deref(),
        );
        if let (Some(resolved), Some(preferred)) = (resolved, preferred)
            && resolved.legacy
        {
            notices.push(InfoWarning {
                code: CONFIG_LOCATION_DEPRECATED,
                message: format!(
                    "{file} is read from the deprecated location {}; move it to {} (it is neither copied nor moved automatically)",
                    resolved.path.display(),
                    preferred.display()
                ),
            });
        }
    }
    if let Ok(directory) = resolve_data_dir(config)
        && directory.legacy
    {
        notices.push(InfoWarning {
            code: DATA_DIRECTORY_DEPRECATED,
            message: format!(
                "the pseudonym data directory {} is still in use; to migrate, stop the server and run: mv \"{}\" \"{}\" (it is neither copied nor moved automatically; the secret inside decides every pseudonym)",
                directory.path.display(),
                directory.path.display(),
                directory.preferred.display()
            ),
        });
    }
    notices
}

pub fn print_notices(notices: &[InfoWarning]) {
    for notice in notices {
        eprintln!("warning: {}: {}", notice.code, notice.message);
    }
}
