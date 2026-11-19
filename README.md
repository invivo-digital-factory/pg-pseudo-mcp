# pg-pseudo-mcp

A local MCP server that lets an AI coding agent investigate your PostgreSQL databases on its own, without any real personal value reaching the model provider through the server.

It reads the connections you already have in DBeaver, runs only read-only SQL, and replaces every personal value in results, errors and query plans by a stable, realistic pseudonym. When the agent reuses a pseudonym in a later query, the server translates it back to the real value locally, so the agent can follow a customer across tables without ever seeing who they are.

```text
agent: SELECT email FROM customers WHERE id = 1
server: claire.martin81@example.net            (real: marie.dupont@yourcompany.fr)
agent: SELECT * FROM orders o JOIN customers c ON c.id = o.customer_id
       WHERE c.email = 'claire.martin81@example.net'
server: Marie Dupont's orders, every personal value pseudonymized, translated_literals: 1
```

## Install

Requires a Rust toolchain (1.88 or later) to build; the result is a single binary with no runtime.

```bash
cargo install --path .
```

Register it with Claude Code:

```bash
claude mcp add pg-pseudo -- pg-pseudo-mcp
```

The server finds your DBeaver workspace automatically (`~/Library/DBeaverData` on macOS, `~/.local/share/DBeaverData` on Linux, `%APPDATA%\DBeaverData` on Windows). No configuration file is needed.

| Environment variable | Purpose |
|---|---|
| `PG_PSEUDO_DBEAVER_HOME` | DBeaver data root, when it is not in the default place |
| `PG_PSEUDO_DATA_DIR` | Where the pseudonym secret and mapping live (default: the OS data directory, e.g. `~/Library/Application Support/pg-pseudo-mcp`) |
| `RUST_LOG` | Log level; logs go to stderr and never contain values, SQL or credentials |

## Tools

All tools are read-only (`readOnlyHint: true`) and can be allowlisted.

| Tool | What it does |
|---|---|
| `list_connections` | DBeaver PostgreSQL connections with status (available, unavailable, unsupported), reason and type (dev, test, prod). Never hosts or credentials. |
| `search_schema` | Tables and columns whose name matches a pattern. |
| `list_tables` | Tables and views with kind, estimated rows and column count. |
| `describe_table` | Columns, keys in both directions, indexes, estimates, and which columns will be pseudonymized. |
| `query` | One read-only statement; rows with pseudonymized values, the list of pseudonymized columns and why, translated literals, warnings, structured truncation. |
| `explain` | The plan of one statement, without executing it; literals in the plan are pseudonymized. |
| `server_info` | Live limits, reversible and non-reversible kinds, known limitations. |

## How values are pseudonymized

- **Detection is automatic.** Column names are matched against the [PostgreSQL Anonymizer](https://gitlab.com/dalibo/postgresql_anonymizer) identifier dictionary; values are scanned for emails, French phone numbers, IBAN (mod 97), SIREN and SIRET (Luhn), NIR (key), card numbers, IP addresses, person names (French name lists and civility markers such as `M.` or `Mme`) and street addresses.
- **Anything that cannot be judged is anonymized.** Short capitalised values in unrecognised text columns are treated as names; computed columns (`lower(email)`, `to_json(row)`, `string_agg(...)`) are scanned value by value; hashes and base64 in computed columns become opaque tokens; `bytea` and unknown types become `<type len=N>`.
- **Pseudonyms keep their shape and stay stable.** An email stays an email on a reserved domain (`example.com`, `example.net`, `example.org`), a phone number stays a French number in a fictional range, an IBAN keeps a valid key. The same real value always gets the same pseudonym on your machine.
- **Free text keeps its structure.** Only the personal parts of a sentence or a JSON document are replaced.
- **Reuse in filters.** Emails, phones, IBAN, SIREN, SIRET, NIR, card numbers, IP addresses and full or last names are translated back when compared with a column (`=`, `<>`, `IN`, `= ANY`, `LIKE`). First names alone, cities, postal codes, civility, gender, street addresses and birth dates are not reversible. A pseudonym used anywhere else stays as written and the result carries a warning.

Sorting, `MIN`/`MAX`, ranges, `LIKE`, regular expressions, length, substrings, hashing and date arithmetic run on the real values inside the database, so their results may not match the pseudonyms you see.

## Read-only guarantee

Every statement must be a single `SELECT`, `WITH ... SELECT`, `VALUES` or `EXPLAIN` without `ANALYZE`. Writable CTEs, `SELECT INTO`, row locks, session changes, server functions (`pg_terminate_backend`, `set_config`, `nextval`, `pg_sleep`, `dblink`, large objects, file access...) and catalog relations other than structural ones are refused with the rule that blocked them. Each call runs in a `READ ONLY` transaction with a statement timeout, then rolls back. For defence in depth, point DBeaver at a role that can only `SELECT`.

## Threat model and known limits

pg-pseudo-mcp protects against **accidental disclosure by a cooperative agent**. It is not a security boundary against an agent that tries to recover real values on purpose:

- **Character-level probing.** SQL predicates run on real values, so `WHERE email LIKE 'a%'`, `ascii(substr(email, 1, 1))` or counts can rebuild a value one character at a time. Text read from the database can also carry instructions aimed at the agent (prompt injection).
- **Same-account file access.** The agent runs as your user. With its own shell or file tools it can read DBeaver's credentials (DBeaver's encryption key is public), the pseudonym secret and the mapping, or connect with `psql`. Deny those paths, see below.
- **Backups.** The mapping stores real values encrypted with a key derived from the secret, but both sit in the same directory; a backup (Time Machine, for example) that copies the directory copies everything needed to decrypt it.
- **Cut free text.** SQL string functions such as `left(body, 30)` cut free text on real values, so a phone number or an email split at the cut can come back as fragments no detector recognises. Read long values with `cell_offset` instead.
- **Detection misses.** Names absent from the built-in lists, inside long free text and without a civility marker, can pass through. Unusual identifiers without a checksum are not recognised.
- **Computed timestamps.** Computed `date` values are shifted like birth dates, but computed timestamps are kept so that `date_trunc` and time-series analysis stay exact; a birth date explicitly cast to `timestamp` therefore passes in clear. Intervals computed from birth dates (`age()`, date differences) are exact and reveal the real birth date.
- **Unsupported connections.** SSH jump servers, proxies, authentication models other than native and pgpass, and credentials protected by a DBeaver master password or secure storage are listed as unsupported or unavailable. Servers with certificates from a private CA are not supported yet.
- **SSH tunnels.** Connections that go through DBeaver's SSH tunnel are usable with public key, password or agent authentication. The SSH user and the password or key passphrase are read from DBeaver's saved credentials; a key without a saved passphrase is accepted and fails at connect time if it is encrypted. A key file that does not exist marks the connection unavailable. The SSH host key is checked against `~/.ssh/known_hosts` (or `PG_PSEUDO_SSH_KNOWN_HOSTS`), plain or hashed, with the `[host]:port` form for ports other than 22; an unknown, changed or `@revoked` key refuses the connection with the fingerprint to check, as does a key whose type differs from the one already recorded for the host. `@cert-authority` lines and host certificates are not supported. Add a bastion once with `ssh -p <port> <user>@<host>` or, after checking the fingerprint with its administrator, `ssh-keyscan -p <port> <host> >> ~/.ssh/known_hosts`. Verification is skipped only when "bypass host verification" is checked in DBeaver; `list_connections` then shows `"ssh_host_key_check": "bypassed"` for that connection and each connect logs a warning. SSH jump servers are not supported yet. With pgpass authentication, the database password comes from `PGPASSFILE` or `~/.pgpass` (`%APPDATA%\postgresql\pgpass.conf` on Windows), matched on the database host as written in DBeaver, not the tunnel host. As with libpq, a pgpass file readable by group or others is ignored and the connection is unavailable until it is `chmod 600`.

### Claude Code deny rules

Add these to your Claude Code settings (`~/.claude/settings.json`) so the agent's own tools do not read the secret, the mapping or your DBeaver credentials, or bypass the server with a database client. Adjust the paths to your OS, and add your `PG_PSEUDO_DATA_DIR` if you set one. The same rules are in [docs/claude-code-deny-rules.json](docs/claude-code-deny-rules.json).

These rules prevent accidental access; they are not a security boundary, since a shell can reach the same files in other ways. For real isolation, run the agent in a sandbox that cannot read those paths (for example the Claude Code sandbox) or under a separate OS user.

```json
{
  "permissions": {
    "deny": [
      "Read(~/Library/Application Support/pg-pseudo-mcp/**)",
      "Read(~/.local/share/pg-pseudo-mcp/**)",
      "Read(~/Library/DBeaverData/**)",
      "Read(~/.local/share/DBeaverData/**)",
      "Bash(psql:*)",
      "Bash(pg_dump:*)",
      "Bash(sqlite3:*)"
    ]
  }
}
```

### Resetting pseudonyms

```bash
pg-pseudo-mcp mapping purge
```

Deletes the secret and the mapping after confirmation. Pseudonyms the agent saw before become unknown and are reported as `unresolved_pseudonym`.

## Development

```bash
docker compose up -d --wait
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
PG_PSEUDO_TEST_DATABASE_URL=postgres://pgpseudo:pgpseudo@localhost:55432/pgpseudo cargo test -- --include-ignored
```

The integration suite seeds fictional French personal data and checks that no seeded value, in any case, as an MD5 or base64 encoding, or as a fragment of an email, appears in any response.

## Third-party data

- `data/identifier.csv` comes from PostgreSQL Anonymizer by DALIBO, under the PostgreSQL Licence; see `data/NOTICE`.
- French first and last name lists come from the [`fake`](https://crates.io/crates/fake) crate (MIT or Apache-2.0).

## Licence

Not chosen yet.
