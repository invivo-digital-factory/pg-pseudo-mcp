# Connections

How scarif-database-mcp finds, authenticates and reaches the databases of a developer.

Contents:

- [Authentication modes](#authentication-modes)
- [SSH tunnels](#ssh-tunnels)
- [Precedence and DBeaver](#precedence-and-dbeaver)
- [Moving off DBeaver](#moving-off-dbeaver)
- [Choosing the database](#choosing-the-database)
- [Azure Database for PostgreSQL](#azure-database-for-postgresql)
- [Environment variables](#environment-variables)


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
| `engine` | `postgresql` (default) or `mongodb`, see [MongoDB](mongodb.md) for the keys of a MongoDB entry |
| `host`, `port` (5432), `database` (`postgres`), `user` | The database, as seen from the SSH host when `ssh_alias` is set; `database` is the connection's default database |
| `sslmode` | `disable`, `allow`, `prefer` (default), `require`, `verify-ca` or `verify-full` |
| `type` | `dev`, `test` or `prod` |
| `auth` | How the password is obtained, see below |
| `ssh_alias` | A `Host` alias of `~/.ssh/config` |
| `read_dbeaver` | Top-level; `false` turns the DBeaver source off entirely (default `true`) |

Edits to the file are picked up at the next tool call. A key that holds a secret (`password`, `token`, `passphrase`, any key containing `password` except `password_command`), an unknown key or an invalid value makes that entry unavailable, with a reason that names the field; two entries with the same name are both unavailable. A file that exists but cannot be parsed or read, or that has an invalid `read_dbeaver` value, a `connection` key that is not an array of tables (`connection = "x"`, `connection = [1]`) or an unknown or secret-shaped top-level key, is reported in the `list_connections` hint (with its line and column for a syntax error, never its content), and the DBeaver connections stay listed but unavailable, with a reason naming the connection file, until it is fixed: a broken file cannot silently re-expose connections it was meant to shadow or turn off. A well-formed `[[connection]]` array with one invalid entry is different: only that entry is unavailable, with its reason, and DBeaver follows `read_dbeaver`. With a readable `read_dbeaver = false`, DBeaver stays off. A missing default file is silent; a missing file named by `SCARIF_CONNECTIONS_FILE` is reported with its path.

## Authentication modes

- `pgpass`: the password comes from `PGPASSFILE` or `~/.pgpass`, matched on the database host as written in the file (not the SSH host), with the usual permission rule (`chmod 600`).
- `azure`: the password is the Entra ID access token from the Azure CLI, see [Azure Database for PostgreSQL](#azure-database-for-postgresql). The mode is declared, not guessed from the host.
- `command`: the password is the standard output of `password_command`, a list made of the program then its arguments, run without a shell. It runs only when a session opens, never when connections are listed, so listing never opens a Keychain prompt. The output is kept in memory only (one trailing line ending is removed), reused by the sessions of the same connection, and the command is run once more after an authentication failure. A failing command gives a fixed reason (not found, could not start, exit status, timeout after 30 s, no output, not UTF-8) and never the command's output.

No password, token or command output appears in a tool response, a reason or a log.

## SSH tunnels

`ssh_alias` names a `Host` of `~/.ssh/config`. The server runs `ssh -G <alias>` and takes the host name, port, user, identity files, identity agent and `ProxyJump` chain from OpenSSH's own resolution, so `Include`, `Match`, wildcards and defaults behave as they do for `ssh`. The tunnel itself runs in the server, so every hop's host key is verified in the same way.

- **Jump hosts.** `ProxyJump` chains of one or more hops work, including `user@host:port` and `ssh://` forms; a first hop with its own `ProxyJump` is followed, up to eight levels.
- **Host keys are always verified**, on every hop, against the `known_hosts` files that `ssh -G` lists (`UserKnownHostsFile` and `GlobalKnownHostsFile`, `HostKeyAlias` honored) or against `SCARIF_SSH_KNOWN_HOSTS` when set. `StrictHostKeyChecking no` in `~/.ssh/config` is ignored on purpose, so it cannot turn verification off for the agent. An unknown, changed or revoked key refuses the connection with the hop's position and alias and the key's fingerprint. Host certificates and `@cert-authority` lines are not supported.
- **Authentication never prompts.** Keys come from the SSH agent (`IdentityAgent` honored; `IdentitiesOnly` filters the agent's keys by your `IdentityFile` entries) or from unencrypted identity files. An encrypted key without an agent fails at connect time and tells you to load it with `ssh-add`. Password authentication to a bastion is not supported for file connections.
- **Unsupported directives.** If the resolved alias sets `ProxyCommand`, `ProxyUseFdpass`, `KnownHostsCommand`, `CertificateFile`, `PKCS11Provider`, `CanonicalizeHostname`, `BindAddress` or `BindInterface`, the connection is listed as unsupported and names the directive; it is never connected with the directive silently dropped. An `IdentityFile` with a token other than `%d %h %p %r %u %%` is unsupported too.
- **The alias must be defined.** `ssh -G` resolves any name with OpenSSH defaults (port 22, your login, default identity files), so a typo or a missing `Host` block would look available and then time out at query time. The server therefore also reads the `Host` lines of `~/.ssh/config` (or of `SCARIF_SSH_CONFIG`), following `Include` up to 16 levels, and requires the connection's own `ssh_alias` to match a `Host` pattern other than a bare `*`; wildcards (`*`, `?`) and negations (`!pattern`) are evaluated as OpenSSH does. Only that alias is checked: a `ProxyJump` hop may be a literal host name or address with no `Host` block. Otherwise the connection is listed unavailable with the reason `SSH alias <alias> is not defined in the OpenSSH config; add a Host block for it (connections import-dbeaver prints one)`, which names only your own alias, and `ssh` is not run. `Match` blocks are not evaluated: they neither define an alias nor cause a refusal on their own. System-wide `/etc/ssh` files are not read. `ssh -G` still gives the resolved values.
- **No `ssh` on `PATH`.** Connections with an `ssh_alias` are unavailable with a reason saying so; the others are unaffected.

`SCARIF_SSH_CONFIG` (absolute path) makes the server pass `-F <path>` to every `ssh -G`, which is how the tests stay independent of your own `~/.ssh/config`.

## Precedence and DBeaver

The server lists the connections of the file and, unless `read_dbeaver = false`, those of the DBeaver workspace. A DBeaver connection whose name is defined in the file (even if that entry is unavailable) is not listed and cannot be used, so the file wins. `list_connections` shows the `source` of each connection (`file` or `dbeaver`). A missing DBeaver workspace is not an error. When neither source yields a connection, the hint tells you to create the file or run the import.

## Moving off DBeaver

```bash
scarif-database-mcp connections import-dbeaver [--force]
```

writes the file once from DBeaver's PostgreSQL connections, without any password, token or passphrase. It refuses to overwrite an existing file unless `--force` is given. It carries name, host, port, database, user, `sslmode` and type (a type other than dev, test or prod is imported as `prod`, the most cautious one), maps native authentication to `pgpass` and pgpass authentication on an Azure host to `azure`, applies the same name suffixes as `list_connections` for duplicate names, and keeps the host and port a DBeaver tunnel points at. For each SSH tunnel it reuses an alias of your `~/.ssh/config` that already resolves to the same host, port and user, and otherwise prints a suggested `Host` block (host, port, user, identity file); it never edits `~/.ssh/config`. It ends with the skipped connections and their reason (other engines, jump servers, unsupported authentication) and, per connection, what is left to do: a pgpass entry or a `password_command`, the `Host` block, the `known_hosts` entries. Once the file exists it takes precedence over DBeaver, so add the suggested `Host` blocks before using the servers (a connection whose alias has no `Host` block is unavailable), or keep the file aside until then; the report says so at its top. Then check each connection shows `source: file` and `available` in `list_connections`, and set `read_dbeaver = false` when you no longer need DBeaver.

## Choosing the database

A connection points at one database, often `postgres`, while the application databases live on the same server. `query`, `explain`, `search_schema`, `list_tables` and `describe_table` accept an optional `database` argument; its names come from `list_databases`. It keeps the host, port, SSH tunnel, credentials, Azure token handling and `sslmode` of the connection and only changes the database name, including the one matched in `~/.pgpass`. Sessions are kept per connection and database: the default database plus at most four others per connection, the least recently used one being closed with its tunnel. Without the argument the connection's own database is used. The read-only guard and the pseudonymization are the same for every database. Database names are returned as they are, since they are the value of the argument. A database that does not exist is reported with the server message and SQLSTATE `3D000`.

## Azure Database for PostgreSQL

For a connection declared with `auth = "azure"` in the connection file, or that uses DBeaver's pgpass authentication model, and whose host ends with `.postgres.database.azure.com` (private link hosts included), the server gets the password itself from the Azure CLI instead of `~/.pgpass`: it runs `az account get-access-token --resource-type oss-rdbms --output json` (no shell, `az` found on `PATH`) and uses the Entra ID access token as the password.

- The token is kept in memory only, reused until about five minutes before it expires, then fetched again. A connection refused for authentication (SQLSTATE `28P01` or `28000`) is retried once with a freshly fetched token, and reconnections of an idle session pick up the current token.
- If `az` is missing, not signed in or fails, the matching `~/.pgpass` entry is used when there is one, as before. Otherwise `list_connections` shows the connection as unavailable with a reason that tells you to run `az login`. The reason is a fixed message per failure (CLI missing, sign-in required, unreadable output); it never carries the CLI output, which can name the signed-in account, nor a token.
- A file connection declared `auth = "azure"` on any other host is unavailable and never runs `az`: the Entra token is valid against every Azure PostgreSQL server you can reach, so it is only ever sent to a host ending in `.postgres.database.azure.com`. A custom DNS name for an Azure server is therefore refused in `azure` mode.
- Other hosts and other authentication models are unchanged.

The script that wrote an access token into `~/.pgpass` before each session is no longer needed for the MCP server; DBeaver itself still reads `~/.pgpass`.

## Environment variables

| Environment variable | Purpose |
|---|---|
| `SCARIF_CONNECTIONS_FILE` | Absolute path of the connection file, instead of `~/.config/scarif/connections.toml` |
| `SCARIF_COLUMNS_FILE` | Absolute path of the shared [column policy](pseudonymization.md#column-policy); the local one is `~/.config/scarif/columns.toml` |
| `SCARIF_SSH_CONFIG` | Absolute path of an SSH config passed to `ssh -G` with `-F`, instead of `~/.ssh/config` |
| `SCARIF_SSH_KNOWN_HOSTS` | Absolute path of a `known_hosts` file used for every SSH hop instead of the files `ssh -G` lists |
| `SCARIF_DBEAVER_HOME` | DBeaver data root, when it is not in the default place |
| `SCARIF_DATA_DIR` | Where the pseudonym secret and mapping live (default: the OS data directory, e.g. `~/Library/Application Support/scarif-database-mcp`) |
| `RUST_LOG` | Log level; logs go to stderr and never contain values, SQL or credentials |
