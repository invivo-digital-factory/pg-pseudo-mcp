use std::fs;
use std::path::{Path, PathBuf};

const ALLOWED: &[(&str, &str)] = &[(
    "src/contexts/connections/test/dbeaver.rs",
    "PGMCP_TEST_HOST_SET",
)];

fn rust_files(directory: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(directory).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(rust_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    found
}

fn env_mutations(relative: &str, text: &str) -> Vec<String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| line.contains("set_var(") || line.contains("remove_var("))
        .filter(|(_, line)| {
            !ALLOWED.iter().any(|(file, variable)| {
                *file == relative && line.contains(&format!("\"{variable}\""))
            })
        })
        .map(|(index, line)| format!("{relative}:{}: {}", index + 1, line.trim()))
        .collect()
}

fn mutations_in(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for directory in ["src", "tests"] {
        for file in rust_files(&root.join(directory)) {
            let relative = file
                .strip_prefix(root)
                .expect("file under root")
                .to_string_lossy()
                .replace('\\', "/");
            if relative == "tests/env_hygiene.rs" {
                continue;
            }
            let text = fs::read_to_string(&file).expect("readable source");
            found.extend(env_mutations(&relative, &text));
        }
    }
    found
}

mod unit {
    use super::*;

    #[test]
    fn aucun_test_ni_le_code_ne_modifie_l_environnement_du_processus() {
        let found = mutations_in(Path::new(env!("CARGO_MANIFEST_DIR")));
        assert!(
            found.is_empty(),
            "process environment mutated at {found:?}; tests run in parallel threads of one \
             process, so any code reading that variable races with the mutation even behind \
             a mutex it does not take; inject the value (a path, a setting) as a dependency \
             instead of reading it from the environment"
        );
    }

    #[test]
    fn le_controle_detecte_une_mutation_et_tolere_l_exception_declaree() {
        let line = "        unsafe { std::env::set_var(\"PGPASSFILE\", path) };";
        assert_eq!(env_mutations("tests/x.rs", line).len(), 1);
        assert_eq!(
            env_mutations("tests/x.rs", "unsafe { std::env::remove_var(\"HOME\") };").len(),
            1
        );
        let allowed = "unsafe { std::env::set_var(\"PGMCP_TEST_HOST_SET\", \"h\") };";
        assert!(env_mutations("src/contexts/connections/test/dbeaver.rs", allowed).is_empty());
        assert_eq!(env_mutations("src/other.rs", allowed).len(), 1);
    }
}
