use crate::common::AppConfig;
use crate::composition::server_info::{
    CONFIG_LOCATION_DEPRECATED, DATA_DIRECTORY_CONFLICT, DATA_DIRECTORY_DEPRECATED, InfoWarning,
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
    notices.extend(data_directory_notices(config));
    notices
}

pub fn data_directory_notices(config: &AppConfig) -> Vec<InfoWarning> {
    let mut notices = Vec::new();
    let Ok(directory) = resolve_data_dir(config) else {
        return notices;
    };
    if directory.conflict {
        notices.push(InfoWarning {
            code: DATA_DIRECTORY_CONFLICT,
            message: format!(
                "two pseudonym data directories exist: {in_use} is in use because its secret decides the pseudonyms already seen, and {ignored} is ignored; to keep the known pseudonyms, stop the server, remove \"{ignored}\" then run: mv \"{in_use}\" \"{ignored}\"; to accept new pseudonyms instead, remove \"{in_use}\" (nothing is deleted, moved or merged automatically)",
                in_use = directory.path.display(),
                ignored = directory.preferred.display(),
            ),
        });
    } else if directory.legacy {
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
