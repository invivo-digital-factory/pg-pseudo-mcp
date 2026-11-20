use cucumber::given;

use crate::bdd_support::{BASTION_HOST, BASTION_USER, bastion_port, ssh_asset};
use crate::connections_world::ConnectionsWorld;

fn write_ssh_config(world: &mut ConnectionsWorld, contents: &str) {
    let path = world.config_dir.path().join("ssh_config");
    std::fs::write(&path, contents).expect("ssh config");
    world.ssh_config = Some(path);
}

#[given(regex = r#"^un fichier de configuration SSH de test avec l'alias "([^"]+)"$"#)]
async fn ssh_config_with_alias(world: &mut ConnectionsWorld, alias: String) {
    let contents = format!(
        "Host {alias}\n  HostName {BASTION_HOST}\n  Port {}\n  User {BASTION_USER}\n  IdentityFile {}\n  IdentitiesOnly yes\n  IdentityAgent none\n",
        bastion_port(),
        ssh_asset("tunnel_test_ed25519").display()
    );
    write_ssh_config(world, &contents);
}

#[given(regex = r#"^un fichier de configuration SSH dont l'alias "([^"]+)" déclare ProxyCommand$"#)]
async fn ssh_config_with_proxy_command(world: &mut ConnectionsWorld, alias: String) {
    let contents =
        format!("Host {alias}\n  HostName bastion.internal.example\n  ProxyCommand nc %h %p\n");
    write_ssh_config(world, &contents);
}
