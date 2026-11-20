use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use crate::contexts::anonymization::use_cases::purge_mapping::PurgeMapping;

pub struct MappingPurgeHandler {
    purge_mapping: PurgeMapping,
    data_dir: PathBuf,
}

impl MappingPurgeHandler {
    pub fn new(purge_mapping: PurgeMapping, data_dir: PathBuf) -> Self {
        Self {
            purge_mapping,
            data_dir,
        }
    }

    pub fn run(&self, confirmed: bool) -> ExitCode {
        if !confirmed {
            eprint!(
                "This deletes the pseudonym mapping and its secret in {}.\nPseudonyms already handed to agents will no longer translate back.\nType yes to confirm: ",
                self.data_dir.display()
            );
            let _ = std::io::stderr().flush();
            let mut answer = String::new();
            if std::io::stdin().lock().read_line(&mut answer).is_err() || answer.trim() != "yes" {
                eprintln!("Aborted, nothing deleted.");
                return ExitCode::FAILURE;
            }
        }
        match self.purge_mapping.execute() {
            Ok(deleted) if deleted.is_empty() => {
                eprintln!("Nothing to delete in {}.", self.data_dir.display());
                ExitCode::SUCCESS
            }
            Ok(deleted) => {
                for path in deleted {
                    eprintln!("deleted {}", path.display());
                }
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        }
    }
}
