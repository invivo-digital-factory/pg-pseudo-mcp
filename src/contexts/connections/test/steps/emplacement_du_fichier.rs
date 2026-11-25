use std::ffi::OsString;
use std::path::PathBuf;

use cucumber::{given, then, when};
use scarif_database_mcp::contexts::connections::infrastructure::data_sources::connection_file::{
    locate, locate_for_write,
};
use serde_json::json;

use crate::connections_world::ConnectionsWorld;

fn optional(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

fn environment(world: &ConnectionsWorld) -> (Option<PathBuf>, Option<OsString>, PathBuf) {
    let probe = world.probe.as_ref().expect("an environment");
    (
        probe["variable"].as_str().map(PathBuf::from),
        probe["xdg"].as_str().map(OsString::from),
        PathBuf::from(probe["home"].as_str().expect("a home directory")),
    )
}

#[given(
    regex = r#"^la variable SCARIF_CONNECTIONS_FILE "([^"]*)", la variable XDG_CONFIG_HOME "([^"]*)" et le dossier personnel "([^"]+)"$"#
)]
async fn given_environment(
    world: &mut ConnectionsWorld,
    variable: String,
    xdg: String,
    home: String,
) {
    world.probe = Some(json!({
        "variable": optional(&variable),
        "xdg": optional(&xdg),
        "home": home,
    }));
}

#[given(
    regex = r#"^un dossier personnel où existent les fichiers de connexions des dossiers "([^"]+)"$"#
)]
async fn home_with_files(world: &mut ConnectionsWorld, present: String) {
    let home = world.workspace.path().join("home");
    for directory in present.split(", ").filter(|name| *name != "aucun") {
        let dir = home.join(".config").join(directory);
        std::fs::create_dir_all(&dir).expect("config dir");
        std::fs::write(dir.join("connections.toml"), "").expect("connection file");
    }
    std::fs::create_dir_all(&home).expect("home dir");
    world.probe = Some(json!({ "variable": null, "xdg": null, "home": home }));
}

#[given(regex = r#"^la variable SCARIF_CONNECTIONS_FILE qui désigne "([^"]+)"$"#)]
async fn given_variable(world: &mut ConnectionsWorld, variable: String) {
    let probe = world.probe.as_mut().expect("an environment");
    probe["variable"] = json!(variable);
}

#[when(regex = r"^le serveur cherche son fichier de connexions$")]
async fn locate_file(world: &mut ConnectionsWorld) {
    let (variable, xdg, home) = environment(world);
    let location = locate(variable.as_deref(), xdg, Some(&home)).expect("a location");
    let probe = world.probe.as_mut().expect("an environment");
    probe["located"] = json!(location.path);
}

fn located(world: &ConnectionsWorld) -> PathBuf {
    PathBuf::from(
        world.probe.as_ref().expect("a search")["located"]
            .as_str()
            .expect("a located file"),
    )
}

#[then(regex = r#"^le fichier de connexions lu est "([^"]+)"$"#)]
async fn located_file_is(world: &mut ConnectionsWorld, expected: String) {
    assert_eq!(located(world), PathBuf::from(expected));
}

#[then(regex = r#"^le fichier de connexions lu est celui du dossier "([^"]+)"$"#)]
async fn located_directory_is(world: &mut ConnectionsWorld, directory: String) {
    let (_, _, home) = environment(world);
    assert_eq!(
        located(world),
        home.join(".config")
            .join(directory)
            .join("connections.toml")
    );
}

#[then(regex = r#"^un import écrirait le fichier de connexions dans le dossier "([^"]+)"$"#)]
async fn written_directory_is(world: &mut ConnectionsWorld, directory: String) {
    let (variable, xdg, home) = environment(world);
    let target = locate_for_write(variable.as_deref(), xdg, Some(&home)).expect("a target");
    assert_eq!(
        target,
        home.join(".config")
            .join(directory)
            .join("connections.toml")
    );
}
