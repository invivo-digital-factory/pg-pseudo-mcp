# scarif-database-mcp

A local MCP server that lets an AI coding agent investigate your PostgreSQL databases on its own, without any real personal value reaching the model provider through the server.

It reads the connections of its own secret-free file (or the ones you already have in DBeaver), runs only read-only SQL, and replaces every personal value in results, errors and query plans by a stable, realistic pseudonym. When the agent reuses a pseudonym in a later query, the server translates it back to the real value locally, so the agent can follow a customer across tables without ever seeing who they are.

```text
agent: SELECT email FROM customers WHERE id = 1
server: claire.martin81@example.net            (real: marie.dupont@yourcompany.fr)
agent: SELECT * FROM orders o JOIN customers c ON c.id = o.customer_id
       WHERE c.email = 'claire.martin81@example.net'
server: Marie Dupont's orders, every personal value pseudonymized, translated_literals: 1
```

## Install

Each `v*` tag publishes a release with a single binary per platform (macOS arm64 and x86_64, Linux x86_64 and arm64) and a `SHA256SUMS` file:

```bash
version=0.3.0
target=aarch64-apple-darwin
curl -fsSLO "https://github.com/invivo-digital-factory/scarif-database-mcp/releases/download/v$version/scarif-database-mcp-$target.tar.gz"
curl -fsSLO "https://github.com/invivo-digital-factory/scarif-database-mcp/releases/download/v$version/SHA256SUMS"
grep "scarif-database-mcp-$target.tar.gz" SHA256SUMS | shasum -a 256 -c
tar -xzf "scarif-database-mcp-$target.tar.gz"
mkdir -p ~/.local/bin
install -m 755 "scarif-database-mcp-$target/scarif-database-mcp" ~/.local/bin/
```

To build from source instead, with a Rust toolchain (1.88 or later):

```bash
cargo install --path .
```

Register it with Claude Code:

```bash
claude mcp add scarif -- scarif-database-mcp
```

Describe your connections in [the connection file](#connections), or generate it from DBeaver with `scarif-database-mcp connections import-dbeaver`. Without a file, the server reads your DBeaver workspace, found automatically (`~/Library/DBeaverData` on macOS, `~/.local/share/DBeaverData` on Linux, `%APPDATA%\DBeaverData` on Windows).

| Environment variable | Purpose |
|---|---|
| `SCARIF_CONNECTIONS_FILE` | Absolute path of the connection file, instead of `~/.config/scarif/connections.toml` |
| `SCARIF_COLUMNS_FILE` | Absolute path of the shared [column policy](#column-policy); the local one is `~/.config/scarif/columns.toml` |
| `SCARIF_SSH_CONFIG` | Absolute path of an SSH config passed to `ssh -G` with `-F`, instead of `~/.ssh/config` |
| `SCARIF_SSH_KNOWN_HOSTS` | Absolute path of a `known_hosts` file used for every SSH hop instead of the files `ssh -G` lists |
| `SCARIF_DBEAVER_HOME` | DBeaver data root, when it is not in the default place |
| `SCARIF_DATA_DIR` | Where the pseudonym secret and mapping live (default: the OS data directory, e.g. `~/Library/Application Support/scarif-database-mcp`) |
| `RUST_LOG` | Log level; logs go to stderr and never contain values, SQL or credentials |

## Connections

scarif-database-mcp reads its connections from its own file, `~/.config/scarif/connections.toml` (or `$XDG_CONFIG_HOME/scarif/connections.toml`, or the absolute path in `SCARIF_CONNECTIONS_FILE`). The file holds **no secret**, so a team can keep it in a shared repository. DBeaver stays readable as a fallback source.

```toml
read_dbeaver = true

[[connection]]
name = "shop-prod"
host = "shop.postgres.database.azure.com"
database = "shop"
user = "alice@corp.example"
sslmode = "require"
type = "prod"
auth = "azure"

[[connection]]
name = "billing-test"
host = "billing-db.internal"
port = 5432
database = "billing"
user = "reader"
type = "test"
auth = "pgpass"
ssh_alias = "corp-bastion"

[[connection]]
name = "ledger-dev"
host = "ledger-db.internal"
user = "reader"
type = "dev"
auth = "command"
password_command = ["op", "read", "op://Dev/ledger-db/password"]
ssh_alias = "corp-jump-ledger"
```

| Key | Meaning |
|---|---|
| `name` | Unique name shown by `list_connections` and passed to the tools |
| `host`, `port` (5432), `database` (`postgres`), `user` | The database, as seen from the SSH host when `ssh_alias` is set; `database` is the connection's default database |
| `sslmode` | `disable`, `allow`, `prefer` (default), `require`, `verify-ca` or `verify-full` |
| `type` | `dev`, `test` or `prod` |
| `auth` | How the password is obtained, see below |
| `ssh_alias` | A `Host` alias of `~/.ssh/config` |
| `read_dbeaver` | Top-level; `false` turns the DBeaver source off entirely (default `true`) |

Edits to the file are picked up at the next tool call. A key that holds a secret (`password`, `token`, `passphrase`, any key containing `password` except `password_command`), an unknown key or an invalid value makes that entry unavailable, with a reason that names the field; two entries with the same name are both unavailable. A file that exists but cannot be parsed or read, or that has an invalid `read_dbeaver` value or an unknown or secret-shaped top-level key, is reported in the `list_connections` hint (with its line and column for a syntax error, never its content), and the DBeaver connections stay listed but unavailable, with a reason naming the connection file, until it is fixed: a broken file cannot silently re-expose connections it was meant to shadow or turn off. With a readable `read_dbeaver = false`, DBeaver stays off. A missing default file is silent; a missing file named by `SCARIF_CONNECTIONS_FILE` is reported with its path.

### Authentication modes

- `pgpass`: the password comes from `PGPASSFILE` or `~/.pgpass`, matched on the database host as written in the file (not the SSH host), with the usual permission rule (`chmod 600`).
- `azure`: the password is the Entra ID access token from the Azure CLI, see [Azure Database for PostgreSQL](#azure-database-for-postgresql). The mode is declared, not guessed from the host.
- `command`: the password is the standard output of `password_command`, a list made of the program then its arguments, run without a shell. It runs only when a session opens, never when connections are listed, so listing never opens a Keychain prompt. The output is kept in memory only (one trailing line ending is removed), reused by the sessions of the same connection, and the command is run once more after an authentication failure. A failing command gives a fixed reason (not found, could not start, exit status, timeout after 30 s, no output, not UTF-8) and never the command's output.

No password, token or command output appears in a tool response, a reason or a log.

### SSH tunnels

`ssh_alias` names a `Host` of `~/.ssh/config`. The server runs `ssh -G <alias>` and takes the host name, port, user, identity files, identity agent and `ProxyJump` chain from OpenSSH's own resolution, so `Include`, `Match`, wildcards and defaults behave as they do for `ssh`. The tunnel itself runs in the server, so every hop's host key is verified in the same way.

- **Jump hosts.** `ProxyJump` chains of one or more hops work, including `user@host:port` and `ssh://` forms; a first hop with its own `ProxyJump` is followed, up to eight levels.
- **Host keys are always verified**, on every hop, against the `known_hosts` files that `ssh -G` lists (`UserKnownHostsFile` and `GlobalKnownHostsFile`, `HostKeyAlias` honored) or against `SCARIF_SSH_KNOWN_HOSTS` when set. `StrictHostKeyChecking no` in `~/.ssh/config` is ignored on purpose, so it cannot turn verification off for the agent. An unknown, changed or revoked key refuses the connection with the hop's position and alias and the key's fingerprint. Host certificates and `@cert-authority` lines are not supported.
- **Authentication never prompts.** Keys come from the SSH agent (`IdentityAgent` honored; `IdentitiesOnly` filters the agent's keys by your `IdentityFile` entries) or from unencrypted identity files. An encrypted key without an agent fails at connect time and tells you to load it with `ssh-add`. Password authentication to a bastion is not supported for file connections.
- **Unsupported directives.** If the resolved alias sets `ProxyCommand`, `ProxyUseFdpass`, `KnownHostsCommand`, `CertificateFile`, `PKCS11Provider`, `CanonicalizeHostname`, `BindAddress` or `BindInterface`, the connection is listed as unsupported and names the directive; it is never connected with the directive silently dropped. An `IdentityFile` with a token other than `%d %h %p %r %u %%` is unsupported too.
- **The alias must be defined.** `ssh -G` resolves any name with OpenSSH defaults (port 22, your login, default identity files), so a typo or a missing `Host` block would look available and then time out at query time. The server therefore also reads the `Host` lines of `~/.ssh/config` (or of `SCARIF_SSH_CONFIG`), following `Include` up to 16 levels, and requires the connection's own `ssh_alias` to match a `Host` pattern other than a bare `*`; wildcards (`*`, `?`) and negations (`!pattern`) are evaluated as OpenSSH does. Only that alias is checked: a `ProxyJump` hop may be a literal host name or address with no `Host` block. Otherwise the connection is listed unavailable with the reason `SSH alias <alias> is not defined in the OpenSSH config; add a Host block for it (connections import-dbeaver prints one)`, which names only your own alias, and `ssh` is not run. `Match` blocks are not evaluated: they neither define an alias nor cause a refusal on their own. System-wide `/etc/ssh` files are not read. `ssh -G` still gives the resolved values.
- **No `ssh` on `PATH`.** Connections with an `ssh_alias` are unavailable with a reason saying so; the others are unaffected.

`SCARIF_SSH_CONFIG` (absolute path) makes the server pass `-F <path>` to every `ssh -G`, which is how the tests stay independent of your own `~/.ssh/config`.

### Precedence and DBeaver

The server lists the connections of the file and, unless `read_dbeaver = false`, those of the DBeaver workspace. A DBeaver connection whose name is defined in the file (even if that entry is unavailable) is not listed and cannot be used, so the file wins. `list_connections` shows the `source` of each connection (`file` or `dbeaver`). A missing DBeaver workspace is not an error. When neither source yields a connection, the hint tells you to create the file or run the import.

### Moving off DBeaver

```bash
scarif-database-mcp connections import-dbeaver [--force]
```

writes the file once from DBeaver's PostgreSQL connections, without any password, token or passphrase. It refuses to overwrite an existing file unless `--force` is given. It carries name, host, port, database, user, `sslmode` and type (a type other than dev, test or prod is imported as `prod`, the most cautious one), maps native authentication to `pgpass` and pgpass authentication on an Azure host to `azure`, applies the same name suffixes as `list_connections` for duplicate names, and keeps the host and port a DBeaver tunnel points at. For each SSH tunnel it reuses an alias of your `~/.ssh/config` that already resolves to the same host, port and user, and otherwise prints a suggested `Host` block (host, port, user, identity file); it never edits `~/.ssh/config`. It ends with the skipped connections and their reason (other engines, jump servers, unsupported authentication) and, per connection, what is left to do: a pgpass entry or a `password_command`, the `Host` block, the `known_hosts` entries. Once the file exists it takes precedence over DBeaver, so add the suggested `Host` blocks before using the servers (a connection whose alias has no `Host` block is unavailable), or keep the file aside until then; the report says so at its top. Then check each connection shows `source: file` and `available` in `list_connections`, and set `read_dbeaver = false` when you no longer need DBeaver.

## Tools

All tools are read-only (`readOnlyHint: true`) and can be allowlisted.

| Tool | What it does |
|---|---|
| `list_connections` | PostgreSQL connections of the connection file and of DBeaver, with source (`file` or `dbeaver`), status (available, unavailable, unsupported), reason and type (dev, test, prod). Never hosts or credentials. |
| `list_databases` | The databases of the server behind a connection, ordered by name (connectable, not templates), and the connection's default database, the one the connection points at. The fixed query is run by the server itself; `SELECT ... FROM pg_database` sent through `query` stays refused. |
| `search_schema` | Tables and columns whose name matches a pattern. |
| `list_tables` | Tables and views with kind, estimated rows and column count. |
| `describe_table` | Columns, keys in both directions, indexes, estimates, and which columns will be pseudonymized. |
| `query` | One read-only statement; rows with pseudonymized values, the list of pseudonymized columns and why, translated literals, warnings, structured truncation. |
| `explain` | The plan of one statement, without executing it; literals in the plan are pseudonymized. |
| `server_info` | Live limits, reversible and non-reversible kinds, known limitations. |

### Choosing the database

A connection points at one database, often `postgres`, while the application databases live on the same server. `query`, `explain`, `search_schema`, `list_tables` and `describe_table` accept an optional `database` argument; its names come from `list_databases`. It keeps the host, port, SSH tunnel, credentials, Azure token handling and `sslmode` of the connection and only changes the database name, including the one matched in `~/.pgpass`. Sessions are kept per connection and database: the default database plus at most four others per connection, the least recently used one being closed with its tunnel. Without the argument the connection's own database is used. The read-only guard and the pseudonymization are the same for every database. Database names are returned as they are, since they are the value of the argument. A database that does not exist is reported with the server message and SQLSTATE `3D000`.

## Azure Database for PostgreSQL

For a connection declared with `auth = "azure"` in the connection file, or that uses DBeaver's pgpass authentication model, and whose host ends with `.postgres.database.azure.com` (private link hosts included), the server gets the password itself from the Azure CLI instead of `~/.pgpass`: it runs `az account get-access-token --resource-type oss-rdbms --output json` (no shell, `az` found on `PATH`) and uses the Entra ID access token as the password.

- The token is kept in memory only, reused until about five minutes before it expires, then fetched again. A connection refused for authentication (SQLSTATE `28P01` or `28000`) is retried once with a freshly fetched token, and reconnections of an idle session pick up the current token.
- If `az` is missing, not signed in or fails, the matching `~/.pgpass` entry is used when there is one, as before. Otherwise `list_connections` shows the connection as unavailable with a reason that tells you to run `az login`. The reason is a fixed message per failure (CLI missing, sign-in required, unreadable output); it never carries the CLI output, which can name the signed-in account, nor a token.
- A file connection declared `auth = "azure"` on any other host is unavailable and never runs `az`: the Entra token is valid against every Azure PostgreSQL server you can reach, so it is only ever sent to a host ending in `.postgres.database.azure.com`. A custom DNS name for an Azure server is therefore refused in `azure` mode.
- Other hosts and other authentication models are unchanged.

The script that wrote an access token into `~/.pgpass` before each session is no longer needed for the MCP server; DBeaver itself still reads `~/.pgpass`.

## How values are pseudonymized

- **Detection is automatic.** Column names are matched against the [PostgreSQL Anonymizer](https://gitlab.com/dalibo/postgresql_anonymizer) identifier dictionary; values are scanned for emails, French phone numbers, IBAN (mod 97), SIREN and SIRET (Luhn), NIR (key), card numbers, IP addresses, person names (French name lists and civility markers such as `M.` or `Mme`) and street addresses.
- **Anything that cannot be judged is anonymized.** Short capitalised values in unrecognised text columns are treated as names; computed columns (`lower(email)`, `to_json(row)`, `string_agg(...)`) are scanned value by value; hashes and base64 in computed columns become opaque tokens; `bytea` and unknown types become `<type len=N>`.
- **Enum labels stay in clear.** A text value equal to an enum label of the database (ignoring case and accents) is kept as is, even through casts, functions, `UNION` or `CASE`, unless the column name marks it as personal data. Labels are read once per session.
- **Expressions over clear columns stay in clear.** A computed column is kept as is when every column it reads (through aliases, CTEs, subqueries in `FROM` and `UNION`) is clear by type outside the dictionary (enum, boolean, uuid, number, date; never text) and it only calls an allowlist of built-in functions that read no data (casts, `||`, `concat`, `lower`, `coalesce`, `CASE`, arithmetic, `to_char`, `count`, `string_agg`...). Any other function, such as a user-defined `get_customer_name(id)`, a subquery in the projection, `SELECT *` or an unresolved name keeps the usual judgment.
- **Pseudonyms keep their shape and stay stable.** An email stays an email on a reserved domain (`example.com`, `example.net`, `example.org`), a phone number stays a French number in a fictional range, an IBAN keeps a valid key. The same real value always gets the same pseudonym on your machine.
- **Free text keeps its structure.** Only the personal parts of a sentence or a JSON document are replaced.
- **Reuse in filters.** Emails, phones, IBAN, SIREN, SIRET, NIR, card numbers, IP addresses and full or last names are translated back when compared with a column (`=`, `<>`, `IN`, `= ANY`, `LIKE`). First names alone, cities, postal codes, civility, gender, street addresses and birth dates are not reversible. A pseudonym used anywhere else stays as written and the result carries a warning.

Sorting, `MIN`/`MAX`, ranges, `LIKE`, regular expressions, length, substrings, hashing and date arithmetic run on the real values inside the database, so their results may not match the pseudonyms you see.

## Column policy

The heuristics make mistakes in both directions: the dictionary maps the bare token `name` to a last name, so `product_name` or `table_name` are pseudonymized, and a text column of codes can look like names. A column policy fixes this by configuration.

Two optional TOML files are read, in this order, on every call (no restart needed); the later one wins on the same key:

1. **Shared**: the path in `SCARIF_COLUMNS_FILE` (absolute), typically a file the team maintains together.
2. **Local**: `$XDG_CONFIG_HOME/scarif/columns.toml`, or `~/.config/scarif/columns.toml`, resolved like the connection file.

```toml
clear = ["*.product_name", "public.payment.carrier_name", "orders.reference"]

[personal]
"customer.contact" = "person_name"
"*.siret_client" = "siret"
```

- **Keys** are column patterns: `schema.table.column`, `table.column` (any schema) or `*.column` (any table), matched without regard to case. When several patterns match a column, the most specific wins: `schema.table.column`, then `table.column`, then `*.column`. A pattern cannot be both `clear` and `personal` in the same file.
- **Kinds** accepted in `personal` are the pseudonym kinds listed by `server_info` (`email`, `phone`, `iban`, `siren`, `siret`, `nir`, `card_number`, `ip_address`, `person_name`, `last_name`, `first_name`, `address`, `city`, `postal_code`, `civility`, `gender`, `birth_date`).
- **Precedence** for a result column read from a table column (found through the catalog, as for the enum labels): policy `personal`, then policy `clear`, then the dictionary, then the type, then the heuristics. A computed column is never matched by its output alias.
- **Safety net.** A `clear` column skips the dictionary and the name, address and free-text heuristics, but its values still go through the structural detectors (email, phone, IBAN mod 97, SIREN and SIRET Luhn, NIR key, card numbers, IP addresses): a value they recognise is still replaced and reported as `detected`. A `clear` column also counts as clear for the expressions over clear columns described above, with the same safety net, so `concat(product_name, ' ', status)` stays in clear.
- **Personal.** A `personal` column is pseudonymized as the given kind whatever its name or type, reported with the reason `policy`, and an expression that reads a column of that name is treated as derived from personal data.
- **Invalid file.** A file that cannot be read or parsed, an invalid pattern or an unknown kind makes that file ignored (the other one still applies); the server keeps working, and `server_info` and every `list_connections` result carry a `column_policy_ignored` warning naming the file and the error, never a value.
- **Hints.** When a `query` result pseudonymizes a column read from a table column that no policy entry covers, either by default (the heuristics) or because the dictionary took it for a name (`last_name`, `first_name`, `person_name`, where false positives such as `product_name` come from), the result carries `column_policy_hints`, one entry per column. Dictionary hits on other kinds (email, phone, IBAN, SIREN, SIRET, NIR, card number, IP address, birth date, address, city, postal code, civility, gender) get no hint. Each entry reads `{ "column": "schema.table.column", "reason": "dictionary" | "default", "hint": "classify it in the column policy: clear or personal" }`. `describe_table` lists the same hints for the columns it reports as pseudonymized, and shows `"policy": "clear" | "personal"` for classified columns. Hints never carry values. The agent is told to propose a classification: in the local file for immediate use, in the shared list through the team's own process.

## Read-only guarantee

Every statement must be a single `SELECT`, `WITH ... SELECT`, `VALUES` or `EXPLAIN` without `ANALYZE`. Writable CTEs, `SELECT INTO`, row locks, session changes, server functions (`pg_terminate_backend`, `set_config`, `nextval`, `pg_sleep`, `dblink`, large objects, file access...) and catalog relations other than structural ones are refused with the rule that blocked them. Each call runs in a `READ ONLY` transaction with a statement timeout, then rolls back. For defence in depth, point DBeaver at a role that can only `SELECT`.

## Threat model and known limits

scarif-database-mcp protects against **accidental disclosure by a cooperative agent**. It is not a security boundary against an agent that tries to recover real values on purpose:

- **Character-level probing.** SQL predicates run on real values, so `WHERE email LIKE 'a%'`, `ascii(substr(email, 1, 1))` or counts can rebuild a value one character at a time. Text read from the database can also carry instructions aimed at the agent (prompt injection).
- **Same-account file access.** The agent runs as your user. With its own shell or file tools it can read DBeaver's credentials (DBeaver's encryption key is public), the pseudonym secret and the mapping, or connect with `psql`. Deny those paths, see below.
- **Backups.** The mapping stores real values encrypted with a key derived from the secret, but both sit in the same directory; a backup (Time Machine, for example) that copies the directory copies everything needed to decrypt it.
- **Cut free text.** SQL string functions such as `left(body, 30)` cut free text on real values, so a phone number or an email split at the cut can come back as fragments no detector recognises. Read long values with `cell_offset` instead.
- **Detection misses.** Names absent from the built-in lists, inside long free text and without a civility marker, can pass through. Unusual identifiers without a checksum are not recognised.
- **Enum labels are cleared database-wide.** The labels of every enum of the database are kept in clear in any text column the dictionary does not mark as personal, not only in enum columns: a value equal to one of them, ignoring case and accents, passes in clear even if it is a person's name.
- **A wrong `clear` entry discloses its column.** The column policy is trusted configuration: a column wrongly declared `clear` is returned in clear, apart from the values the structural detectors recognise (emails, phones, IBAN, SIREN, SIRET, NIR, card numbers, IP addresses). Names, addresses and free text in it pass through. Review the shared list like code, and keep the agent's edits to the local file under your own approval.
- **Redefined operators and functions.** An expression is cleared from its source columns only when its operators and functions are on the built-in allowlist, matched by name. A role allowed to create objects can define its own `||` or `lower(...)` on built-in or enum types that PostgreSQL then picks instead of the built-in one; the parser cannot see that, so such an override can return personal data in clear.
- **Computed timestamps.** Computed `date` values are shifted like birth dates, but computed timestamps are kept so that `date_trunc` and time-series analysis stay exact; a birth date explicitly cast to `timestamp` therefore passes in clear. Intervals computed from birth dates (`age()`, date differences) are exact and reveal the real birth date.
- **Every database of the server.** The `database` argument reaches any database the connection's role may connect to, not only the connection's own. This is deliberate; restrict the role's `CONNECT` privileges on the server if some databases must stay out of reach.
- **The connection file is trusted configuration.** Like `ProxyCommand` in `~/.ssh/config`, it can run a program (`password_command`) and decide where credentials go (`host`, `ssh_alias`, and the Azure token for an `azure` entry). It holds no secret, but a file kept in a shared repository gives its committers command execution on your machine: review its changes like code. The agent must not be able to edit it, see the deny rules below.
- **Unsupported connections.** In DBeaver connections, SSH jump servers, proxies, authentication models other than native and pgpass, and credentials protected by a DBeaver master password or secure storage are listed as unsupported or unavailable; jump servers are supported for file connections. Servers with certificates from a private CA are not supported yet.
- **SSH tunnels.** Connections of the connection file use the SSH tunnels described [above](#ssh-tunnels). Connections that go through DBeaver's SSH tunnel are usable with public key, password or agent authentication. The SSH user and the password or key passphrase are read from DBeaver's saved credentials; a key without a saved passphrase is accepted and fails at connect time if it is encrypted. A key file that does not exist marks the connection unavailable. For them, the SSH host key is checked against `~/.ssh/known_hosts` (or `SCARIF_SSH_KNOWN_HOSTS`), plain or hashed, with the `[host]:port` form for ports other than 22; an unknown, changed or `@revoked` key refuses the connection with the fingerprint to check, as does a key whose type differs from the one already recorded for the host. `@cert-authority` lines and host certificates are not supported. Add a bastion once with `ssh -p <port> <user>@<host>` or, after checking the fingerprint with its administrator, `ssh-keyscan -p <port> <host> >> ~/.ssh/known_hosts`. Verification is skipped only when "bypass host verification" is checked in DBeaver; `list_connections` then shows `"ssh_host_key_check": "bypassed"` for that connection and each connect logs a warning. SSH jump servers are not supported for DBeaver connections. With pgpass authentication, the database password comes from `PGPASSFILE` or `~/.pgpass` (`%APPDATA%\postgresql\pgpass.conf` on Windows), matched on the database host as written in DBeaver, not the tunnel host. As with libpq, a pgpass file readable by group or others is ignored and the connection is unavailable until it is `chmod 600`.

### Claude Code deny rules

Add these to your Claude Code settings (`~/.claude/settings.json`) so the agent's own tools do not read the secret, the mapping or your DBeaver credentials, edit the connection file, or bypass the server with a database client. Adjust the paths to your OS, and add your `SCARIF_DATA_DIR` and `SCARIF_CONNECTIONS_FILE` paths if you set them. The DBeaver read denies stay recommended as long as the DBeaver source is on. The same rules are in [docs/claude-code-deny-rules.json](docs/claude-code-deny-rules.json).

These rules prevent accidental access; they are not a security boundary, since a shell can reach the same files in other ways. For real isolation, run the agent in a sandbox that cannot read those paths (for example the Claude Code sandbox) or under a separate OS user.

```json
{
  "permissions": {
    "deny": [
      "Read(~/Library/Application Support/scarif-database-mcp/**)",
      "Read(~/.local/share/scarif-database-mcp/**)",
      "Read(~/Library/Application Support/pg-pseudo-mcp/**)",
      "Read(~/.local/share/pg-pseudo-mcp/**)",
      "Read(~/Library/DBeaverData/**)",
      "Read(~/.local/share/DBeaverData/**)",
      "Edit(~/.config/scarif/**)",
      "Edit(~/.config/pg-pseudo/**)",
      "Bash(psql:*)",
      "Bash(pg_dump:*)",
      "Bash(sqlite3:*)"
    ]
  }
}
```

### Resetting pseudonyms

```bash
scarif-database-mcp mapping purge
```

Deletes the secret and the mapping after confirmation, in the data directory actually in use (see [Renamed from pg-pseudo-mcp](#renamed-from-pg-pseudo-mcp)). Pseudonyms the agent saw before become unknown and are reported as `unresolved_pseudonym`.

## Renamed from pg-pseudo-mcp

This project was called `pg-pseudo-mcp` up to 0.2.0. From 0.3.0 the crate, the binary and the release archives are named `scarif-database-mcp`, and the MCP server announces itself as `scarif`. The old names still work for this version, with a warning, and will be removed later.

| | Before (0.2.x) | From 0.3.0 |
|---|---|---|
| Binary and archives | `pg-pseudo-mcp`, `pg-pseudo-mcp-<target>.tar.gz` | `scarif-database-mcp`, `scarif-database-mcp-<target>.tar.gz` |
| MCP server name | `pg-pseudo-mcp` | `scarif` |
| Environment variables | `PG_PSEUDO_CONNECTIONS_FILE`, `PG_PSEUDO_COLUMNS_FILE`, `PG_PSEUDO_SSH_CONFIG`, `PG_PSEUDO_SSH_KNOWN_HOSTS`, `PG_PSEUDO_DBEAVER_HOME`, `PG_PSEUDO_DATA_DIR` | the same names with the `SCARIF_` prefix |
| Config directory | `~/.config/pg-pseudo/` | `~/.config/scarif/` (or `$XDG_CONFIG_HOME/scarif/`) |
| Data directory | `~/Library/Application Support/pg-pseudo-mcp` (macOS), `~/.local/share/pg-pseudo-mcp` (Linux) | `.../scarif-database-mcp` in the same parent |

Fallbacks, none of which copies or moves anything:

- **Variables**: an old `PG_PSEUDO_*` variable is used only when its `SCARIF_*` counterpart is unset, and the server writes one deprecation warning per old variable to stderr (never its value).
- **Config directory**: `connections.toml` and `columns.toml` are each read from `~/.config/scarif/`; when a file is absent there but present in `~/.config/pg-pseudo/`, the old one is read and a `config_location_deprecated` warning appears on stderr and in `server_info`. `connections import-dbeaver` always writes to the new directory.
- **Data directory**: it holds the pseudonym secret, so losing it changes every pseudonym. When the new directory does not exist and the old one does, the old one keeps being used, reported by a `data_directory_deprecated` warning on stderr and in `server_info`. `mapping purge` acts on the directory in use. Once the new directory exists, it wins.

To finish the migration by hand, with the server stopped (macOS shown; on Linux use `~/.local/share`):

```bash
mv ~/Library/Application\ Support/pg-pseudo-mcp ~/Library/Application\ Support/scarif-database-mcp
```

Then move `~/.config/pg-pseudo` to `~/.config/scarif`, rename the variables in your MCP registration (`claude mcp add scarif -- scarif-database-mcp`), and update your deny rules (the old and new paths are both in [docs/claude-code-deny-rules.json](docs/claude-code-deny-rules.json)).

## Development

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

### Contexts and business scenarios

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

## Licence

Apache License 2.0, see `LICENSE`.
