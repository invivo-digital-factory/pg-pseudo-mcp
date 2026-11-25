use cucumber::gherkin::Step;
use cucumber::{given, then};

use crate::bdd_support::{BASTION_HOST, BASTION_USER, bastion_port, database_target, ssh_asset};
use crate::connections_world::ConnectionsWorld;
use crate::fichier_de_connexions::write_connection_file;

const STRICT_NO: &str = "  StrictHostKeyChecking no\n";

pub fn write_ssh_config(world: &mut ConnectionsWorld, contents: &str) {
    let path = world.config_dir.path().join("ssh_config");
    std::fs::write(&path, contents).expect("ssh config");
    world.ssh_config = Some(path);
}

fn test_bastion(alias: &str, extra: &str) -> String {
    format!(
        "Host {alias}\n  HostName {BASTION_HOST}\n  Port {}\n  User {BASTION_USER}\n  IdentityFile {}\n  IdentitiesOnly yes\n  IdentityAgent none\n{extra}",
        bastion_port(),
        ssh_asset("tunnel_test_ed25519").display()
    )
}

#[given(regex = r#"^un fichier de configuration SSH de test avec l'alias "([^"]+)"$"#)]
async fn ssh_config_with_alias(world: &mut ConnectionsWorld, alias: String) {
    write_ssh_config(world, &test_bastion(&alias, ""));
}

#[given(
    regex = r#"^un fichier de configuration SSH de test avec l'alias "([^"]+)" en StrictHostKeyChecking no$"#
)]
async fn ssh_config_with_alias_strict_no(world: &mut ConnectionsWorld, alias: String) {
    write_ssh_config(world, &test_bastion(&alias, STRICT_NO));
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

#[given(regex = r"^un fichier de configuration SSH qui contient :$")]
async fn ssh_config_contents(world: &mut ConnectionsWorld, step: &Step) {
    write_ssh_config(world, step.docstring.as_deref().expect("a docstring"));
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

#[given(
    regex = r#"^un fichier de configuration SSH qui définit l'alias "([^"]+)" vers "([^"]+)"$"#
)]
async fn ssh_config_alias_to(world: &mut ConnectionsWorld, alias: String, host: String) {
    write_ssh_config(world, &format!("Host {alias}\n  HostName {host}\n"));
}

#[given(
    regex = r#"^un fichier de configuration SSH dont l'alias "([^"]+)" mène à "([^"]+)" sur le port (\d+) avec l'utilisateur "([^"]+)"$"#
)]
async fn ssh_config_alias_to_endpoint(
    world: &mut ConnectionsWorld,
    alias: String,
    host: String,
    port: u16,
    user: String,
) {
    let contents = format!("Host {alias}\n  HostName {host}\n  Port {port}\n  User {user}\n");
    write_ssh_config(world, &contents);
    std::fs::write(world.config_dir.path().join("ssh_config.before"), contents)
        .expect("ssh config copy");
}

#[then(regex = r"^le fichier de configuration SSH est inchangé$")]
async fn ssh_config_unchanged(world: &mut ConnectionsWorld) {
    let dir = world.config_dir.path();
    assert_eq!(
        std::fs::read_to_string(dir.join("ssh_config")).expect("ssh config"),
        std::fs::read_to_string(dir.join("ssh_config.before")).expect("ssh config copy")
    );
}

#[given(
    regex = r#"^un fichier de configuration SSH où l'alias "([^"]+)" atteint son bastion par (\d+) rebonds successifs$"#
)]
async fn ssh_config_with_chain(world: &mut ConnectionsWorld, alias: String, jumps: usize) {
    let mut contents = format!("Host {alias}\n  HostName bastion.example.com\n  ProxyJump hop1\n");
    for index in 1..=jumps {
        contents.push_str(&format!(
            "Host hop{index}\n  HostName hop{index}.example.com\n"
        ));
        if index < jumps {
            contents.push_str(&format!("  ProxyJump hop{}\n", index + 1));
        }
    }
    write_ssh_config(world, &contents);
}

#[given(regex = r"^le client OpenSSH n'est pas installé$")]
async fn no_ssh_client(world: &mut ConnectionsWorld) {
    world.ssh_installed = false;
}

fn two_hop_connection(world: &mut ConnectionsWorld, name: &str, alias: &str, strict: &str) {
    let common = format!(
        "  User {BASTION_USER}\n  IdentityFile {}\n  IdentitiesOnly yes\n  IdentityAgent none\n{strict}",
        ssh_asset("tunnel_test_ed25519").display()
    );
    let contents = format!(
        "Host jump-a\n  HostName {BASTION_HOST}\n  Port {}\n{common}\nHost {alias}\n  HostName {BASTION_HOST}\n  Port 22\n  ProxyJump jump-a\n{common}",
        bastion_port()
    );
    write_ssh_config(world, &contents);
    let target = database_target();
    write_connection_file(
        world,
        &format!(
            "[[connection]]\nname = \"{name}\"\nhost = \"postgres\"\ndatabase = \"{}\"\nuser = \"{}\"\nsslmode = \"disable\"\ntype = \"dev\"\nauth = \"command\"\npassword_command = [\"/bin/echo\", \"{}\"]\nssh_alias = \"{alias}\"\n",
            target.database, target.user, target.password
        ),
    );
}

#[given(
    regex = r#"^une connexion du fichier "([^"]+)" vers la base de test derrière l'alias "([^"]+)" qui rebondit par le bastion de test$"#
)]
async fn two_hop(world: &mut ConnectionsWorld, name: String, alias: String) {
    two_hop_connection(world, &name, &alias, "");
}

#[given(
    regex = r#"^une connexion du fichier "([^"]+)" vers la base de test derrière l'alias "([^"]+)" qui rebondit par le bastion de test en StrictHostKeyChecking no$"#
)]
async fn two_hop_strict_no(world: &mut ConnectionsWorld, name: String, alias: String) {
    two_hop_connection(world, &name, &alias, STRICT_NO);
}
