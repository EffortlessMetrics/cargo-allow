---
id: CARGO-ALLOW-CLOSEOUT-0022
kind: closeout
status: done
owner: repo-infra
created: 2026-06-20
linked_plan: CARGO-ALLOW-PLAN-0009
linked_proposal: CARGO-ALLOW-PROP-0008
linked_spec: CARGO-ALLOW-SPEC-0008
linked_adr: CARGO-ALLOW-ADR-0002
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0004
support_tier_impact: advisory
policy_impact:
  - .allow/artifacts/doc-artifacts.toml
  - .allow/goals/active.toml
  - policy/allow.toml
---

# Closeout: GOAL-0004 PR 3 — Policy Revision Contract Design

## Summary

Design-only acceptance of the `.allow/revisions/` policy revision-note contract
for #1475. Fixes the record schema, the governed change-kind vocabulary,
multi-entry coverage, the fingerprint-anchored diff-matching rule, and the
append-only / non-expiring durability posture so GOAL-0004 PR 4 enforces an
accepted contract. No enforcement, parser, or CLI behavior lands here.

## Landed Changes

- `docs/adr/CARGO-ALLOW-ADR-0002-policy-revision-contract.md`: accepted ADR
  recording the five contract decisions and alternatives.
- `docs/schemas/revision.schema.json`: JSON Schema for a revision-note record.
- `.allow/revisions/README.md` and `.allow/revisions/example-revision.toml`:
  directory contract and a committed example record.
- `docs/specs/CARGO-ALLOW-SPEC-0008`: revision section finalized and linked to
  ADR-0002.
- `docs/schemas/README.md`: revision schema indexed.
- Governance: ADR-0002 and this closeout registered in
  `.allow/artifacts/doc-artifacts.toml`; `ledger-coherence-pr3-revision-contract-design`
  marked done and `ledger-coherence-pr4-enforce-change-notes` set ready in
  `.allow/goals/active.toml`; new tracked docs added to `policy/allow.toml`.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo-allow check --profile spec-system --mode audit` | pass | PR 3 proof |
| `cargo-allow check --mode no-new` | pass | `target/cargo-allow/check.receipt.json` |
| `cargo test -p allow-policy spec_system::tests::parses_current_repository_active_goal_manifest` | pass | active-goal manifest validation |

## Non-Goals

- CLI enforcement (`diff --require-change-note`,
  `--write-change-note-template`) — PR 4.
- Revision-record parsing/validation in code — PR 4.
- Release authorization or external adoption.

## Claim Boundary

Design contract registration only. Does not enforce change notes, parse or
validate revision records in code, prove diff-matching correctness, or authorize
a release cut.

## Support-Tier Updates

`CARGO-ALLOW-SUPPORT-0001` unchanged. No promotion until PR 4 enforcement and
PR 8 dogfood evidence exist.

## Policy Updates

- `.allow/artifacts/doc-artifacts.toml`: register CARGO-ALLOW-ADR-0002 and
  CARGO-ALLOW-CLOSEOUT-0022.
- `policy/allow.toml`: track the new ADR, revision schema, `.allow/revisions/`
  docs, and this closeout as governed source-tree documentation.
- `.allow/goals/active.toml`: PR 3 done; PR 4 ready.

## Remaining Work

- **Ready:** `ledger-coherence-pr4-enforce-change-notes`.
- **Blocked:** PR 5–9 sequenced behind PR 4.

## Rollback

Revert ADR-0002, the revision schema, the `.allow/revisions/` directory, the
SPEC-0008 revision-section edit, and the governance registration. Restore the
PR 3 work item to ready and PR 4 to blocked.

## Follow-Up Links

- PR: GOAL-0004 PR 3
- Issue: #1475
- Next plan item: `ledger-coherence-pr4-enforce-change-notes`
