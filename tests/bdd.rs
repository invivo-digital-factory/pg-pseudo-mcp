mod bdd_support;

#[path = "../src/contexts/anonymization/test/steps/mod.rs"]
mod anonymization_steps;
#[path = "../src/contexts/anonymization/test/steps/anonymization_world.rs"]
mod anonymization_world;
#[path = "../src/contexts/connections/test/steps/connections_world.rs"]
mod connections_world;
#[path = "../src/contexts/querying/test/steps/querying_world.rs"]
mod querying_world;

#[path = "../src/contexts/querying/test/steps/champs_derives_mongodb.rs"]
mod champs_derives_mongodb;
#[path = "../src/contexts/querying/test/steps/choix_de_la_base.rs"]
mod choix_de_la_base;
#[path = "../src/contexts/querying/test/steps/colonnes_derivees.rs"]
mod colonnes_derivees;
#[path = "../src/contexts/connections/test/steps/decouverte_des_connexions.rs"]
mod decouverte_des_connexions;
#[path = "../src/contexts/querying/test/steps/decouverte_du_schema.rs"]
mod decouverte_du_schema;
#[path = "../src/contexts/querying/test/steps/erreurs_des_outils.rs"]
mod erreurs_des_outils;
#[path = "../src/contexts/connections/test/steps/fichier_de_connexions.rs"]
mod fichier_de_connexions;
#[path = "../src/contexts/connections/test/steps/import_dbeaver.rs"]
mod import_dbeaver;
#[path = "../src/contexts/querying/test/steps/informations_du_serveur.rs"]
mod informations_du_serveur;
#[path = "../src/contexts/querying/test/steps/lecture_seule.rs"]
mod lecture_seule;
#[path = "../src/contexts/querying/test/steps/lecture_seule_mongodb.rs"]
mod lecture_seule_mongodb;
#[path = "../src/contexts/querying/test/steps/limites_des_reponses.rs"]
mod limites_des_reponses;
#[path = "../src/contexts/querying/test/steps/outils_mongodb.rs"]
mod outils_mongodb;
#[path = "../src/contexts/querying/test/steps/plan_d_execution.rs"]
mod plan_d_execution;
#[path = "../src/contexts/querying/test/steps/politique_de_champs_mongodb.rs"]
mod politique_de_champs_mongodb;
#[path = "../src/contexts/querying/test/steps/politique_de_colonnes.rs"]
mod politique_de_colonnes;
#[path = "../src/contexts/anonymization/test/steps/pseudonymisation.rs"]
mod pseudonymisation;
#[path = "../src/contexts/anonymization/test/steps/pseudonymisation_des_documents.rs"]
mod pseudonymisation_des_documents;
#[path = "../src/contexts/connections/test/steps/rebond_ssh.rs"]
mod rebond_ssh;
#[path = "../src/contexts/querying/test/steps/suivi_par_pseudonyme.rs"]
mod suivi_par_pseudonyme;
#[path = "../src/contexts/querying/test/steps/suivi_par_pseudonyme_mongodb.rs"]
mod suivi_par_pseudonyme_mongodb;
#[path = "../src/contexts/connections/test/steps/tunnel_ssh.rs"]
mod tunnel_ssh;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use cucumber::gherkin::{Feature, Rule, Scenario};
use cucumber::writer::Stats;
use cucumber::{World, cli};

use crate::anonymization_world::AnonymizationWorld;
use crate::bdd_support::DATABASE_URL_VAR;
use crate::connections_world::ConnectionsWorld;
use crate::querying_world::QueryingWorld;

const FEATURES: &str = "src/contexts";
const CONTEXTS: [&str; 3] = ["connections", "anonymization", "querying"];

fn feature_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()));
    for entry in entries {
        let path = entry.expect("cannot read directory entry").path();
        if path.is_dir() {
            feature_files(&path, found);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "feature")
        {
            found.push(path);
        }
    }
}

fn context_tags(path: &Path) -> BTreeSet<String> {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    content
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('@'))
        .flat_map(str::split_whitespace)
        .filter_map(|tag| tag.strip_prefix('@'))
        .filter(|tag| CONTEXTS.contains(tag))
        .map(str::to_owned)
        .collect()
}

fn orphan_features() -> Vec<String> {
    let root = Path::new(FEATURES);
    let mut files = Vec::new();
    feature_files(root, &mut files);
    files.sort();
    files
        .iter()
        .filter_map(|path| {
            let owner = path
                .strip_prefix(root)
                .ok()
                .and_then(|relative| relative.components().next())
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .unwrap_or_default();
            let tags = context_tags(path);
            let expected = BTreeSet::from([owner.clone()]);
            (tags != expected).then(|| {
                format!(
                    "{}: lives under context `{owner}` but carries context tags {:?}; expected exactly @{owner} (known contexts: {})",
                    path.display(),
                    tags,
                    CONTEXTS.join(", ")
                )
            })
        })
        .collect()
}

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
    let orphans = orphan_features();
    if !orphans.is_empty() {
        eprintln!("feature files that no World would run:");
        for orphan in &orphans {
            eprintln!("  {orphan}");
        }
        std::process::exit(1);
    }
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
