use std::io::{ErrorKind, Read};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(10);
const CHUNK_SIZE: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundedFailure {
    NotFound,
    CouldNotStart,
    TimedOut,
    OutputTooLarge,
}

pub struct BoundedOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
}

pub fn run_bounded(
    command: &mut Command,
    deadline: Duration,
    max_output: usize,
) -> Result<BoundedOutput, BoundedFailure> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(command, 0);
    let started = Instant::now();
    let mut child = command.spawn().map_err(|error| match error.kind() {
        ErrorKind::NotFound => BoundedFailure::NotFound,
        _ => BoundedFailure::CouldNotStart,
    })?;
    let pipe = child.stdout.take();
    let (sender, receiver) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        let Some(mut pipe) = pipe else { return };
        let mut chunk = [0u8; CHUNK_SIZE];
        while let Ok(read) = pipe.read(&mut chunk) {
            if read == 0 || sender.send(chunk[..read].to_vec()).is_err() {
                return;
            }
        }
    });
    let mut stdout = Vec::new();
    let failure = loop {
        let Some(remaining) = deadline.checked_sub(started.elapsed()) else {
            break Some(BoundedFailure::TimedOut);
        };
        match receiver.recv_timeout(remaining) {
            Ok(chunk) => {
                stdout.extend_from_slice(&chunk);
                if stdout.len() > max_output {
                    break Some(BoundedFailure::OutputTooLarge);
                }
            }
            Err(RecvTimeoutError::Timeout) => break Some(BoundedFailure::TimedOut),
            Err(RecvTimeoutError::Disconnected) => break None,
        }
    };
    if let Some(failure) = failure {
        terminate(&mut child);
        return Err(failure);
    }
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(BoundedOutput { status, stdout }),
            Ok(None) if started.elapsed() < deadline => thread::sleep(POLL_INTERVAL),
            Ok(None) => {
                terminate(&mut child);
                return Err(BoundedFailure::TimedOut);
            }
            Err(_) => {
                terminate(&mut child);
                return Err(BoundedFailure::CouldNotStart);
            }
        }
    }
}

fn terminate(child: &mut std::process::Child) {
    #[cfg(unix)]
    if let Ok(group) = i32::try_from(child.id()) {
        unsafe {
            libc::kill(-group, libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(all(test, unix))]
mod integration {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use super::*;

    fn script(dir: &Path, body: &str) -> Command {
        let path = dir.join("tool");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        Command::new(path)
    }

    fn run_timed(
        body: &str,
        deadline: Duration,
        max: usize,
    ) -> (Result<BoundedOutput, BoundedFailure>, Duration) {
        let dir = tempfile::tempdir().unwrap();
        let mut command = script(dir.path(), body);
        let started = Instant::now();
        let outcome = run_bounded(&mut command, deadline, max);
        (outcome, started.elapsed())
    }

    fn run(body: &str, deadline: Duration, max: usize) -> Result<BoundedOutput, BoundedFailure> {
        let dir = tempfile::tempdir().unwrap();
        run_bounded(&mut script(dir.path(), body), deadline, max)
    }

    #[test]
    fn un_descendant_en_arriere_plan_qui_garde_la_sortie_ouverte_respecte_le_delai() {
        let deadline = Duration::from_millis(300);
        let (outcome, elapsed) = run_timed("sleep 30 &\nprintf 'pw'", deadline, 1024);
        assert_eq!(outcome.err(), Some(BoundedFailure::TimedOut));
        assert!(elapsed < deadline * 20);
    }

    #[test]
    fn le_delai_depasse_tue_tout_le_groupe_de_processus() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("survivor");
        let body = format!("(sleep 1; touch {}) &\nsleep 30", marker.display());
        let outcome = run(&body, Duration::from_millis(200), 1024);
        assert_eq!(outcome.err(), Some(BoundedFailure::TimedOut));
        std::thread::sleep(Duration::from_millis(1500));
        assert!(!marker.exists());
    }

    #[test]
    fn une_sortie_trop_volumineuse_est_refusee_et_arrete_la_commande() {
        let deadline = Duration::from_secs(30);
        let (outcome, elapsed) = run_timed("yes", deadline, 1024);
        assert_eq!(outcome.err(), Some(BoundedFailure::OutputTooLarge));
        assert!(elapsed < deadline / 2);
    }

    #[test]
    fn une_commande_normale_rend_son_statut_et_sa_sortie() {
        let output = run("printf 'abc'\nexit 3", Duration::from_secs(5), 1024).unwrap();
        assert_eq!(output.status.code(), Some(3));
        assert_eq!(output.stdout, b"abc");
    }

    #[test]
    fn un_programme_absent_ou_non_executable_est_distingue() {
        let dir = tempfile::tempdir().unwrap();
        let missing = run_bounded(
            &mut Command::new(dir.path().join("nope")),
            Duration::from_secs(1),
            16,
        );
        assert_eq!(missing.err(), Some(BoundedFailure::NotFound));
        let plain = dir.path().join("plain");
        std::fs::write(&plain, "data").unwrap();
        let refused = run_bounded(&mut Command::new(plain), Duration::from_secs(1), 16);
        assert_eq!(refused.err(), Some(BoundedFailure::CouldNotStart));
    }
}
