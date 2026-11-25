# Development

How to build, test and extend scarif-database-mcp.

```bash
docker compose up -d --wait
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
SCARIF_TEST_DATABASE_URL=postgres://pgpseudo:pgpseudo@localhost:55432/pgpseudo cargo test -- --include-ignored
```

`SCARIF_PG_HOST_PORT` and `SCARIF_SSH_HOST_PORT` (compose) and `SCARIF_TEST_DATABASE_URL` and `SCARIF_TEST_SSH_PORT` (tests) must point at the same stack, which lets several local stacks coexist under different project names; `SCARIF_TEST_MONGODB=1` enables the MongoDB tests.

The two-hop SSH tests need a bastion that may open a channel to itself (`PermitOpen` in `tests/ssh/sshd_config`). After pulling a change to `tests/ssh`, rebuild the bastion once with `docker compose up -d --build ssh`.

The integration suite seeds fictional French personal data and checks that no seeded value, in any case, as an MD5 or base64 encoding, or as a fragment of an email, appears in any response.

## Contexts and business scenarios
The code lives in three contexts under `src/contexts/`, wired together in `src/composition/`:

- `connections`: reads the connection file and the DBeaver workspace, resolves SSH aliases with OpenSSH, imports DBeaver connections, and opens sessions, through a chain of SSH hops when configured.
- `anonymization`: pseudonymizes values, translates a pseudonym back locally, purges the mapping.
- `querying`: guards statements as read-only and answers the query, schema and explain tools.

Each context documents what it does with Cucumber features written in French, in `src/contexts/<context>/test/features/`, with their steps in `src/contexts/<context>/test/steps/`. Scenarios tagged `@integration` need the docker compose database and SSH bastion; without `SCARIF_TEST_DATABASE_URL` the runner skips them and says so.

```bash
cargo test --test bdd
SCARIF_TEST_DATABASE_URL=postgres://pgpseudo:pgpseudo@localhost:55432/pgpseudo cargo test --test bdd
```

## Third-party data

- `data/identifier.csv` comes from PostgreSQL Anonymizer by DALIBO, under the PostgreSQL Licence; see `data/NOTICE`.
- French first and last name lists come from the [`fake`](https://crates.io/crates/fake) crate (MIT or Apache-2.0).
