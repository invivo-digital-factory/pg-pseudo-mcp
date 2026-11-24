---
title: A renamed data directory must not let an accidental new directory win over the existing secret
date: 2026-11-24
category: runtime-errors
module: anonymization data directory
problem_type: runtime_error
component: infrastructure
symptoms:
  - "After upgrading to 0.3.0, every pseudonym changes for one user"
  - "Pseudonyms seen before are no longer translated back"
root_cause: logic_error
resolution_type: code_fix
severity: high
tags: [data-directory, rename, migration, secret, pseudonym, test-isolation]
---

# A renamed data directory must not let an accidental new directory win over the existing secret

## Problem
0.3.0 renamed the data directory from `pg-pseudo-mcp` to `scarif-database-mcp`. The rule was: use the old directory while the new one does not exist, then let the new one win. On a developer machine the new directory already existed, created with a fresh secret while the rename was being developed. 0.3.0 used it and ignored the old one: every pseudonym changed and earlier pseudonyms no longer translated back. Nothing was lost, but nothing said that the old secret was being ignored.

## Cause
- The rule tested whether the new directory existed, not which directory held the secret the user's pseudonyms came from.
- The server creates the directory it resolves, and its secret, at startup (`open_pseudonymizer`). So a single run that resolves to the new path (a test, a run with default settings) turns an absent directory into a competing one with its own secret. An empty new directory gets the same treatment on the next start.
- `tests/stdio_smoke.rs` started the binary without `SCARIF_DATA_DIR`, so `cargo test` opened the developer's real data directory. Before the fallback was written, a run like that is enough to create the new directory.

## Rule
- With a renamed data directory, the directory that holds the existing secret wins. When both exist and the old one holds a secret, the server keeps the old one, reports `data_directory_conflict` on stderr and in `server_info` with both paths and the two ways out, and never deletes, moves or merges anything. The new directory is used when the old one is absent, or when the old one holds no secret. An explicit `SCARIF_DATA_DIR` is always used as given.
- The default location comes from `AppConfig::data_home`, which only `AppConfig::from_env` fills. A config built in a test with no data directory resolves nothing (`NoDataDir`) and cannot reach the real one.
- Every test that starts the binary sets `SCARIF_DATA_DIR` to a temporary directory. `tests/data_dir_hygiene.rs` enforces it.

## Related
- `src/contexts/anonymization/infrastructure/data_sources/data_directory.rs` (`keep_existing_legacy`)
- `src/composition/test/data_directory.rs`
- README, section "Renamed from pg-pseudo-mcp"
