use std::fs;
use std::path::{Path, PathBuf};

const LAUNCH: &str = "Command::new(";
const LAUNCH_ENDS: [&str; 3] = [".spawn()", ".output()", ".status()"];
const ISOLATION: &str = ".env(\"SCARIF_DATA_DIR\"";

fn launches_without_isolated_data_dir(source: &str) -> usize {
    source
        .split(LAUNCH)
        .skip(1)
        .filter(|launch| {
            let end = LAUNCH_ENDS
                .iter()
                .filter_map(|marker| launch.find(marker))
                .min()
                .unwrap_or(launch.len());
            !launch[..end].contains(ISOLATION)
        })
        .count()
}

fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.file_name() != Path::new(file!()).file_name()
            && path.extension().is_some_and(|extension| extension == "rs")
        {
            found.push(path);
        }
    }
}

mod unit {
    use super::*;

    #[test]
    fn chaque_lancement_du_binaire_dans_les_tests_isole_son_dossier_de_donnees() {
        let mut files = Vec::new();
        rust_files(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests"),
            &mut files,
        );
        let offenders: Vec<_> = files
            .iter()
            .filter(|path| {
                launches_without_isolated_data_dir(&fs::read_to_string(path).unwrap()) > 0
            })
            .map(|path| path.display().to_string())
            .collect();
        assert!(
            offenders.is_empty(),
            "{offenders:?} launch the binary without SCARIF_DATA_DIR: the server then opens, \
             and creates when missing, the real pseudonym directory of the developer's machine"
        );
    }

    #[test]
    fn le_controle_d_hygiene_detecte_un_lancement_sans_dossier_de_donnees() {
        let isolated = "Command::new(bin).env(\"SCARIF_DATA_DIR\", dir).spawn()";
        let exposed = "Command::new(bin).env(\"HOME\", dir).spawn()";
        assert_eq!(launches_without_isolated_data_dir(isolated), 0);
        assert_eq!(launches_without_isolated_data_dir(exposed), 1);
        assert_eq!(
            launches_without_isolated_data_dir(&format!("{isolated}\n{exposed}")),
            1
        );
    }
}
