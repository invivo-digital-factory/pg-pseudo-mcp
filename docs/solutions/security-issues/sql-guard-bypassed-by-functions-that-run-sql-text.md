---
title: The SQL guard is bypassed by any function that runs SQL text
date: 2026-11-20
category: security-issues
module: querying guard
problem_type: security_issue
component: database
symptoms:
  - "SELECT * FROM crosstab('select pg_terminate_backend(1), 1, 1') AS t(a int, b int) was accepted while SELECT pg_terminate_backend(1) is refused"
root_cause: missing_validation
resolution_type: code_fix
severity: high
tags: [sql-guard, read-only, denylist, tablefunc, crosstab, dblink]
---

# The SQL guard is bypassed by any function that runs SQL text

## Problem

The guard parses the one statement the agent sends and refuses writes and dangerous functions by name. A function that takes SQL as a string and executes it inside the server runs a statement the guard never parsed, so every rule the guard enforces is skipped for that inner statement.

## Symptoms

- `crosstab('select pg_terminate_backend(…)')` from the `tablefunc` extension ran a function the guard refuses when called directly. An automated security review found it after the refactor of PR #3; it had been there since the first version.

## What Didn't Work

- Relying on the `READ ONLY` transaction: it blocks writes, not functions such as `pg_terminate_backend`, `pg_cancel_backend` or `pg_sleep`, so the inner statement still acts on the server.

## Solution

`crosstab` and `connectby` joined `dblink` and the `query_to_xml` family in `DENIED_FUNCTION_PREFIXES` (`src/contexts/querying/domain/constants/sql_policy.rs`). A prefix covers the variants (`crosstab2`, `crosstab3`, `crosstab4`, `crosstab_hash`). The guard tests in `src/contexts/querying/test/guard.rs` assert each one is refused with its name, including when it is called in `FROM`.

## Why This Works

Executing SQL text is a property of the function, not of the statement around it, so the only place the guard can stop it is the function name. The same reasoning made function calls used as table factors (`FROM LATERAL f(…)`) a separate hook of the guard: a check that only walks expressions misses them.

## Prevention

- When a target database gains an extension, list its functions that take a query string (`text` argument executed as SQL, or a relation name read without the catalog checks) and add them to the denylist with a guard test before agents use that database.
- The structural fix is an allowlist of functions, refusing unknown ones; it changes what agents can run on databases that rely on extensions (`unaccent`, PostGIS), so it waits for a decision on which extensions to allow.

## Related Issues

- [[canary-values-outside-generator-pools]] (same leak-test discipline: a check only proves what its inputs exercise)
