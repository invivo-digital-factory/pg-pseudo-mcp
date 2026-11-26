# Security

What the server guarantees, what it does not, and the Claude Code deny rules that back it up.

## Read-only guarantee

Every statement must be a single `SELECT`, `WITH ... SELECT`, `VALUES` or `EXPLAIN` without `ANALYZE`. Writable CTEs, `SELECT INTO`, row locks, session changes, server functions (`pg_terminate_backend`, `set_config`, `nextval`, `pg_sleep`, `dblink`, large objects, file access...) and catalog relations other than structural ones are refused with the rule that blocked them. Each call runs in a `READ ONLY` transaction with a statement timeout, then rolls back. On a connection of type `prod`, a request estimated heavy is refused until the developer approves it, see [Production connections](connections.md#production-connections); the estimate is a safeguard against load, not a security boundary. For defence in depth, point DBeaver at a role that can only `SELECT`. MongoDB requests go through their own [read-only guard](mongodb.md#read-only-guard).

## Threat model and known limits

scarif-database-mcp protects against **accidental disclosure by a cooperative agent**. It is not a security boundary against an agent that tries to recover real values on purpose:

- **Character-level probing.** SQL predicates run on real values, so `WHERE email LIKE 'a%'`, `ascii(substr(email, 1, 1))` or counts can rebuild a value one character at a time. Text read from the database can also carry instructions aimed at the agent (prompt injection).
- **Same-account file access.** The agent runs as your user. With its own shell or file tools it can read DBeaver's credentials (DBeaver's encryption key is public), the pseudonym secret and the mapping, or connect with `psql` or `mongosh`. Deny those paths and clients, see below.
- **Backups.** The mapping stores real values encrypted with a key derived from the secret, but both sit in the same directory; a backup (Time Machine, for example) that copies the directory copies everything needed to decrypt it.
- **Cut free text.** SQL string functions such as `left(body, 30)` cut free text on real values, so a phone number or an email split at the cut can come back as fragments no detector recognises. Read long values with `cell_offset` instead.
- **Detection misses.** Names absent from the built-in lists, inside long free text and without a civility marker, can pass through. Unusual identifiers without a checksum are not recognised. A name written entirely in capitals and absent from the built-in lists (`KERVADEC`) is not taken for a name in a column the dictionary does not know, so that upper-case codes such as `PAID` or `ACTIVE` stay in clear; name columns the dictionary knows (`nom`, `last_name`...) are pseudonymized whatever the case, and the column policy can mark others personal.
- **Enum labels are cleared database-wide.** The labels of every enum of the database are kept in clear in any text column the dictionary does not mark as personal, not only in enum columns: a value equal to one of them, ignoring case and accents, passes in clear even if it is a person's name.
- **A wrong `clear` entry discloses its column.** The column policy is trusted configuration: a column wrongly declared `clear` is returned in clear, apart from the values the structural detectors recognise (emails, phones, IBAN, SIREN, SIRET, NIR, card numbers, IP addresses). Names, addresses and free text in it pass through. Review the shared list like code, and keep the agent's edits to the local file under your own approval.
- **Redefined operators and functions.** An expression is cleared from its source columns only when its operators and functions are on the built-in allowlist, matched by name. A role allowed to create objects can define its own `||` or `lower(...)` on built-in or enum types that PostgreSQL then picks instead of the built-in one; the parser cannot see that, so such an override can return personal data in clear.
- **Computed timestamps.** Computed `date` values are shifted like birth dates, but computed timestamps are kept so that `date_trunc` and time-series analysis stay exact; a birth date explicitly cast to `timestamp` therefore passes in clear. Intervals computed from birth dates (`age()`, date differences) are exact and reveal the real birth date.
- **Every database of the server.** The `database` argument reaches any database the connection's role may connect to, not only the connection's own. This is deliberate; restrict the role's `CONNECT` privileges on the server if some databases must stay out of reach.
- **The connection file is trusted configuration.** Like `ProxyCommand` in `~/.ssh/config`, it can run a program (`password_command`) and decide where credentials go (`host`, `ssh_alias`, and the Azure token for an `azure` entry). It holds no secret, but a file kept in a shared repository gives its committers command execution on your machine: review its changes like code. The agent must not be able to edit it, see the deny rules below.
- **MongoDB writes.** When the MongoDB user holds `readWrite` or more, the [read-only guard](mongodb.md#read-only-guard) is the only protection against writes: an operator it wrongly accepted would run with that role. Ask for a user with the `read` role only wherever you can, so that the guard is defence in depth.
- **Probing MongoDB values.** `$regex`, `$substrCP`, `$strLenCP`, `$indexOfCP`, ranges (`$gt`, `$lt`...), sorts, `$group` keys and counts run on real values, as `LIKE` and `ORDER BY` do in SQL, so a regular expression, its options included, or a range can rebuild a value one character at a time.
- **Arithmetic on personal numbers.** A computed integer derived from a personal number (`{"$add": ["$telephone", 1]}`) is pseudonymized only when the result still has a known shape (phone, SIREN, SIRET, card number...), as in SQL; otherwise it is returned as computed.
- **Sampled documents.** `describe_table` and `search_schema` read a random sample of up to 100 documents per collection. Their values never reach the response, but their field names do, judged by the detectors like JSON keys: a personal value used as a key and missed by them passes through.
- **Views and the field policy.** A view is matched by its own name: entries declared on its base collection do not apply to it. Declare them for the view too, or with `*`.
- **Decoded before the byte budget.** `max_bytes` bounds the response, not what is read: up to `max_rows + 1` whole documents are read and decoded in memory before the budget applies.
- **The SOCKS5 front.** It listens on `127.0.0.1` only and requires per-session credentials, but a process running as your user could read those credentials from the server's memory, as it could read the Keychain. A local process that floods the front with connections can also evict the driver's pending handshake (32 pending at most, 2 s each) and make the call fail.
- **Unsupported connections.** In DBeaver connections, SSH jump servers, proxies, authentication models other than native and pgpass, and credentials protected by a DBeaver master password or secure storage are listed as unsupported or unavailable; jump servers are supported for file connections. Servers with certificates from a private CA are not supported yet.
- **SSH tunnels.** Connections of the connection file use the SSH tunnels described [above](connections.md#ssh-tunnels). Connections that go through DBeaver's SSH tunnel are usable with public key, password or agent authentication. The SSH user and the password or key passphrase are read from DBeaver's saved credentials; a key without a saved passphrase is accepted and fails at connect time if it is encrypted. A key file that does not exist marks the connection unavailable. For them, the SSH host key is checked against `~/.ssh/known_hosts` (or `SCARIF_SSH_KNOWN_HOSTS`), plain or hashed, with the `[host]:port` form for ports other than 22; an unknown, changed or `@revoked` key refuses the connection with the fingerprint to check, as does a key whose type differs from the one already recorded for the host. `@cert-authority` lines and host certificates are not supported. Add a bastion once with `ssh -p <port> <user>@<host>` or, after checking the fingerprint with its administrator, `ssh-keyscan -p <port> <host> >> ~/.ssh/known_hosts`. Verification is skipped only when "bypass host verification" is checked in DBeaver; `list_connections` then shows `"ssh_host_key_check": "bypassed"` for that connection and each connect logs a warning. SSH jump servers are not supported for DBeaver connections. With pgpass authentication, the database password comes from `PGPASSFILE` or `~/.pgpass` (`%APPDATA%\postgresql\pgpass.conf` on Windows), matched on the database host as written in DBeaver, not the tunnel host. As with libpq, a pgpass file readable by group or others is ignored and the connection is unavailable until it is `chmod 600`.

## Claude Code deny rules

Add these to your Claude Code settings (`~/.claude/settings.json`) so the agent's own tools do not read the secret, the mapping or your DBeaver credentials, edit the connection file, or bypass the server with a database client. Adjust the paths to your OS, and add your `SCARIF_DATA_DIR` and `SCARIF_CONNECTIONS_FILE` paths if you set them. The DBeaver read denies stay recommended as long as the DBeaver source is on. The same rules are in [claude-code-deny-rules.json](claude-code-deny-rules.json).

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
      "Bash(sqlite3:*)",
      "Bash(mongosh:*)",
      "Bash(mongo:*)",
      "Bash(mongodump:*)",
      "Bash(mongoexport:*)"
    ]
  }
}
```
