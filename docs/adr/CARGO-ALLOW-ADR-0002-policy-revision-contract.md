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
  - .allow/revisions/
---

# ADR: Policy Revision Contract for Governed Exception Edits

## Context

`reason` on a source-exception entry explains **why an exception exists**. It does
not explain **why its selector, scope, evidence, ownership, lifecycle, or
capacity changed** after the entry was first accepted. Issue #1475 records this
gap: a reviewer reading a `policy/allow.toml` diff cannot tell an honest
narrowing apart from a silent weakening, and no durable record survives the merge
that broadened an entry.

GOAL-0004 PR 1 introduced the canonical `PostureDelta` vocabulary
(`improved | worsened | review_required | unchanged`) in `allow-core`, and PR 2
made every diff row carry an orthogonal `movement` and `posture_delta`. The
classifier already distinguishes a weakening from an improvement. What is missing
is a **durable, matchable record** that authorizes a weakening, plus a contract
for which edits demand one.

This ADR designs that contract. It is the PR 3 design slice of
[CARGO-ALLOW-SPEC-0008](../specs/CARGO-ALLOW-SPEC-0008-ledger-coherence-change-control.md);
enforcement (`diff --require-change-note`) lands in PR 4 and is explicitly out of
scope here.

The failure mode to prevent is **silent weakening**: broadening a selector,
raising an occurrence limit, extending an expiry, or removing evidence with no
record of who decided it, why, or under what review. The contract must not, in
preventing that, force ceremony onto edits that strictly improve posture.

## Decision

Adopt a **durable, append-only revision ledger under `.allow/revisions/` that
reuses the existing `policy_change_kind` and `posture_delta` vocabulary** rather
than inventing a parallel change taxonomy: a record names the canonical change
kinds it authorizes, and the note requirement is the matched diff row's
`posture_delta` from PR 1/2.

### 1. Revision record schema

Each record is one TOML file, `.allow/revisions/CARGO-ALLOW-REV-NNNN-<slug>.toml`:

```toml
schema_version = "1.0"
id = "CARGO-ALLOW-REV-0001"
created = "2026-06-20"
owner = "repo-infra"
reason = "Raise occurrence_limit on the generated-module entries after the parser refactor."

# Entries this record authorizes. Stable allow IDs, not file lines.
allow_ids = ["allow-0042", "allow-0043"]

# Canonical change kinds this record authorizes for those entries. Values are
# members of the existing `policy_change_kind` vocabulary (see below).
change_kinds = ["occurrence_limit_loosened"]

# Optional: bind the record to one specific transition (PR 4 may require this for
# repeatable change kinds). Absent => match by change_kind alone.
before_fingerprint = "sha256:…"   # optional
after_fingerprint = "sha256:…"    # optional

# Optional governance metadata.
review_after = "2026-09-01"        # advisory revisit date; does not invalidate
supersedes = "CARGO-ALLOW-REV-0000" # optional correction chain (append-only)
links = ["issue:1475", "pr:1772"]
```

Required fields: `schema_version`, `id`, `created`, `owner`, `reason`,
`allow_ids` (non-empty), `change_kinds` (non-empty). All other fields are
optional. Records are validated by a future `allow-policy` parser (PR 4) with
`#[serde(deny_unknown_fields)]`, mirroring the existing spec-system manifests.

### 2. Change kinds reuse the existing vocabulary; the note rule is the row's `posture_delta`

`change_kinds` are **members of the existing `policy_change_kind` enum**, not a
new taxonomy. That enum is defined in `allow-diff`
(`crates/allow-diff/src/policy_change_kind.rs`, `PolicyChangeKind`) and published
in `docs/schemas/common.v1.json` (`policy_change_kind`); its band classification
is documented in [docs/policy-weakening.md](../policy-weakening.md). Reusing it is
the whole point of GOAL-0004: change control and the diff classifier must share
one definition of what a weakening is, not maintain two.

**The machine rule for whether a note is required is the matched diff row's
`posture_delta`** — already computed by `allow-diff`
(`policy_change_posture_delta`, `crates/allow-diff/src/movement.rs`) from each
change's `PolicyChangeSeverity`:

| Row `posture_delta` (severity) | Revision note |
| --- | --- |
| `worsened` (`fail`) | **required** |
| `review_required` (`review`) | note **or** explicit reviewer acknowledgement (surface chosen in PR 4) |
| `improved` (`improvement`) | not required |
| `unchanged` | not required |

`posture_delta` is authoritative rather than a static per-kind band because some
kinds are **context dependent** — for example `evidence_removed` is `fail` only
when it removes the last locally valid evidence, and `improvement` otherwise
(`crates/allow-diff/src/policy_entry_evidence.rs`). A record names the
`change_kind`s it authorizes; enforcement matches them against rows the diff has
independently classified `worsened`. The three bands map onto the
[docs/policy-weakening.md](../policy-weakening.md) signal groups:

- **Failing weakening signals** (note required): e.g. `scope_broadened`,
  `selector_precision_decreased`, `occurrence_limit_loosened`, `expiry_extended`,
  `review_after_extended`, `evidence_removed`, `owner_removed`,
  `requirement_loosened`, `baseline_debt_normalized`.
- **Review-required changes** (note or ack): e.g. `scope_changed`,
  `selector_changed`, `owner_changed`, `reason_changed`, `classification_changed`.
- **Improvement signals** (exempt): e.g. `scope_narrowed`,
  `selector_precision_increased`, `occurrence_limit_tightened`,
  `expiry_shortened`, `evidence_added`, `owner_added`, `removed_allow`.

**Out of scope of the revision contract:** creating a brand-new exception
(`added_allow`). New entries carry `reason`, evidence, owner, and lifecycle at
creation through the `add` workflow, which already governs them; double-governing
creation would only add ceremony.

### 3. Multi-entry coverage

`allow_ids` and `change_kinds` are arrays so one record covers a coordinated
edit. A record authorizes the cartesian product it lists: any entry in
`allow_ids` undergoing any kind in `change_kinds`. A single worsened diff is
satisfied by the **union** of all matching records, so reviewers may split a large
change across several focused records or consolidate it into one.

### 4. How notes match a diff

Matching is by **stable identity, never by file line**:

1. Collect every diff row with `posture_delta == worsened` for a governed entry.
2. For each such row, derive its `change_kind` from the before/after fields.
3. The row is **covered** if some record exists where
   `row.allow_id ∈ record.allow_ids` and `row.change_kind ∈ record.change_kinds`,
   and — when the record carries fingerprints — `after_fingerprint` matches the
   row's post-edit fingerprint.
4. Worsened rows with no covering record are reported (PR 4) as
   `change_note_required`. Records that cover no row in the current diff are
   **inert, not errors** — this is what lets a record be authored ahead of, or
   alongside, the edit it explains.

Matching consumes the existing `allow_id` carried on every diff row since PR 2, so
no new identity plumbing is required.

### 5. Expiry and append-only posture

- **Records do not expire after merge.** They are a permanent audit trail. A
  merged record stays in `.allow/revisions/` as the durable explanation of a
  historical decision. `review_after` is advisory only — it can surface a
  revisit reminder but never invalidates the record or re-opens the requirement.
- **Records are append-only and immutable.** A mistake is corrected by writing a
  new record with `supersedes` pointing at the old one; the old record is never
  edited or deleted in place. This yields a tamper-evident chain.
- **Repeatable weakenings need fresh authorization.** Because matching is by
  change kind, a record authorizing `occurrence_limit_loosened` for `allow-0042`
  would otherwise also cover a *second, later* increase. To close that loophole,
  PR 4 may require `before_fingerprint`/`after_fingerprint` on repeatable kinds so
  a record binds to exactly one transition. When fingerprints are absent the
  contract falls back to change-kind matching, which is sufficient for one-shot
  kinds (e.g. `owner_removed`).

### Ledger placement

`.allow/revisions/` is a distinct governance graph, not a spec-system
doc-artifact. Revision records are **not** registered in
`.allow/artifacts/doc-artifacts.toml`; they will be registered as a federation
ledger in `.allow/config.toml` (dialect `cargo-allow-revisions`, lane
`change-control`) when PR 4 wires runtime parsing. This ADR and the directory
`README.md` are the only spec-system artifacts the design slice adds.

## Alternatives Considered

| Alternative | Tradeoff |
| --- | --- |
| Reuse `reason` for change rationale | Conflates "why it exists" with "why it changed"; loses both once edited again |
| One change log appended to `policy/allow.toml` | Couples audit trail to the policy file; merge-conflict prone; not matchable per entry |
| A fresh four-value change enum independent of `PostureDelta` | Creates a second source of truth about weakening, the exact incoherence GOAL-0004 removes |
| Notes that expire/clear after merge | Erases provenance; the record's whole value is durability |
| Mutable records edited in place | Destroys the audit trail; correction must be append-only |
| Require a note for every edit including improvements | Punishes the behavior we want (narrowing, adding evidence) with ceremony |
| Match notes to diff by file line or hunk | Breaks under reformatting and refactor; stable `allow_id` is the right key |

## Consequences

### Positive

- Weakening edits gain a durable, matchable, append-only record keyed to stable
  identity.
- The note requirement reuses the canonical `PostureDelta`, so diff classifier and
  change-control share one definition of "weakening".
- Improvements stay friction-free; the contract never blocks narrowing, adding
  evidence, assigning an owner, or removing entries.
- PR 4 enforcement has an unambiguous matching algorithm and schema to implement.

### Negative

- A closed `change_kind` enum must be derived from before/after fields in PR 4;
  some edits (e.g. a selector that both narrows and broadens) need a deliberate
  `review_required` classification rather than a clean improve/worsen call.
- Repeatable change kinds need fingerprints to avoid stale-record reuse, adding
  optional complexity to PR 4.

### Neutral Or Operational

- The spec-system audit validates this ADR and the `.allow/revisions/README.md`
  link graph statically; it does not parse or enforce revision records.
- Revision records join the federation ledger model (ADR-0001) as a future
  `change-control` lane rather than the doc-artifact graph.

## Support-Tier Impact

advisory — designing the contract changes no `check` support claim. Support-tier
promotion is reconsidered only after PR 4 enforcement and PR 8 dogfood evidence,
per CARGO-ALLOW-SPEC-0008.

## Policy Impact

- `.allow/artifacts/doc-artifacts.toml` — register this ADR and the PR 3 closeout.
- `.allow/revisions/` — new governance directory with `README.md` and an example
  record; no runtime parsing or enforcement in this slice.
- `policy/allow.toml` — unchanged; no new source exceptions required for design
  docs.

## Required Evidence

- Spec-system audit passes with the ADR and revisions directory registered.
- No-new guard passes after the docs land.
- PR 4 must add `allow-policy` parse/validate tests for the record schema and an
  `allow-diff` matching test for covered vs uncovered worsened rows before
  claiming enforcement.

## Non-Goals

- CLI enforcement (`diff --require-change-note`,
  `--write-change-note-template`) — PR 4.
- Runtime parsing or federation-ledger registration of `.allow/revisions/` — PR 4.
- Automatic note generation or silent approval of weakening.
- Repository dogfood of the change-control loop — PR 8.

## Claim Boundary

This ADR records the revision-contract design decision: schema, change-kind
taxonomy anchored to `PostureDelta`, multi-entry coverage, identity-based diff
matching, durable append-only posture, and ledger placement. It does not prove
enforcement correctness, parse the record schema, classify real diffs, authorize
a release cut, or claim macro-expanded, type-aware, MIR-level, or build-aware
behavior.

## Rollback Or Supersession

Supersede this ADR if cargo-allow adopts a different change-control model (for
example, in-policy change logs, or a note requirement decoupled from
`PostureDelta`). A replacement ADR must link here and update
CARGO-ALLOW-SPEC-0008 before PR 4 enforcement ships.
