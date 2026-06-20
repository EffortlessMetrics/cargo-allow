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
  - policy/allow.toml
---

# ADR: Policy Revision Contract and Change-Note Matching

## Context

cargo-allow's exception ledger records **why an exception exists** through each
entry's `reason`, but it has no durable record of **why a governed entry
changed**. Issue #1475 identifies the gap: when a maintainer broadens a
selector, widens scope, raises an occurrence limit, weakens evidence, or extends
a lifecycle window, the diff shows the edit but nothing explains or authorizes
it. Movement classification (GOAL-0004 PR 2, #1471) now labels every diff row
with an orthogonal `movement` and `posture_delta`, so the system can already
detect a `worsened` posture delta — but it cannot yet require accountability for
one.

This ADR records the design contract for `.allow/revisions/` change notes.
Enforcement (`diff --require-change-note`) is deferred to GOAL-0004 PR 4; this
decision only fixes the schema, the vocabulary of governed change kinds, the
multi-entry coverage model, the diff-matching rule, and the durability posture
so that PR 4 implements an accepted contract rather than inventing one.

The design pressure is to authorize weakening **without** laundering temporary
debt into silent durable approval: a note must point at a specific intended
end state, cover exactly the entries a single logical edit touched, and remain
an append-only record after merge.

## Decision

Adopt a **fingerprint-anchored, append-only revision-note contract** stored as
one TOML record per file under `.allow/revisions/`.

### 1. Which changes require a note

A note is required only for **governed weakening edits** — edits where a diff
row's `posture_delta` is `worsened`. The governed change-kind vocabulary is
fixed:

| `change_kind` | Weakening edit it records |
| --- | --- |
| `selector_broadened` | Structural selector constraints removed or generalized |
| `scope_widened` | Path/glob widened, or lane moved to a broader owner |
| `occurrence_limit_raised` | `occurrence_limit` increased above prior value |
| `evidence_weakened` | Typed evidence removed or downgraded |
| `classification_relaxed` | Classification moved to a weaker governance class |
| `lifecycle_extended` | `expires` or `review_after` pushed further out |
| `owner_removed` | Accountable owner cleared or made less specific |
| `posture_weakened` | Effective posture moved to a weaker state not covered above |

Edits whose `posture_delta` is `improved`, `review_required`, or `unchanged`
**do not** require a note. Obvious improvements are explicitly exempt: narrowing
a selector or scope, adding typed evidence, assigning or sharpening an owner,
reducing `occurrence_limit`, bringing `review_after`/`expires` forward, removing
stale or expired entries via prune. `review_required` edits surface in PR
posture for human attention but are not blocked by the note contract.

### 2. How one note covers multiple entries

A single note carries `allow_ids = [...]` and covers every listed entry. One
note may cover multiple entries **only when they share the same logical edit** —
the same `reason` and `links`. The note's `change_kinds` is the union of the
governed change kinds across the covered entries. PR 4 enforcement treats a note
as covering an entry when the entry's stable `allow_id` appears in
`note.allow_ids` and the row's detected governed change kind appears in
`note.change_kinds`.

### 3. How notes match a diff

To prevent a single note from rubber-stamping all future weakening of the same
entry, each covered entry records an intended end state in `after_fingerprints`:
the post-edit selector/snippet fingerprint already computed for that entry
(`fnv1a64:` snippet hash or structural selector fingerprint, mirroring
`policy/allow.toml` `normalized_snippet_hash`). A note **matches** a `worsened`
diff row when all of the following hold:

1. `note.allow_ids` contains the row's `allow_id`;
2. `note.change_kinds` covers every governed change kind detected on the row;
3. the row's after-state fingerprint is a member of the note's
   `after_fingerprints` set, when the note declares one;
4. the note's `status` is `accepted` (not `superseded`).

`after_fingerprints` is optional in the schema but recommended; it is a set
(membership, not positional alignment with `allow_ids`) so a single note can
anchor several entries' end states. When omitted the note matches on `allow_id`
+ `change_kind` alone, which PR 4 may treat as a weaker advisory match rather
than a blocking authorization.

### 4. Whether notes expire after merge

Notes are **durable provenance and do not expire or get consumed at merge**.
A note is not deleted, archived, or invalidated when its diff lands; it remains
the permanent record of why that specific end state was authorized. There is no
TTL field. A later weakening of the same entry to a *different* end state will
not match an existing note (its fingerprint differs), so durability does not
weaken future enforcement.

### 5. Whether notes are append-only

Notes are **append-only**. After merge a note is never edited in place; the git
history is the audit trail. A superseding correction adds a *new* note that sets
`supersedes` to the prior note ID, and the prior note is marked
`status = "superseded"` with `superseded_by` pointing forward. This mirrors the
doc-artifact `supersedes`/`superseded_by` chain already validated by the
spec-system profile.

## Record Shape

```toml
schema_version = "1.0"
id = "CARGO-ALLOW-REV-0001"
created = "2026-06-20"
owner = "core/policy"
reason = "Narrow then re-broaden selector after parser refactor changed the AST shape."
status = "accepted"

allow_ids = ["allow-0042"]
change_kinds = ["selector_broadened"]
after_fingerprints = ["fnv1a64:d89cea4b9dc969d2"]

links = [
  "issue:1475",
  "pr:456",
]

# supersedes = "CARGO-ALLOW-REV-0000"   # optional, append-only correction chain
```

The JSON Schema for this record is
[docs/schemas/revision.schema.json](../schemas/revision.schema.json). The
directory contract is described in
[.allow/revisions/README.md](../../.allow/revisions/README.md).

## Alternatives Considered

| Alternative | Tradeoff |
| --- | --- |
| Collapse the note into a free-text `change_reason` on each `[[allow]]` entry | No multi-entry coverage, no diff matching, and edits to the field are invisible against the entry itself; loses append-only provenance |
| One revision file listing all notes | Forces in-place edits on every change, breaking append-only and creating merge conflicts; per-file records keep edits isolated |
| Match notes by `allow_id` only | A single note would authorize unlimited future weakening of the same entry; fingerprint anchoring scopes a note to one intended end state |
| Expire notes after merge (TTL) | Erases the durable record of why an end state was authorized and reopens the accountability gap the contract closes |

## Consequences

### Positive

- Consequence: governed weakening edits gain a durable, reviewable authorization
  record without blocking improvements.
- Consequence: fingerprint anchoring prevents one note from silently approving
  all future weakening of the same entry.
- Consequence: append-only per-file records reuse the existing
  `supersedes`/`superseded_by` chain and avoid merge churn.

### Negative

- Consequence: maintainers must author a note for each governed weakening edit,
  adding a step to those PRs.
- Consequence: fingerprint drift between note authoring and merge can require
  re-anchoring a note before PR 4 enforcement accepts it.

### Neutral Or Operational

- Consequence: notes accumulate under `.allow/revisions/` and become part of the
  tracked source-tree inventory governed by `policy/allow.toml`.
- Consequence: the change-kind vocabulary is fixed here and extended only by a
  superseding ADR.

## Support-Tier Impact

advisory. No support-tier promotion or proof-command mapping changes in the
design slice; review `CARGO-ALLOW-SUPPORT-0001` after PR 4 enforcement and PR 8
dogfood evidence land.

## Policy Impact

Registers `.allow/revisions/` as a governed source-tree directory and adds the
revision JSON Schema under `docs/schemas/`. Adds no enforcement to `diff` and
changes no existing `policy/allow.toml` entry semantics.

## Required Evidence

- Evidence expected to validate the decision remains true: PR 4 enforcement maps
  every `worsened` diff row to a matching accepted note or reports a missing
  note, using the schema and matching rule fixed here.
- Evidence expected to validate the decision remains true: a fixture revision
  record under `.allow/revisions/` parses against
  `docs/schemas/revision.schema.json` once a parser lands.

## Non-Goals

- Non-goal: CLI enforcement (`diff --require-change-note`,
  `--write-change-note-template`) — GOAL-0004 PR 4.
- Non-goal: automatic note generation, approval, or silent policy mutation.

## Claim Boundary

This ADR records the revision-contract design and matching rule. It does not
implement enforcement, prove diff-matching correctness, parse or validate
revision records in code, authorize a release cut, or claim macro-expanded,
type-aware, MIR-level, or build-aware analysis.

## Rollback Or Supersession

Revert the ADR, the revision schema, and the `.allow/revisions/` directory
contract if the change-control lane is withdrawn. A future change to the
change-kind vocabulary, fingerprint anchoring, or durability posture must land
as a new ADR that sets `supersedes: CARGO-ALLOW-ADR-0002` and updates the
`superseded_by` pointer here.
