---
id: CARGO-ALLOW-CLOSEOUT-0020
kind: closeout
status: done
owner: repo-infra
created: 2026-06-19
linked_plan: CARGO-ALLOW-PLAN-0009
linked_proposal: CARGO-ALLOW-PROP-0008
linked_spec: CARGO-ALLOW-SPEC-0008
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0004
support_tier_impact: advisory
policy_impact: []
---

# Closeout: GOAL-0004 PR 1 — Canonical Ledger State Model

## Summary

Internal vocabulary consolidation in `allow-core` and `allow-report::ledger_posture`.
No user-visible semantic change.

## Landed

- `PresenceMovement`, `PostureDelta`, `LedgerPosture`, `NetPosture` in `allow-core`.
- Centralized artifact string registries in `allow-report::ledger_posture`.
- `FindingPostureKind` and `DiffNetPosture` delegate to canonical types.
- SPEC-0008 updated for internal model vs PR-summary projection.

## Validation

| Check | Result |
| --- | --- |
| `cargo test -p allow-core` | pass |
| `cargo test -p allow-report` | pass |
| `cargo test -p allow-diff` | pass |
| `cargo test -p cargo-allow artifact_schema` | pass |
| `cargo-allow check --mode no-new` | pass |
| `cargo-allow check --profile spec-system --mode audit` | pass |

## Remaining

- **Ready:** `ledger-coherence-pr2-movement-classification`.
- Dual summary counts and per-row movement/posture fields (PR 2).

## Claim Boundary

Internal type consolidation and characterization tests only. Does not implement
diff row classification, revision enforcement, or release cut.
