# Policy Revision Records

Append-only records that document *why* a governed exception or policy entry
changed posture. The contract is fixed in
[`CARGO-ALLOW-ADR-0002`](../../docs/adr/CARGO-ALLOW-ADR-0002-policy-revision-contract.md);
the record schema is [`docs/schemas/revision.schema.json`](../../docs/schemas/revision.schema.json).

Each `.toml` file under this directory holds one record:

```toml
schema_version = "1.0"
id = "CARGO-ALLOW-REV-0001"
created = "2026-06-20"
owner = "repo-infra"
reason = "Narrow selector after parser refactor."

allow_ids = ["allow-0042"]
change_kinds = ["selector_changed"]

links = ["issue:123", "pr:456"]
```

Rules (see the ADR for rationale):

- A note is required only for **governed weakening edits** — diff rows whose
  `posture_delta` is `worsened` or `review_required`. Improvements (narrowing
  scope, adding evidence, removing stale entries) never need a note.
- One record may cover several entries: it claims every
  `(allow_id, change_kind)` cell it lists.
- `change_kinds` must be explicit and non-empty; blanket waivers are rejected.
- Records are **durable** and do not expire on merge; optional `expires` is
  advisory only.
- Records are **append-only**: correct a record by adding a new one that
  `supersedes` it, never by editing in place.

Enforcement (`diff --require-change-note`) is a later slice. Today these records
are parsed and validated by `allow_policy::revision` but not yet required by any
command.
