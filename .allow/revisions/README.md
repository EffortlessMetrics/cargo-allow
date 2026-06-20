# `.allow/revisions/` — policy revision notes

This directory holds **policy revision notes**: durable, append-only records
that explain and authorize **governed weakening edits** to the source-exception
ledger (`policy/allow.toml`).

A regular `[[allow]]` entry's `reason` explains *why an exception exists*. A
revision note explains *why a governed entry changed* — why a selector was
broadened, scope widened, occurrence limit raised, evidence weakened,
classification relaxed, lifecycle extended, owner removed, or posture otherwise
weakened.

## Status

**Design only (GOAL-0004 PR 3).** This directory and its contract are defined by
[CARGO-ALLOW-ADR-0002](../../docs/adr/CARGO-ALLOW-ADR-0002-policy-revision-contract.md)
and validated against
[docs/schemas/revision.schema.json](../../docs/schemas/revision.schema.json).
Enforcement (`diff --require-change-note`) lands in GOAL-0004 PR 4. Until then
notes are advisory documentation; cargo-allow does not yet require or parse them.

## Record format

One TOML record per file, named after its ID (for example
`CARGO-ALLOW-REV-0001.toml`):

```toml
schema_version = "1.0"
id = "CARGO-ALLOW-REV-0001"
created = "2026-06-20"
owner = "core/policy"
reason = "Re-broaden selector after a parser refactor changed the AST shape."
status = "accepted"

allow_ids = ["allow-0042"]
change_kinds = ["selector_broadened"]
after_fingerprints = ["fnv1a64:d89cea4b9dc969d2"]

links = ["issue:1475", "pr:456"]
```

See [`example-revision.toml`](example-revision.toml) for a committed example.

## Contract summary

- **Required note:** only for diff rows whose `posture_delta` is `worsened`.
  Improvements (narrowing scope, adding evidence, assigning an owner, reducing
  `occurrence_limit`, bringing review forward, pruning stale entries) need no
  note.
- **Multi-entry coverage:** one note may list several `allow_ids` only when they
  share one logical edit (same `reason` and `links`); `change_kinds` is the
  union across covered entries.
- **Diff matching:** a note matches a `worsened` row when it lists the row's
  `allow_id`, covers the row's governed change kind(s), the row's after-state
  fingerprint is in the note's `after_fingerprints` set (when declared), and the
  note's `status` is `accepted`.
- **Durability:** notes do not expire and are not consumed at merge.
- **Append-only:** never edit a note in place; a correction is a new note that
  `supersedes` the prior one, which is then marked `superseded_by`.

The full rationale and alternatives are in
[CARGO-ALLOW-ADR-0002](../../docs/adr/CARGO-ALLOW-ADR-0002-policy-revision-contract.md).
