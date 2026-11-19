mod bdd_support;

#[path = "../src/contexts/anonymization/test/steps/anonymization_world.rs"]
mod anonymization_world;
#[path = "../src/contexts/connections/test/steps/connections_world.rs"]
mod connections_world;
#[path = "../src/contexts/querying/test/steps/querying_world.rs"]
mod querying_world;

#[path = "../src/contexts/querying/test/steps/colonnes_derivees.rs"]
mod colonnes_derivees;
#[path = "../src/contexts/connections/test/steps/decouverte_des_connexions.rs"]
mod decouverte_des_connexions;
#[path = "../src/contexts/querying/test/steps/lecture_seule.rs"]
mod lecture_seule;
#[path = "../src/contexts/anonymization/test/steps/pseudonymisation.rs"]
mod pseudonymisation;
#[path = "../src/contexts/querying/test/steps/suivi_par_pseudonyme.rs"]
mod suivi_par_pseudonyme;
#[path = "../src/contexts/connections/test/steps/tunnel_ssh.rs"]
mod tunnel_ssh;

use cucumber::gherkin::{Feature, Rule, Scenario};
use cucumber::writer::Stats;
use cucumber::{World, cli};

use crate::anonymization_world::AnonymizationWorld;
use crate::bdd_support::DATABASE_URL_VAR;
use crate::connections_world::ConnectionsWorld;
use crate::querying_world::QueryingWorld;

const FEATURES: &str = "src/contexts";

fn has_tag(tag: &str, feature: &Feature, rule: Option<&Rule>, scenario: &Scenario) -> bool {
    feature.tags.iter().any(|candidate| candidate == tag)
        || rule.is_some_and(|rule| rule.tags.iter().any(|candidate| candidate == tag))
        || scenario.tags.iter().any(|candidate| candidate == tag)
}

fn keeps(
    context: &'static str,
    integration_available: bool,
) -> impl Fn(&Feature, Option<&Rule>, &Scenario) -> bool {
    move |feature, rule, scenario| {
        has_tag(context, feature, rule, scenario)
            && (integration_available || !has_tag("integration", feature, rule, scenario))
    }
}

macro_rules! run_context {
    ($world:ty, $context:expr, $integration:expr) => {
        <$world>::cucumber()
            .with_cli(cli::Opts::<_, _, _, cli::Empty>::default())
            .fail_on_skipped()
            .max_concurrent_scenarios(1)
            .filter_run(FEATURES, keeps($context, $integration))
            .await
            .execution_has_failed()
    };
}

#[tokio::main]
async fn main() {
    let integration_available = std::env::var(DATABASE_URL_VAR).is_ok();
    if !integration_available {
        println!(
            "{DATABASE_URL_VAR} is not set: scenarios tagged @integration are skipped (they need the docker compose database and SSH bastion)"
        );
    }
    let mut failed = false;
    failed |= run_context!(ConnectionsWorld, "connections", integration_available);
    failed |= run_context!(AnonymizationWorld, "anonymization", integration_available);
    failed |= run_context!(QueryingWorld, "querying", integration_available);
    if failed {
        std::process::exit(1);
    }
}
