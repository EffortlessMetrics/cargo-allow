---
id: CARGO-ALLOW-ADR-0002
kind: adr
status: accepted
owner: repo-infra
created: 2026-06-20
linked_proposal: CARGO-ALLOW-PROP-0008
linked_spec: CARGO-ALLOW-SPEC-0008
supersedes: none
superseded_by: none
support_tier_impact: advisory
policy_impact:
  - .allow/artifacts/doc-artifacts.toml
---

# ADR: Policy Revision Contract for `.allow/revisions/`

## Context

`CARGO-ALLOW-SPEC-0008` calls for durable policy revision records that explain
*why* a governed exception or policy entry changed posture. The spec sketches a
record shape under `.allow/revisions/` and defers five decisions to this design
slice (PR 3 of `plans/ledger-coherence/implementation-plan.md`):

- which changes require a note;
- how one note covers multiple entries;
- how notes are matched to a diff;
- whether notes expire after merge;
- whether notes are append-only.

PR 1 (`CARGO-ALLOW-CLOSEOUT-0020`) landed the canonical posture vocabulary in
`allow-core` (`PresenceMovement`, `PostureDelta`, `LedgerPosture`,
`NetPosture`). PR 2 (`CARGO-ALLOW-CLOSEOUT-0021`) made every diff row carry an
orthogonal `movement` and `posture_delta`, with each policy change already
classified to a `PolicyChangeSeverity` of `improvement`, `review`, or `fail`
(`allow-diff` `PolicyChangeKind` / `PolicyChangeSeverity`). The revision contract
must reuse that vocabulary rather than invent a parallel severity model.

This ADR records the contract. It is a design slice: it fixes the record shape
and coverage semantics and lands a parse/validate stub in `allow-policy`, but it
does **not** enforce notes on any command. Enforcement
(`diff --require-change-note`) is the next slice.

## Decision

Adopt an **append-only revision ledger keyed on stable `allow_id` and canonical
`change_kind`, scoped to governed weakening edits, with structural diff
matching and no merge-time expiry.**

### Record shape

Each `.allow/revisions/*.toml` file holds one record:

```toml
schema_version = "1.0"
id = "CARGO-ALLOW-REV-0001"
created = "2026-06-20"
owner = "repo-infra"
reason = "Narrow selector after parser refactor."

allow_ids = ["allow-0042"]
change_kinds = ["selector_changed"]

links = ["issue:123", "pr:456"]
# optional: expires = "never", supersedes = "...", superseded_by = "..."
```

`allow-policy` owns the parse and validate contract
(`allow_policy::revision`); the JSON schema lives at
`.allow/revisions/revision.schema.json`.

### 1. Which changes require a note

A note is required only for **governed weakening edits** — diff rows whose net
`posture_delta` is `worsened` (severity `fail`) or `review_required` (severity
`review`). This reuses the PR 2 classification directly; no new severity model
is introduced.

Obvious improvements are exempt and must never require a note: narrowing scope,
adding typed evidence, assigning an owner, reducing `occurrence_limit`, removing
stale entries, and removing an allow entirely (a `resolved` movement). These map
to `posture_delta = improved` (or `unchanged`).

The canonical weakening vocabulary is `allow-diff`'s `PolicyChangeKind`, e.g.
`selector_changed`, `scope_broadened`, `selector_precision_decreased`,
`policy_status_weakened`, `requirement_loosened`, `occurrence_limit_loosened`,
`expiry_extended`.

### 2. How one note covers multiple entries

A single record lists `allow_ids = [...]` with one or more stable IDs and
`change_kinds = [...]` with one or more canonical tokens. Coverage is the
cartesian product: the record claims every `(allow_id, change_kind)` cell it
lists. This lets one record justify a coordinated change across several entries
(for example a parser refactor that broadens scope on a family of allows)
without duplicating prose per entry.

### 3. How notes are matched to a diff

Matching is **structural, not positional**. A weakening diff row is *covered*
when some record `covers(allow_id, change_kind)` for the row's stable
`allow_id` (the PR 2 diff field) and observed `change_kind`. Multiple records
may jointly cover one diff; the diff is fully covered when **every** weakening
cell is claimed by at least one record.

`change_kinds` must be explicit and non-empty: a record with no change kinds
would be a blanket waiver and is rejected at parse time. Records whose
`allow_ids` or `change_kinds` are not observed in a given diff are simply
inert for that diff — they neither cover anything nor error.

### 4. Whether notes expire after merge

Records are **durable** and do not auto-expire on merge. A weakening becomes
part of the baseline once merged; the record remains as permanent provenance for
why the baseline looks the way it does. An optional `expires` field
(`"never"` or an ISO date) bounds *advisory* freshness only — it never silently
revokes a recorded justification.

### 5. Whether notes are append-only

Yes. `.allow/revisions/` is **append-only**. Records carry an immutable stable
`id`; a correction is a new record that `supersedes` the prior one, mirroring
the ADR `supersedes` / `superseded_by` chain. Editing or deleting a committed
record is a governance smell. `validate_revision_ledger` rejects duplicate IDs
as the cheap, mechanical guard against rewritten or copied records; deeper
immutability proof (git history) is out of scope for this slice.

## Alternatives Considered

| Alternative | Tradeoff |
| --- | --- |
| Per-entry note files keyed by path | Breaks under rename/selector drift; stable `allow_id` is the durable key already surfaced by PR 2 diff rows. |
| Note severity model independent of diff | Duplicates `PolicyChangeSeverity`; risks divergent classification between `diff` and revision enforcement. |
| Notes expire at merge | Erases the provenance trail exactly when the weakening becomes durable baseline. |
| Mutable records edited in place | Launders history; defeats the audit purpose of the ledger. |
| Allow blanket waivers (empty `change_kinds`) | Reintroduces silent approval the spec forbids; rejected at parse time. |

## Consequences

### Positive

- Enforcement (PR 4) reads `posture_delta` and `change_kind` already produced by
  PR 2 — no new classification surface.
- One record can justify a coordinated multi-entry change without prose
  duplication.
- The ledger is a durable, auditable provenance trail of every weakening.

### Negative

- Coordinated weakenings require listing every `(allow_id, change_kind)` cell;
  large refactors produce verbose records.
- Append-only correction via `supersedes` is more ceremony than editing a file.

### Neutral Or Operational

- `allow-policy::revision` parses and validates record shape now; diff-time
  coverage matching is wired in the enforcement slice.
- Canonical `change_kinds` are validated for token shape here; cross-checking
  against the concrete `PolicyChangeKind` set happens on the diff path, where
  `allow-diff` is available (it depends on `allow-policy`, not the reverse).

## Support-Tier Impact

Advisory only. No support-tier promotion until the dogfood slice
(`CARGO-ALLOW-SUPPORT-0001`, plan PR 8) produces change-control evidence.

## Claim Boundary

This ADR fixes the revision-record contract and lands a parse/validate stub. It
does not enforce notes on any command, mutate policy, or authorize release.
