# Pseudonymization

How personal values are detected and replaced, how to correct the heuristics with a policy, and how to reset the pseudonyms.

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

The heuristics make mistakes in both directions: the dictionary maps the bare token `name` to a last name, so `product_name` or `table_name` are pseudonymized, and a text column of codes can look like names. A column policy fixes this by configuration. MongoDB fields go in a `[mongodb]` section of the same files, see [Field policy](#field-policy).

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

## Field policy

The [column policy](#column-policy) files hold MongoDB fields in a `[mongodb]` section, read from the same files in the same order:

```toml
[mongodb.clear]
"shop.orders" = ["status", "items.label"]
"*.products" = ["name", "select_options.*.label"]

[mongodb.personal."shop.customers"]
"contact.mobile" = "phone"
```

- **Keys** are `"<database>.<collection>"`, split at the first dot since collection names may contain dots: `"shop.orders.archive"` is the collection `orders.archive` of `shop`. `*` stands for any database or any collection. Database and collection names are case-sensitive.
- **Values** are field paths in dot notation without array indices, matched exactly (a `clear` on `contact` does not cover `contact.email`) and without regard to case (a `clear` on `name` also covers `NAME`). Object keys that contain a dot are never matched; their values are judged derived.
- **Wildcards**: a path segment written `*` matches exactly one segment of any name, for documents that use data as keys: `select_options.*.label` covers `select_options.red.label` and `select_options.A12.label`, not `select_options.label`, `select_options.red.fr.label` nor anything below `select_options.red.label`. A `*` mixed with other characters (`opt*`, `*_id`) and `**` are invalid. A `*` never matches the document root, so `["*"]` covers the top-level fields only. A `clear` that lands on an object, exact or wildcard, scans that object's own keys with the structural detectors only, while each value below keeps its own path's judgment.
- **Precedence**: `database.collection`, then `*.collection`, then `database.*`, then `*.*`. Under the same key level, the path with the most named segments wins, so an exact path beats a wildcard path; when a `clear` and a `personal` path match with as many named segments, `personal` wins. A later file only replaces the same key and path, so a local `clear` does not override a shared `personal` matched more precisely. `clear`, `personal`, the kinds, the safety net and the invalid-file rule work as for PostgreSQL: a path both `clear` and `personal` for the same key, an unknown kind or a malformed `[mongodb]` section, invalid wildcards included, makes the whole file ignored with `column_policy_ignored`.
- **Hints** read `{ "collection": "shop.products", "field": "name", "reason": "dictionary" | "default", "hint": "classify it in the column policy: clear or personal" }` under `column_policy_hints`, with the PostgreSQL rules; `describe_table` shows them and `"policy": "clear" | "personal"` for classified fields.

## Resetting pseudonyms

```bash
scarif-database-mcp mapping purge
```

Deletes the secret and the mapping after confirmation, in the data directory actually in use (see [Renamed from pg-pseudo-mcp](migration-from-pg-pseudo.md#renamed-from-pg-pseudo-mcp)). Pseudonyms the agent saw before become unknown and are reported as `unresolved_pseudonym`.
