# Revision Records

This directory holds durable **policy revision records**: append-only notes that
explain *why a governed source-exception entry changed* (selector, scope,
evidence, ownership, lifecycle, or capacity), as distinct from `reason`, which
explains *why the exception exists*.

The contract is defined in
[CARGO-ALLOW-ADR-0002](../../docs/adr/CARGO-ALLOW-ADR-0002-policy-revision-contract.md)
and sequenced by
[CARGO-ALLOW-SPEC-0008](../../docs/specs/CARGO-ALLOW-SPEC-0008-ledger-coherence-change-control.md).

## When a record is required

A revision record is required only for **weakening** edits — those an `allow-diff`
run classifies as `posture_delta = worsened` (e.g. `scope_broadened`,
`occurrence_limit_loosened`, `expiry_extended`, `evidence_removed`,
`owner_removed`). Improvements (`scope_narrowed`, `evidence_added`, `owner_added`,
`occurrence_limit_tightened`, `removed_allow`) and neutral text edits never
require a record. Ambiguous `review_required` edits (`scope_changed`,
`selector_changed`, `owner_changed`) need either a record or an explicit reviewer
acknowledgement (surface defined in PR 4). The note rule is the matched row's
`posture_delta`; see ADR-0002 for how it maps onto the
[policy-weakening](../../docs/policy-weakening.md) signal bands.

## Record format

One TOML file per record, named `CARGO-ALLOW-REV-NNNN-<slug>.toml`:

```toml
schema_version = "1.0"
id = "CARGO-ALLOW-REV-0001"
created = "2026-06-20"
owner = "repo-infra"
reason = "Short, durable explanation of the weakening."

allow_ids = ["allow-0042"]
change_kinds = ["occurrence_limit_loosened"]

# Optional
before_fingerprint = "sha256:…"
after_fingerprint = "sha256:…"
review_after = "2026-09-01"
supersedes = "CARGO-ALLOW-REV-0000"
links = ["issue:1475", "pr:1772"]
```

| Field | Required | Purpose |
| --- | --- | --- |
| `schema_version` | yes | Record schema version; currently `"1.0"`. |
| `id` | yes | `CARGO-ALLOW-REV-NNNN`, sequential and unique. |
| `created` | yes | ISO date the record was authored. |
| `owner` | yes | Accountable team or person. |
| `reason` | yes | Why the weakening was made. |
| `allow_ids` | yes | Stable allow IDs the record authorizes (non-empty). |
| `change_kinds` | yes | Authorized change kinds from the canonical `policy_change_kind` vocabulary (non-empty). |
| `before_fingerprint` / `after_fingerprint` | no | Bind a record to one transition for repeatable kinds. |
| `review_after` | no | Advisory revisit date; does not invalidate the record. |
| `supersedes` | no | Prior record this one corrects (append-only chain). |
| `links` | no | External references (`issue:NNN`, `pr:NNN`). |

## Governance posture

- **Append-only and immutable.** Correct a mistake by writing a new record with
  `supersedes`; never edit or delete a record in place.
- **Durable.** Records do not expire after merge; they are a permanent audit
  trail keyed to stable `allow_id` + `change_kinds`.
- **Matched by identity.** Enforcement (PR 4) matches records to worsened diff
  rows by `allow_id` and change kind, not by file line.

## Claim boundary

This directory and ADR-0002 are the **design** of the revision contract (PR 3).
cargo-allow does not yet parse, match, or enforce these records; that is PR 4
(`diff --require-change-note`). The `spec-system` profile validates the ADR's
link graph statically and does not read records here.

The example under `examples/` illustrates the schema and is not consumed by any
runtime path until the PR 4 parser lands.
