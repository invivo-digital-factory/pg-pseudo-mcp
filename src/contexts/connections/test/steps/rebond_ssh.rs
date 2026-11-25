use cucumber::{given, then};

use crate::bdd_support::{
    BASTION_HOST, BASTION_USER, bastion_port, database_target, known_hosts_pattern, ssh_asset,
};
use crate::connections_steps::support::{
    STRICT_NO, bastion_key, params, reason, write_connection_file, write_ssh_config,
};
use crate::connections_world::ConnectionsWorld;

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

#[then(regex = r#"^la raison de la connexion "([^"]+)" ne contient pas "(.+)"$"#)]
async fn reason_does_not_contain(world: &mut ConnectionsWorld, name: String, forbidden: String) {
    let reason = reason(world, &name);
    assert!(!reason.contains(&forbidden), "{reason}");
}

#[then(regex = r#"^la connexion "([^"]+)" traverse (\d+) sauts SSH$"#)]
async fn hop_count(world: &mut ConnectionsWorld, name: String, count: usize) {
    let tunnel = params(world, &name).tunnel.expect("a tunnel");
    assert_eq!(tunnel.hops.len(), count);
}

#[then(regex = r#"^la connexion "([^"]+)" traverse les sauts SSH "([^"]+)"$"#)]
async fn hop_list(world: &mut ConnectionsWorld, name: String, hops: String) {
    let tunnel = params(world, &name).tunnel.expect("a tunnel");
    let actual: Vec<String> = tunnel
        .hops
        .iter()
        .map(|hop| format!("{}:{}", hop.host, hop.port))
        .collect();
    assert_eq!(actual.join(", "), hops);
}

#[given(regex = r"^un fichier known_hosts qui connaît la clé du bastion à ses deux sauts$")]
async fn known_hosts_both_hops(world: &mut ConnectionsWorld) {
    world.write_known_hosts(&format!(
        "{} {key}\n{BASTION_HOST} {key}\n",
        known_hosts_pattern(),
        key = bastion_key()
    ));
}

#[given(regex = r"^un fichier known_hosts qui ne connaît la clé du bastion qu'à son second saut$")]
async fn known_hosts_second_hop(world: &mut ConnectionsWorld) {
    world.write_known_hosts(&format!("{BASTION_HOST} {}\n", bastion_key()));
}

#[given(regex = r"^un fichier known_hosts qui ne connaît la clé du bastion qu'à son premier saut$")]
async fn known_hosts_first_hop(world: &mut ConnectionsWorld) {
    world.write_known_hosts(&format!("{} {}\n", known_hosts_pattern(), bastion_key()));
}
