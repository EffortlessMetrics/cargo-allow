# Policy Revision Records

Append-only records that document *why* a governed exception or policy entry
changed posture. The contract is fixed in
[`CARGO-ALLOW-ADR-0002`](../../docs/adr/CARGO-ALLOW-ADR-0002-policy-revision-contract.md);
the record schema is [`revision.schema.json`](revision.schema.json).

Each `.toml` file directly under this directory holds one record:

```toml
schema_version = "1.0"
id = "CARGO-ALLOW-REV-0001"
created = "2026-06-20"
owner = "repo-infra"
reason = "Raise occurrence limit for the generated module family."

allow_ids = ["allow-0042"]
change_kinds = ["occurrence_limit_loosened"]

# Optional: pins a repeatable weakening to one specific transition.
after_fingerprint = "v1:…"
links = ["issue:123", "pr:456"]
```

Rules (see the ADR for rationale):

- A note is required only for **governed weakening edits** — diff rows whose
  `posture_delta` is `worsened` or `review_required`. Improvements (narrowing
  scope, adding evidence, removing stale entries) never need a note.
- `change_kinds` are canonical `policy_change_kind` tokens (the same vocabulary
  `diff` emits); an unknown token is rejected at parse time. `change_kinds` must
  be explicit and non-empty — blanket waivers are rejected.
- One record may cover several entries: it claims every `(allow_id, change_kind)`
  cell it lists.
- **Repeatable** weakening kinds (`occurrence_limit_loosened`, `expiry_extended`,
  `review_after_extended`, `scope_broadened`, `selector_precision_decreased`)
  must pin the transition with `after_fingerprint`, so a note for one change does
  not authorize the next. One-shot kinds need no fingerprint.
- Records are **durable** and do not expire on merge; optional `expires` is
  advisory only.
- Records are **append-only**: correct a record by adding a new one that
  `supersedes` it, never by editing in place.

## Enforcement

`cargo-allow diff --require-change-note` fails when a weakening edit is not
covered by a matching record here.
`cargo-allow diff --write-change-note-template <path>` writes a starter record
covering a diff's uncovered weakening edits, with the needed transition
fingerprints. Records are parsed and validated by `allow_policy::revision`.
