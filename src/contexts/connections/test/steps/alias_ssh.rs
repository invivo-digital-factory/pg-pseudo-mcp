use cucumber::{given, then};

use crate::connections_steps::support::{
    assert_single_hop, params, test_bastion, write_ssh_config,
};
use crate::connections_world::ConnectionsWorld;

#[then(regex = r#"^la connexion "([^"]+)" passe par le bastion "([^"]+)" en tant que "([^"]+)"$"#)]
async fn postgres_bastion(
    world: &mut ConnectionsWorld,
    name: String,
    endpoint: String,
    user: String,
) {
    let tunnel = params(world, &name).tunnel.expect("a tunnel");
    assert_single_hop(&tunnel.hops, &endpoint, &user);
}

#[given(regex = r#"^un fichier de configuration SSH de test avec l'alias "([^"]+)"$"#)]
async fn ssh_config_with_alias(world: &mut ConnectionsWorld, alias: String) {
    write_ssh_config(world, &test_bastion(&alias, ""));
}

#[given(
    regex = r#"^un fichier de configuration SSH de test qui ne définit pas l'alias "([^"]+)"$"#
)]
async fn ssh_config_without_alias(world: &mut ConnectionsWorld, alias: String) {
    let contents = format!(
        "Host autre-bastion\n  HostName bastion.example.com\n\nHost *\n  User someone\n# {alias} is deliberately absent\n"
    );
    write_ssh_config(world, &contents);
}

#[given(
    regex = r#"^un fichier de configuration SSH qui inclut un fichier où est défini l'alias "([^"]+)"$"#
)]
async fn ssh_config_with_include(world: &mut ConnectionsWorld, alias: String) {
    let included = world.config_dir.path().join("config.d");
    std::fs::create_dir_all(&included).expect("include dir");
    std::fs::write(
        included.join("bastions.conf"),
        format!("Host {alias}\n  HostName bastion.example.com\n"),
    )
    .expect("included file");
    let contents = format!("Include {}/*.conf\n", included.display());
    write_ssh_config(world, &contents);
}

#[given(regex = r"^le client OpenSSH n'est pas installé$")]
async fn no_ssh_client(world: &mut ConnectionsWorld) {
    world.ssh_installed = false;
}
