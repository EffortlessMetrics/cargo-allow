---
id: CARGO-ALLOW-ADR-0002
kind: adr
status: accepted
owner: repo-infra
created: 2026-07-03
linked_proposal: CARGO-ALLOW-PROP-0008
linked_spec: CARGO-ALLOW-SPEC-0008
supersedes: none
superseded_by: none
support_tier_impact: advisory
policy_impact:
  - .allow/artifacts/doc-artifacts.toml
  - .allow/revisions/
---

# ADR: Policy Revision Contract for `.allow/revisions/`

## Context

`CARGO-ALLOW-SPEC-0008` calls for durable policy revision records that explain
*why* a governed exception or policy entry changed posture. `reason` explains why
an exception exists; nothing records why its selector, scope, evidence,
ownership, lifecycle, or capacity changed. Issue #1475 asks that the `diff`
classification stop being mere advice and become a gate, mirroring the
goldens-bless enforcement pattern.

PR 1 (`CARGO-ALLOW-CLOSEOUT-0020`) landed the canonical posture vocabulary in
`allow-core` (`PresenceMovement`, `PostureDelta`). PR 2
(`CARGO-ALLOW-CLOSEOUT-0021`) made every diff row carry an orthogonal `movement`
and `posture_delta`, each policy change already classified to a
`PolicyChangeSeverity` of `improvement`, `review`, or `fail`. This contract
reuses that vocabulary rather than inventing a parallel severity model.

This ADR fixes the record contract **and** the runtime enforcement lands with it
(`diff --require-change-note`), consolidating the design (originally split as
PR 3) with the enforcement slice (PR 4) per issue #2075.

## Decision

Adopt an **append-only revision ledger keyed on stable `allow_id` and canonical
`policy_change_kind`, scoped to governed weakening edits, with structural diff
matching, no merge-time expiry, and a transition-fingerprint guard for repeatable
kinds.**

### Record shape

Each `.allow/revisions/*.toml` file holds one record:

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
# Optional: expires = "never", supersedes = "…", superseded_by = "…", links = […]
```

`allow-policy` owns the parse/validate contract (`allow_policy::revision`); the
JSON schema is `.allow/revisions/revision.schema.json`.

### 1. Which changes require a note

A note is required only for **governed weakening edits** — diff rows whose net
`posture_delta` is `worsened` (severity `fail`) or `review_required` (severity
`review`). This reuses the PR 2 classification directly; no new severity model is
introduced. Improvements (`posture_delta = improved`/`unchanged`) never require a
note: narrowing scope, adding evidence, assigning an owner, reducing
`occurrence_limit`, removing stale entries.

### 2. Canonical change kinds (no parallel taxonomy)

`change_kinds` are members of the existing `policy_change_kind` vocabulary
(`allow-diff`'s `PolicyChangeKind`). Because `allow-diff` depends on
`allow-policy` (not the reverse), the canonical token set is published in
`allow-core` (`POLICY_CHANGE_KIND_TOKENS`), and a parity test in `allow-diff`
binds the enum to that list so the two cannot drift. `allow-policy` validates a
note's `change_kinds` against the `allow-core` list — an invented or misspelled
token is rejected at parse time, not merely mis-shaped.

### 3. How one note covers multiple entries, and how notes match a diff

A record lists `allow_ids` and `change_kinds`; coverage is the cartesian product
of the cells it names. Matching is **structural, not positional**: a weakening
diff row is covered when some record claims its `(allow_id, change_kind)` cell.
Multiple records may jointly cover one diff; the diff is fully covered when every
weakening cell is claimed. `change_kinds` must be explicit and non-empty — a
blanket waiver is rejected at parse time.

### 4. Repeatable-weakening guard (transition fingerprints)

Cell-level `(allow_id, change_kind)` matching is sufficient for one-shot kinds
(`owner_removed`, `evidence_removed`) but not for **repeatable** kinds that can
recur, each recurrence weakening further: `occurrence_limit_loosened`,
`expiry_extended`, `review_after_extended`, `scope_broadened`,
`selector_precision_decreased`. For those, a record must additionally pin the
exact transition via an `after_fingerprint` equal to a deterministic fingerprint
of the head entry's post-edit state. A note that authorized raising a limit from
5 to 10 therefore does **not** silently authorize a later 10-to-20 increase — the
fingerprint changes, and the stale record no longer covers. The same fingerprint
function backs both enforcement and `--write-change-note-template`, so an
operator's recorded fingerprint matches what enforcement recomputes for the
committed head.

### 5. Whether notes expire after merge

Records are **durable** and do not auto-expire on merge. A weakening becomes part
of the baseline once merged; the record remains as permanent provenance. An
optional `expires` field (`"never"` or an ISO date) bounds *advisory* freshness
only — it never silently revokes a recorded justification.

### 6. Whether notes are append-only

Yes. `.allow/revisions/` is append-only. Records carry an immutable stable `id`; a
correction is a new record that `supersedes` the prior one.
`validate_revision_ledger` rejects duplicate IDs as the mechanical guard against
rewritten or copied records.

### Enforcement

`diff --require-change-note` fails the diff when any weakening cell is uncovered,
folding into the existing failure decision alongside worsened-policy and no-new
failures. `diff --write-change-note-template <path>` emits a starter record
covering the uncovered cells, with per-transition fingerprints for repeatable
kinds. Enforcement reads the `posture_delta` and `change_kind` already produced
by PR 2 and consumes records via `RevisionRecord::covers_transition`.

## Alternatives Considered

| Alternative | Tradeoff |
| --- | --- |
| A parallel `change_kind` enum in `allow-policy` | Duplicates the diff vocabulary; risks divergent classification. Rejected in favor of the shared `allow-core` token list + parity test. |
| Cell-only matching, no fingerprints | Opens the repeatable-weakening loophole: one note silently authorizes every future increase. |
| Per-entry note files keyed by path | Breaks under rename/selector drift; stable `allow_id` is the durable key PR 2 already surfaces. |
| Notes expire at merge | Erases the provenance trail exactly when the weakening becomes durable baseline. |
| Allow blanket waivers (empty `change_kinds`) | Reintroduces silent approval the spec forbids; rejected at parse time. |

## Consequences

### Positive

- Enforcement reuses `posture_delta`/`change_kind` from PR 2 — no new
  classification surface.
- One definition of "weakening" is shared by the diff classifier and change
  control (the `allow-core` token list, parity-tested).
- The repeatable-weakening loophole is closed by transition fingerprints.

### Negative

- Coordinated weakenings require listing every `(allow_id, change_kind)` cell;
  repeatable transitions need one record per transition (fingerprint-pinned).
- Append-only correction via `supersedes` is more ceremony than editing a file.

### Neutral Or Operational

- `.allow/revisions/*.toml` are loaded non-recursively; the schema and README
  that share the directory are skipped by the `.toml` filter.
- The transition fingerprint is an opaque `v1:` hash; its exact input may evolve
  under a new version prefix without breaking committed records.

## Support-Tier Impact

Advisory only. No support-tier promotion until the dogfood slice
(`CARGO-ALLOW-SUPPORT-0001`, plan PR 8) produces change-control evidence.

## Claim Boundary

This ADR fixes the revision-record contract and lands its parse/validate and
`diff --require-change-note` enforcement. It does not mutate policy
automatically, prove end-to-end dogfood adoption, or authorize release.
