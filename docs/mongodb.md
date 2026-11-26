# MongoDB

How to read MongoDB databases through the `find` and `aggregate` tools, and how documents are guarded and pseudonymized.

Contents:

- [Password in the macOS Keychain](#password-in-the-macos-keychain)
- [Atlas private link through an SSH bastion](#atlas-private-link-through-an-ssh-bastion)
- [Read preference](#read-preference)
- [Tools on a MongoDB connection](#tools-on-a-mongodb-connection)
- [Read-only guard](#read-only-guard)
- [Pseudonyms in filters](#pseudonyms-in-filters)
- [How documents are pseudonymized](#how-documents-are-pseudonymized)
- [Derived fields and lineage](#derived-fields-and-lineage)
- [Limits](#limits)
- [Field policy](pseudonymization.md#field-policy)

An entry with `engine = "mongodb"` reaches a MongoDB replica set, a MongoDB Atlas cluster for example, read with the `find` and `aggregate` tools. Documents come back with every personal value pseudonymized per field path, as rows do for PostgreSQL. MongoDB connections come from the connection file only; DBeaver lists its own as unsupported.

```toml
[[connection]]
name = "orders-test"
engine = "mongodb"
host = "cluster0.example.mongodb.net"
database = "shop"
user = "alice"
type = "test"
auth = "command"
password_command = ["security", "find-generic-password", "-s", "scarif-mongodb-test", "-a", "alice", "-w"]
ssh_alias = "corp-bastion"
```

| Key | Meaning |
|---|---|
| `host` | With `srv = true`, the SRV name of the cluster (`cluster0.example.mongodb.net`); with `srv = false`, the host name of one member |
| `srv` | `true` (default): resolve the SRV and TXT records of `host`, as a `mongodb+srv://` URI does; `false`: connect to `host` and `port` |
| `port` | Only with `srv = false` (default 27017) |
| `tls` | `true` (default); `false` only with `srv = false`, for local or test sets |
| `replica_set` | Name of the replica set, optional |
| `read_preference` | Which member serves the reads: `primary`, `primaryPreferred`, `secondary`, `secondaryPreferred` (default) or `nearest`, see [Read preference](#read-preference) |
| `read_preference_tags` | Optional list of `"key:value"` strings, one tag set the member must carry; only with a `read_preference` other than `primary` |
| `database` | Optional default database; without it, the tools need the `database` argument |
| `auth` | Must be `command`; the user authenticates with its password against the `admin` database |
| `name`, `user`, `type`, `ssh_alias` | As for PostgreSQL |

`sslmode` (use `tls`), `auth = "pgpass"` or `"azure"`, a `port` with `srv = true` and `tls = false` with `srv = true` make a MongoDB entry unavailable with a reason that names the key; an unknown `read_preference`, a malformed tag or `read_preference_tags` without a `read_preference` other than `primary` do the same; `srv`, `tls`, `replica_set`, `read_preference` and `read_preference_tags` do the same on a PostgreSQL entry. Listing never runs the password command nor opens the tunnel, so a connection listed `available` can still fail at its first call (wrong password, unreachable cluster).

## Password in the macOS Keychain

The password only ever comes from `password_command`, never from the file or an environment variable the agent's shell could print. Store it once, typing this yourself in a terminal: `-w` given last makes `security` prompt for the password, so it never lands on a command line or in the shell history.

```bash
security add-generic-password -s scarif-mongodb-<env> -a <user> -w
```

Then read it with `password_command = ["security", "find-generic-password", "-s", "scarif-mongodb-<env>", "-a", "<user>", "-w"]`. The command behaves as described in [Authentication modes](connections.md#authentication-modes): it runs when a session opens, its output stays in memory, and it runs once more after an authentication failure.

## Atlas private link through an SSH bastion

The members of a private-link cluster have host names that only resolve inside its network. With `srv = true` and an `ssh_alias`, the server resolves the public SRV and TXT records locally, then opens every member connection through a SOCKS5 front that it runs itself, which forwards each one through the SSH tunnel by host name, so the bastion resolves the private address, as `ssh -L` would. TLS keeps the cluster's host names, so certificates are checked against public roots; the driver discovers every member and fails over between them, and no `/etc/hosts` entry or fixed local port is needed.

- The front listens on `127.0.0.1`, on a random port, for one session, and requires a user name and password generated at random for that session.
- It forwards only to host names under the parent domain of the SRV name (for `cluster0.example.mongodb.net`, any `<member>.example.mongodb.net`), or of `host` with `srv = false`; addresses and other names are refused. An SRV name needs at least three labels.
- The bastion must allow TCP forwarding to the members, as `ssh -L` needs. Host keys and jump hosts work as described in [SSH tunnels](connections.md#ssh-tunnels).
- Without `ssh_alias`, the driver connects to the members directly.
- Driver errors reach the agent without host names, the user name or the front's credentials.

## Read preference

`read_preference` decides which member of the replica set serves `find`, `aggregate`, the sampling of `describe_table` and `search_schema`, and the estimated counts. Without it, reads go to a secondary when one is up and to the primary otherwise (`secondaryPreferred`). On a production cluster, `secondary` keeps every read off the primary, at the cost of failing when no secondary is reachable; `read_preference_tags` narrows the reads to the members that carry every tag, for example the analytics nodes of an Atlas cluster:

```toml
read_preference = "secondary"
read_preference_tags = ["nodeType:ANALYTICS"]
```

Secondaries are reached the same way as the primary: the driver monitors every member the replica set announces, and through an `ssh_alias` each one goes through the same SOCKS5 front and bastion, which admit any member under the parent domain. Listing collections and indexes always runs on the primary, as the MongoDB driver specifies for these commands; they read metadata only.

## Tools on a MongoDB connection

- `list_connections` shows `"engine": "mongodb"`.
- `list_databases` returns the databases the user is authorized on, plus the connection's default database, minus `admin`, `local` and `config`.
- `list_tables` lists the collections and views of a database (`kind` `collection`, `view` or `timeseries`) with their estimated document count; system collections are not listed and the `schema` argument is refused.
- `describe_table` returns the field paths seen in a random sample of up to 100 documents with their BSON types and presence ratio, the indexes (keys only), the estimated count, the fields of a `$jsonSchema` validator, which paths will be pseudonymized and as which kind, the policy classification and the hints. It never returns a sampled value; field names that are themselves personal values (a map keyed by emails) are pseudonymized.
- `search_schema` matches collection names and sampled field paths, in at most 50 collections of the database.
- `find` takes `filter`, `projection`, `sort`, `skip`, `limit`, `hint` and `collation`; `aggregate` takes a `pipeline` and its `let` variables. Both take `connection`, `database`, `collection` and the limits of `query` (`max_rows`, `max_bytes`, `timeout_ms`, `cell_offset`), in Extended JSON.

Documents are returned as relaxed Extended JSON (`{"$oid": ...}`, `{"$date": ...}`, `{"$numberDecimal": ...}`), which the agent can copy back into a filter; `pseudonymized` maps each field path to its kind and reason.

## Read-only guard

Agent requests only ever run as `find` or `aggregate`; the server's own listing, sampling, index and count commands take no agent text. Every stage, query operator and expression operator must be on an allowlist, listed by `server_info`; anything else is refused with its name and position before anything reaches the server.

- **Stages**: `$match`, `$project`, `$addFields`, `$set`, `$unset`, `$group`, `$sort`, `$limit`, `$skip`, `$count`, `$unwind`, `$lookup`, `$graphLookup`, `$unionWith`, `$facet`, `$bucket`, `$bucketAuto`, `$sortByCount`, `$replaceRoot`, `$replaceWith`, `$sample`, `$densify`, `$fill`, `$setWindowFields`, `$documents`, and `$geoNear` as the first stage. Refused, among others: `$out`, `$merge`, `$where`, `$function`, `$accumulator` and any server-side JavaScript, `$currentOp`, `$collStats`, `$indexStats`, `$changeStream`, and the Atlas Search stages `$search`, `$searchMeta` and `$vectorSearch`.
- **Constant arguments**: the target type of `$convert`, the unit, timezone and format of the date operators, and the pattern and options of `$regexMatch`, `$regexFind` and `$regexFindAll` must be constants, not field paths or expressions.
- **Namespaces**: the `admin`, `local` and `config` databases and the `system.*` collections are refused, also as a connection's default database; `$lookup`, `$graphLookup` and `$unionWith` may only name a collection of the same database, as a string.
- **Size**: a request holds at most 64 KB of JSON, 50 stages in all (sub-pipelines of `$lookup`, `$unionWith` and `$facet` included), 32 levels of nesting and 1,000 elements per array.
- **Execution**: `maxTimeMS` is `timeout_ms`, `allowDiskUse` is off, reads follow the connection's [read preference](#read-preference), and at most `max_rows + 1` documents are read: a larger `find` limit is lowered with a `limit_clamped` warning, and a `$limit` is appended to every pipeline.

A refusal names the rule that blocked it and never echoes a value from the request.

## Pseudonyms in filters

A pseudonym of a kind that `server_info` lists as reversible (the same as for PostgreSQL) is translated back to the real value where it is compared with a field: `{field: value}`, `$eq`, `$ne`, `$in`, `$nin`, `$all`, `$elemMatch` and `$not` in a filter (`find`'s filter and every `$match`, sub-pipelines included, `$graphLookup`'s `restrictSearchWithMatch` and `$geoNear`'s `query`), and `$eq`, `$ne` or `$in` inside `$expr` between a field path and a value. A pseudonym written as a JSON number becomes a number again when the real value is all digits. `translated_literals` counts the translations, and the real values are scrubbed from results and errors.

Anywhere else (ranges, `$regex`, `$text`, projections, `$concat`...) a pseudonym stays as written and the result carries `pseudonym_not_translated`; a value shaped like a pseudonym but unknown on this machine gets `unresolved_pseudonym`.

## How documents are pseudonymized

Each value is judged by its field path in dot notation without array indices (`items.customer.email`), with the PostgreSQL precedence: policy `personal`, policy `clear` (with the structural safety net), the dictionary on the last segment (`firstName` and `first_name` alike), the BSON type, then the heuristics. ObjectIds, booleans, null, timestamps and UUIDs stay in clear; numbers too, unless the dictionary or the policy marks the field (a phone stored as a number under `telephone` is pseudonymized); dates are shifted only for birth dates; other binary values and BSON types without a JSON form become `<type len=N>`. Object keys are scanned like JSON keys, so a map keyed by emails gets pseudonymized keys.

## Derived fields and lineage

In `aggregate` results, and for a `find` projection, a field that passes through or is renamed (`{"a": 1}`, `{"b": "$a"}`, `$group` keys, `$first`, `$last`, `$push` and `$addToSet` of a path, `$unwind`, `$lookup` results, `$replaceRoot` on a path) keeps the classification of its source path in its source collection: policy, dictionary on the source name, hints.

A sub-document the query only passes through is not judged as a computed value: the parents of a nested inclusion (`{"a.b.c": 1}`), a sub-document trimmed by an exclusion (`{"a.b": 0}`) or extended by `$set` and `$addFields` on a nested path keep the classification of their source path, and each field below them is judged on its own, as without the projection. A sub-document whose content is computed (`$mergeObjects`, an object built from other paths) stays derived, keys included.

A computed field stays in clear only when every field it reads is declared `clear` in the field policy and all its operators are on the clear-lineage list: `$concat`, `$toLower`, `$toUpper`, `$toString`, `$trim`, `$ltrim`, `$rtrim`, `$strLenCP`, `$strLenBytes`, `$cond`, `$ifNull`, `$switch`, arithmetic, comparisons, `$dateToString`, `$dateTrunc`, date parts, `$sum`, `$count`, `$avg`, `$min` and `$max`. Any other computed field is derived from personal data, reported with the reason `derived`: its text is scanned and pseudonymized or replaced by an opaque token, its integers are pseudonymized when they keep a known shape (phone, SIREN...), its dates are shifted like birth dates. `$$ROOT` and other variables, `$objectToArray` and the operators on field names, computed field names, `$facet` and unknown stages always give a derived field.

## Limits

Same defaults and maxima as `query`: 50 documents, 16 KB, 10 s by default; 500 documents, 64 KB, 60 s at most. Text values are cut at 500 characters (read the rest with `cell_offset`), arrays at 100 items with a truncation naming the path, and the response at `max_bytes`; when documents are cut, the hint gives keyset pagination (`{"_id": {"$gt": last_id}}` sorted by `{"_id": 1}`). `pseudonymized` lists at most 200 paths. Not supported: writes, `explain`, change streams, Atlas Search, authentication other than user and password, servers with a certificate from a private CA; sharded clusters behind `mongos` are untested.
