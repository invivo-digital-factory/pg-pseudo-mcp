use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

const FORBIDDEN_IN_DOMAIN: &str = r"\b(?:rmcp|tokio_postgres|postgres_types|rusqlite|russh)::";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Layer {
    Domain,
    UseCase,
    Injector,
    Infrastructure,
    Presentation,
    Root,
}

struct SourceFile {
    relative: String,
    module: Vec<String>,
    owner: Owner,
    text: String,
}

enum Owner {
    Context { name: String, layer: Layer },
    Common,
}

fn rust_files(directory: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = fs::read_dir(directory)
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default();
    entries.sort();
    entries
        .into_iter()
        .flat_map(|path| {
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "test") {
                    Vec::new()
                } else {
                    rust_files(&path)
                }
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                vec![path]
            } else {
                Vec::new()
            }
        })
        .collect()
}

fn module_of(root: &Path, file: &Path) -> Vec<String> {
    let relative = file.strip_prefix(root).expect("file under root");
    let mut segments: Vec<String> = relative
        .iter()
        .map(|part| part.to_string_lossy().into_owned())
        .collect();
    let last = segments.pop().expect("file name");
    let stem = last.trim_end_matches(".rs");
    if stem != "mod" {
        segments.push(stem.to_string());
    }
    segments
}

fn layer_of(segments: &[String]) -> Layer {
    match segments.get(2).map(String::as_str) {
        Some("domain") => Layer::Domain,
        Some("use_cases") => Layer::UseCase,
        Some("injector") => Layer::Injector,
        Some("infrastructure") => Layer::Infrastructure,
        Some("presentation") => Layer::Presentation,
        _ => Layer::Root,
    }
}

fn sources(root: &Path) -> Vec<SourceFile> {
    let contexts = root.join("contexts");
    let mut contexts_dirs: Vec<PathBuf> = fs::read_dir(&contexts)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect()
        })
        .unwrap_or_default();
    contexts_dirs.sort();
    let context_files = contexts_dirs
        .iter()
        .flat_map(|directory| rust_files(directory));
    let common_files = rust_files(&root.join("common"));
    context_files
        .chain(common_files)
        .map(|file| {
            let module = module_of(root, &file);
            let owner = if module.first().is_some_and(|first| first == "common") {
                Owner::Common
            } else {
                Owner::Context {
                    name: module[1].clone(),
                    layer: layer_of_file(&module, &file),
                }
            };
            SourceFile {
                relative: file
                    .strip_prefix(root)
                    .expect("file under root")
                    .to_string_lossy()
                    .into_owned(),
                module,
                owner,
                text: fs::read_to_string(&file).expect("readable source"),
            }
        })
        .collect()
}

fn layer_of_file(module: &[String], file: &Path) -> Layer {
    let layer = layer_of(module);
    if layer == Layer::Root && file.file_stem().is_some_and(|stem| stem == "injector") {
        return Layer::Injector;
    }
    layer
}

fn without_test_code(text: &str) -> String {
    let comments = Regex::new(r"//[^\n]*").expect("valid regex");
    let mut code = comments.replace_all(text, "").into_owned();
    while let Some(start) = code.find("#[cfg(test)]") {
        let rest = &code[start..];
        let end = match rest.find(['{', ';']) {
            Some(position) if rest.as_bytes()[position] == b';' => start + position + 1,
            Some(position) => matching_brace(&code, start + position) + 1,
            None => code.len(),
        };
        code.replace_range(start..end.min(code.len()), "");
    }
    code
}

fn matching_brace(text: &str, open: usize) -> usize {
    let mut depth = 0usize;
    for (offset, character) in text[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return open + offset;
                }
            }
            _ => {}
        }
    }
    text.len().saturating_sub(1)
}

fn split_top_level(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (index, character) in text.char_indices() {
        match character {
            '{' => depth += 1,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&text[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

fn flatten(tree: &str) -> Vec<Vec<String>> {
    let tree = tree.trim();
    if tree.is_empty() {
        return Vec::new();
    }
    let alias = Regex::new(r"\s+as\s+\w+$").expect("valid regex");
    match tree.find('{') {
        None => {
            let bare = alias.replace(tree, "");
            let segments: Vec<String> = bare.split("::").map(str::to_string).collect();
            vec![segments]
        }
        Some(open) => {
            let prefix: Vec<String> = tree[..open]
                .trim_end_matches("::")
                .split("::")
                .filter(|segment| !segment.is_empty())
                .map(str::to_string)
                .collect();
            let close = matching_brace(tree, open);
            split_top_level(&tree[open + 1..close])
                .into_iter()
                .flat_map(flatten)
                .map(|mut path| {
                    if path.last().is_some_and(|last| last == "self") {
                        path.pop();
                    }
                    let mut full = prefix.clone();
                    full.extend(path);
                    full
                })
                .collect()
        }
    }
}

fn normalize(tree: &str) -> String {
    let around = Regex::new(r"\s*(::|\{|\}|,)\s*").expect("valid regex");
    let spaces = Regex::new(r"\s+").expect("valid regex");
    let tight = around.replace_all(tree, "$1");
    spaces.replace_all(tight.trim(), " ").into_owned()
}

fn raw_paths(code: &str) -> Vec<Vec<String>> {
    let uses = Regex::new(r"\buse\s+([^;]+);").expect("valid regex");
    let inline = Regex::new(r"\b(?:crate|super)(?:::\w+)+").expect("valid regex");
    let mut paths: Vec<Vec<String>> = uses
        .captures_iter(code)
        .flat_map(|captures| flatten(&normalize(&captures[1])))
        .collect();
    let remainder = uses.replace_all(code, "");
    paths.extend(
        inline
            .find_iter(&remainder)
            .map(|found| found.as_str().split("::").map(str::to_string).collect()),
    );
    paths
}

enum Resolved {
    Crate(Vec<String>),
    Escapes,
    NotCrateLocal,
}

fn resolve(module: &[String], path: &[String]) -> Resolved {
    match path.first().map(String::as_str) {
        Some("crate") => Resolved::Crate(path[1..].to_vec()),
        Some("super") => {
            let ups = path
                .iter()
                .take_while(|segment| *segment == "super")
                .count();
            if ups > module.len() {
                return Resolved::Escapes;
            }
            let mut absolute = module[..module.len() - ups].to_vec();
            absolute.extend(path[ups..].iter().cloned());
            Resolved::Crate(absolute)
        }
        _ => Resolved::NotCrateLocal,
    }
}

fn starts_with(path: &[String], prefix: &[&str]) -> bool {
    path.len() >= prefix.len() && path.iter().zip(prefix).all(|(left, right)| left == right)
}

fn allowed(path: &[String], context: &str, layer: Layer, module: &[String]) -> bool {
    match layer {
        Layer::UseCase => {
            starts_with(path, &["contexts", context, "domain"])
                || module
                    .get(..4)
                    .is_some_and(|own| path.len() >= 4 && path[..4] == *own)
        }
        Layer::Infrastructure => {
            starts_with(path, &["contexts", context, "domain"])
                || starts_with(path, &["contexts", context, "infrastructure"])
                || starts_with(path, &["common"])
        }
        Layer::Presentation => {
            starts_with(path, &["contexts", context, "domain"])
                || starts_with(path, &["contexts", context, "use_cases"])
                || starts_with(path, &["contexts", context, "presentation"])
                || starts_with(path, &["common"])
        }
        Layer::Injector => {
            starts_with(path, &["contexts", context]) || starts_with(path, &["common"])
        }
        Layer::Domain => starts_with(path, &["contexts", context, "domain"]),
        Layer::Root => starts_with(path, &["contexts", context]),
    }
}

fn violations_of(file: &SourceFile) -> Vec<String> {
    let code = without_test_code(&file.text);
    let mut found = Vec::new();
    for path in raw_paths(&code) {
        let shown = path.join("::");
        match resolve(&file.module, &path) {
            Resolved::Escapes => found.push(format!("{}: {shown} leaves the crate", file.relative)),
            Resolved::NotCrateLocal => {}
            Resolved::Crate(absolute) => {
                let reported = match &file.owner {
                    Owner::Common => {
                        starts_with(&absolute, &["contexts"])
                            || starts_with(&absolute, &["composition"])
                    }
                    Owner::Context { name, layer } => {
                        !allowed(&absolute, name, *layer, &file.module)
                    }
                };
                if reported {
                    found.push(format!(
                        "{}: {shown} resolves to crate::{}",
                        file.relative,
                        absolute.join("::")
                    ));
                }
            }
        }
    }
    if matches!(
        &file.owner,
        Owner::Context {
            layer: Layer::Domain,
            ..
        }
    ) {
        let forbidden = Regex::new(FORBIDDEN_IN_DOMAIN).expect("valid regex");
        for found_crate in forbidden.find_iter(&code) {
            found.push(format!(
                "{}: domain imports {}",
                file.relative,
                found_crate.as_str().trim_end_matches("::")
            ));
        }
    }
    found
}

fn violations(root: &Path) -> Vec<String> {
    sources(root).iter().flat_map(violations_of).collect()
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/composition/test/fixtures")
        .join(name)
}

fn assert_reported(name: &str, expected: &[&str]) {
    assert_eq!(violations(&fixture(name)), expected, "fixture {name}");
}

mod integration {
    use super::*;

    #[test]
    fn l_arborescence_reelle_respecte_les_frontieres_entre_couches() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        assert_eq!(violations(&root), Vec::<String>::new());
    }

    #[test]
    fn l_usage_d_un_autre_contexte_est_signale() {
        assert_reported(
            "cross_context_use",
            &[
                "contexts/connections/infrastructure/client.rs: crate::contexts::querying::Report resolves to crate::contexts::querying::Report",
            ],
        );
    }

    #[test]
    fn un_use_groupe_entre_accolades_vers_un_autre_contexte_est_signale() {
        assert_reported(
            "brace_grouped_use",
            &[
                "contexts/connections/injector.rs: crate::contexts::anonymization::Kind resolves to crate::contexts::anonymization::Kind",
            ],
        );
    }

    #[test]
    fn un_groupe_imbrique_ne_signale_que_la_branche_interdite() {
        assert_reported(
            "nested_brace_group",
            &[
                "contexts/connections/injector.rs: crate::contexts::querying::Report resolves to crate::contexts::querying::Report",
            ],
        );
    }

    #[test]
    fn un_chemin_en_ligne_vers_un_autre_contexte_est_signale() {
        assert_reported(
            "inline_path",
            &[
                "contexts/connections/presentation/handler.rs: crate::contexts::anonymization::Kind::is_known resolves to crate::contexts::anonymization::Kind::is_known",
            ],
        );
    }

    #[test]
    fn une_chaine_de_super_sortant_du_contexte_est_signalee() {
        assert_reported(
            "super_escape",
            &[
                "contexts/connections/domain/rule.rs: super::super::super::anonymization::Kind resolves to crate::contexts::anonymization::Kind",
            ],
        );
    }

    #[test]
    fn une_chaine_de_super_sortant_de_la_crate_est_signalee() {
        assert_reported(
            "super_beyond_crate",
            &["contexts/connections/mod.rs: super::super::super::Anything leaves the crate"],
        );
    }

    #[test]
    fn un_ancien_alias_de_chemin_dans_un_contexte_est_signale() {
        assert_reported(
            "old_path_alias",
            &[
                "contexts/connections/presentation/handler.rs: crate::anon::Kind resolves to crate::anon::Kind",
            ],
        );
    }

    #[test]
    fn la_reexportation_d_un_contexte_via_common_est_signalee() {
        assert_reported(
            "common_reexport",
            &[
                "common/mod.rs: crate::contexts::connections::Session resolves to crate::contexts::connections::Session",
            ],
        );
    }

    #[test]
    fn common_qui_importe_composition_est_signale() {
        assert_reported(
            "common_imports_composition",
            &["common/config.rs: crate::composition::wire resolves to crate::composition::wire"],
        );
    }

    #[test]
    fn un_contexte_qui_importe_composition_est_signale() {
        assert_reported(
            "composition_import",
            &[
                "contexts/querying/presentation/handler.rs: crate::composition::server::Server resolves to crate::composition::server::Server",
            ],
        );
    }

    #[test]
    fn common_est_signale_dans_un_cas_d_usage_et_accepte_dans_un_injecteur() {
        assert_reported(
            "common_in_use_case",
            &[
                "contexts/connections/use_cases/open_session/use_case.rs: crate::common::AppConfig resolves to crate::common::AppConfig",
            ],
        );
    }

    #[test]
    fn un_cas_d_usage_qui_atteint_une_autre_couche_de_son_contexte_est_signale() {
        assert_reported(
            "use_case_reaches_infrastructure",
            &[
                "contexts/connections/use_cases/open_session/use_case.rs: crate::contexts::connections::infrastructure::Client resolves to crate::contexts::connections::infrastructure::Client",
            ],
        );
    }

    #[test]
    fn un_domaine_ou_une_infrastructure_qui_atteint_une_couche_exterieure_est_signale() {
        assert_reported(
            "domain_reaches_outer_layers",
            &[
                "contexts/connections/domain/types/session.rs: crate::contexts::connections::infrastructure::Client resolves to crate::contexts::connections::infrastructure::Client",
                "contexts/connections/infrastructure/repositories/catalog.rs: crate::contexts::connections::presentation::Reply resolves to crate::contexts::connections::presentation::Reply",
            ],
        );
    }

    #[test]
    fn une_crate_d_infrastructure_dans_le_domaine_est_signalee() {
        assert_reported(
            "infrastructure_crate_in_domain",
            &["contexts/connections/domain/types/session.rs: domain imports rusqlite"],
        );
    }

    #[test]
    fn les_references_autorisees_et_les_dossiers_de_test_ne_sont_pas_signales() {
        assert_reported("allowed_references", &[]);
    }
}
