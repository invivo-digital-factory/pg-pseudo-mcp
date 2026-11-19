---
title: Leak canaries must be absent from the pseudonym generator's name pools
date: 2026-11-19
category: testing
module: anonymization tests
problem_type: best_practice
component: testing_framework
severity: high
applies_when:
  - "adding or renaming a canary value in tests/sql/seed.sql or tests/common/mod.rs"
  - "changing the name lists used for detection or pseudonym generation"
tags: [canary, pseudonymization, gazetteer, flaky-test, leak-detection]
---

# Leak canaries must be absent from the pseudonym generator's name pools

## Context

The leak checks seed fictional personal values and assert that none of them, nor a 6-character fragment of an email local part, appears in any response. The first canaries used common first names (Ilona, Odilon, Maëlys). Those names are in the `fake` crate's FR_FR lists, which serve both as the detection gazetteer and as the pool the generator draws pseudonym names from.

Two failures followed. A generated pseudonym email such as `odilon.braiblepeville12@example.net` occasionally contained a canary name, so the schema test failed at random. Worse, every query that returned a fragment of an email (`split_part(email, '@', 1)`, `left(email, 12)`, `replace(email, '@', ' at ')`) looked safe only because the gazetteer recognised the common first name inside the fragment. Renaming the canaries to invented names made those queries return real local parts in clear: computed columns had no lineage, and value-level detection cannot recognise a rare name.

## Guidance

- Canary names are invented and absent from `NAME_FIRST_NAME` and `NAME_LAST_NAME` of `fake::locales::FR_FR`. `tests/canary_hygiene.rs` enforces it.
- Also check that no 6-character window of a canary word appears in generated pseudonyms: simulate about a million pseudonym emails with the real `Pseudonymizer` before adopting a new canary. `Thaumassin` was replaced by `Hazvekmor` because its windows (`aumass`, `umassi`) appeared about 5 times per 100,000 generated emails.
- A canary that the detectors recognise does not test the miss path. Keep at least one canary that only the column dictionary or lineage can protect, such as a rare name in a column the dictionary does not match (`orders.titulaire`).

## Why This Matters

A canary that detection already knows makes leak tests pass for the wrong reason. Here it hid a real leak through computed columns until the canaries changed, after which the derived-column lineage in `src/guard/lineage.rs` was added.

## When to Apply

- Any change to the canary list or the seed data.
- Any change to the name lists used by `src/anon/gazetteer.rs` or `src/pseudo/generators.rs`.

## Examples

Before: `Ilona Kervadec` as a canary; `SELECT split_part(email, '@', 1)` returned a pseudonymized value because `Ilona` triggered name detection. After: `Ylwenna Kervadec`; the same query returned `ylwenna.kervadec` in clear until lineage made it an opaque token.

## Related

- `tests/canary_hygiene.rs`
- `tests/adversarial.rs`
