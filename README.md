# scarif-database-mcp

A local MCP server that lets an AI coding agent investigate your PostgreSQL and MongoDB databases on its own, without any real personal value reaching the model provider through the server.

It reads the connections of its own secret-free file (or the ones you already have in DBeaver), runs only read-only SQL and [MongoDB](docs/mongodb.md) queries, and replaces every personal value in results, errors and query plans by a stable, realistic pseudonym. When the agent reuses a pseudonym in a later query, the server translates it back to the real value locally, so the agent can follow a customer across tables without ever seeing who they are.

## How it works

The agent talks to this MCP server, the server talks to the database, and every personal value is replaced by a pseudonym on the way back. An example exchange:

```text
agent: SELECT email FROM customers WHERE id = 1
server: claire.martin81@example.net            (real: marie.dupont@yourcompany.fr)
agent: SELECT * FROM orders o JOIN customers c ON c.id = o.customer_id
       WHERE c.email = 'claire.martin81@example.net'
server: Marie Dupont's orders, every personal value pseudonymized, translated_literals: 1
```

> [!NOTE]
> A pseudonym is not a real value: never treat it as one and never look it up elsewhere. See [Pseudonymization](docs/pseudonymization.md).

> [!WARNING]
> With a MongoDB user that holds `readWrite` or more, the server's read-only guard is the only protection against writes: ask for a user with the `read` role only. See [Security](docs/security.md).

## Install

Each `v*` tag publishes a release with a single binary per platform (macOS arm64 and x86_64, Linux x86_64 and arm64) and a `SHA256SUMS` file:

```bash
version=0.4.0
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

Describe your connections in [the connection file](docs/connections.md), or generate it from DBeaver with `scarif-database-mcp connections import-dbeaver`. Without a file, the server reads your DBeaver workspace, found automatically (`~/Library/DBeaverData` on macOS, `~/.local/share/DBeaverData` on Linux, `%APPDATA%\DBeaverData` on Windows).

## Quick start

Create `~/.config/scarif/connections.toml`; it holds no secret:

```toml
[[connection]]
name = "shop-test"
host = "db.internal"
database = "shop"
user = "reader"
type = "test"
auth = "pgpass"

[[connection]]
name = "orders-test"
engine = "mongodb"
host = "cluster0.example.mongodb.net"
user = "alice"
type = "test"
auth = "command"
password_command = ["security", "find-generic-password", "-s", "scarif-mongodb-test", "-a", "alice", "-w"]
```

Every key, the authentication modes and SSH tunnels are in [Connections](docs/connections.md).

## Tools

All tools are read-only (`readOnlyHint: true`) and can be allowlisted.

| Tool | What it does |
|---|---|
| `list_connections` | Connections of the connection file and of DBeaver (whose other engines are listed as unsupported), with engine (`postgresql` or `mongodb`), source (`file` or `dbeaver`), status (available, unavailable, unsupported), reason and type (dev, test, prod). Never hosts or credentials. |
| `list_databases` | The databases of the server behind a connection, ordered by name (connectable, not templates), and the connection's default database, the one the connection points at. The fixed query is run by the server itself; `SELECT ... FROM pg_database` sent through `query` stays refused. |
| `search_schema` | Tables and columns whose name matches a pattern. |
| `list_tables` | Tables and views with kind, estimated rows and column count. |
| `describe_table` | Columns, keys in both directions, indexes, estimates, and which columns will be pseudonymized. |
| `query` | One read-only statement; rows with pseudonymized values, the list of pseudonymized columns and why, translated literals, warnings, structured truncation. |
| `explain` | The plan of one statement, without executing it; literals in the plan are pseudonymized. |
| `find` | One `find` on a MongoDB collection; documents with pseudonymized values, the pseudonymized field paths and why, translated literals, warnings, structured truncation. |
| `aggregate` | One read-only aggregation pipeline on a MongoDB collection, with the same result shape as `find`. |
| `server_info` | Live limits, reversible and non-reversible kinds, known limitations, and the MongoDB allowlist. |

`query` and `explain` read PostgreSQL connections only, `find` and `aggregate` MongoDB connections only; each refuses the other engine with a reason naming the right tool. `list_databases`, `search_schema`, `list_tables` and `describe_table` work on both, see [Tools on a MongoDB connection](docs/mongodb.md#tools-on-a-mongodb-connection).

## Documentation

- [Connections](docs/connections.md): connection file, authentication, SSH tunnels, DBeaver, Azure, environment variables.
- [MongoDB](docs/mongodb.md): MongoDB entries, private link, tools, read-only guard, documents and lineage.
- [Pseudonymization](docs/pseudonymization.md): how values are pseudonymized, column and field policy, resetting pseudonyms.
- [Security](docs/security.md): read-only guarantee, threat model and limits, Claude Code deny rules.
- [Development](docs/development.md): tests, contexts and business scenarios, third-party data.
- [Migration from pg-pseudo-mcp](docs/migration-from-pg-pseudo.md): what changed with the rename.

## Licence

Apache License 2.0, see `LICENSE`.
