---
id: CARGO-ALLOW-SPEC-0008
kind: spec
status: accepted
owner: repo-infra
created: 2026-06-19
linked_proposal: CARGO-ALLOW-PROP-0008
linked_adrs: []
support_tier_impact: advisory
policy_impact:
  - .allow/goals/active.toml
  - .allow/artifacts/doc-artifacts.toml
---

# Spec: Core Exception Ledger Coherence and Change Control

## Summary

This spec defines the normative contract for a unified exception-ledger domain
model, orthogonal movement/posture vocabulary, policy revision records, shared
mutation receipts, converged read surfaces, and a lifecycle scenario corpus.
Implementation is sequenced in
[plans/ledger-coherence/implementation-plan.md](../../plans/ledger-coherence/implementation-plan.md).

## Ownership Model

| Surface | Role |
| --- | --- |
| `allow-core` | Canonical enums and ledger-state structs |
| `allow-policy` | Ledger parsing, validation, `.allow/revisions/` contract |
| `allow-match` | Finding evaluation, counts, drift, headroom |
| `allow-diff` | Movement and policy-change classification |
| `allow-report` | Shared views and artifact rendering |
| `cargo-allow` | CLI orchestration only |

## Movement and Posture (Normative)

Movement describes **presence** in a diff or check context. Posture delta
describes **quality** of the retained exception or policy entry.

```text
PresenceMovement:
  new | resolved | inherited

PostureDelta:
  improved | worsened | review_required | unchanged
```

Rules:

- Movement and posture delta are orthogonal fields on every diff row and shared
  ledger-state projection.
- Compact sibling projections may collapse to `new` / `worsened` / `resolved` /
  `inherited` for PR summaries, but producers must retain both canonical fields
  in machine-readable artifacts.
- Detailed policy-change reasons (selector precision, scope retarget, evidence
  removal, lifecycle extension, and similar) remain distinct from movement and
  posture delta.

## Ledger State Record (Normative)

Every read or mutation surface must be able to project the same fields for a
retained exception:

| Group | Fields |
| --- | --- |
| Identity | stable ID, kind, family, structural selector |
| Scope | path/glob, ledger ID, lane, effective posture |
| Accountability | owner, classification, reason |
| Evidence | typed evidence, links, evidence health |
| Lifecycle | created, review_after, expires, last_seen |
| Capacity | occurrence_limit, actual count, headroom |
| Current state | matched, stale, expired, review_due, drifted, debt, invalid |
| PR movement | movement, posture_delta |
| Change control | linked revision note IDs when present |
| Repair route | suggested action, proof command |

Different output formats may be concise, but they must not use conflicting
vocabulary or omit critical provenance.

## Diff Contract (Future Implementation)

Every diff row must carry:

```text
movement
posture_delta
changed_in_diff
allow_id
ledger_id
lane
```

Summary counts must expose both vocabularies:

```json
{
  "movement": {
    "new": 2,
    "resolved": 1,
    "inherited": 18
  },
  "posture_delta": {
    "improved": 3,
    "worsened": 1,
    "review_required": 2,
    "unchanged": 15
  }
}
```

The same model projects into human output, Markdown PR summary, JSON, receipt,
and worklist without removing existing detailed policy-change reasons.

## Policy Revision Contract (Design in PR 3, Enforcement in PR 4)

Revision records live under `.allow/revisions/`:

```toml
schema_version = "1.0"
id = "CARGO-ALLOW-REV-0001"
created = "2026-06-19"
owner = "repo-infra"
reason = "Narrow selector after parser refactor."

allow_ids = ["allow-0042"]
change_kinds = ["selector_changed"]

links = [
  "issue:123",
  "pr:456",
]
```

The design slice must decide:

- which changes require a note;
- how one note covers multiple entries;
- how notes are matched to a diff;
- whether notes expire after merge;
- whether notes are append-only.

Enforcement (`diff --require-change-note`) applies only after the contract is
accepted. Governed weakening edits require a matching note; obvious improvements
(narrowing scope, adding typed evidence, assigning owner, reducing
occurrence_limit, removing stale entries) do not.

## Mutation Receipt Envelope (Future Implementation)

Shared envelope fields for `add`, `propose`, `refresh`, `prune`, and `migrate`:

```text
schema_id
operation
tool_version
repo_root
config_source
ledger_ids
changed_allow_ids
before_fingerprints
after_fingerprints
result
next_commands
claim_boundary
```

Command-specific payloads remain, but provenance and changed-entry metadata must
not be independently reinvented per command.

## Lifecycle Scenario Corpus (Future Implementation)

A compact fixture corpus must cover at least:

```text
matched reviewed entry
new finding
stale entry
expired entry
review_due entry
location_drift
occurrence_headroom
limit exceeded
missing evidence
weak evidence
broken evidence
baseline_debt
invalid selector
ambiguous match
mirror divergence
policy weakening
policy improvement
review-required edit
```

The same corpus runs through `audit`, `check`, `diff`, `list`, `explain`,
`worklist`, `refresh`, and `prune` with semantic-consistency oracles rather
than brittle every-line goldens.

## Behavior Contract

The system must:

- centralize movement and posture vocabulary in `allow-core`;
- route all artifact renderers through shared view models in `allow-report`;
- classify diff rows with orthogonal movement and posture delta;
- require revision notes for governed weakening edits once PR 4 lands;
- emit shared mutation envelopes once PR 5 lands.

The system must not:

- collapse movement and posture into one enum;
- silently approve policy weakening without a revision note once enforcement
  is enabled;
- claim macro-expanded, type-aware, MIR-level, or build-aware behavior;
- authorize release cut or external adoption as part of this spec.

## Linked Artifacts

- Proposal:
  [CARGO-ALLOW-PROP-0008](../proposals/CARGO-ALLOW-PROP-0008-ledger-coherence-change-control.md)
- Implementation plan:
  [plans/ledger-coherence/implementation-plan.md](../../plans/ledger-coherence/implementation-plan.md)
- Parent product boundary:
  [docs/source-exception-ledger.md](../source-exception-ledger.md)

## Claim Boundary

This spec defines ledger-coherence design and acceptance criteria. It does not
prove implementation correctness, revision enforcement, release readiness, or
external adoption.
