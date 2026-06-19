---
id: CARGO-ALLOW-CLOSEOUT-0019
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
policy_impact:
  - .allow/goals/active.toml
  - .allow/artifacts/doc-artifacts.toml
  - policy/allow.toml
---

# Closeout: GOAL-0004 Registration (PR 0)

## Summary

Governance closeout for registering **CARGO-ALLOW-GOAL-0004: Core exception
ledger coherence and change control** after GOAL-0003 portable governance
substrate closed (#1768 merge `cb1e27dc`). Adds proposal, spec, implementation
plan, active goal with PR 1–9 work items, doc-artifact registry entries, and
issue reconciliation for completed ratcheting/federation lanes.

## Landed In PR 0

- `CARGO-ALLOW-PROP-0008`, `CARGO-ALLOW-SPEC-0008`, `CARGO-ALLOW-PLAN-0009`.
- `.allow/goals/active.toml` now tracks GOAL-0004 with
  `ledger-coherence-pr1-canonical-state-model` ready.
- Issue reconciliation:
  - #1472 already closed (#1745 `occurrence_headroom`).
  - #1474 already closed (advisory counters + `--deny` escalation).
  - #1473 closed after per-lane posture and F0–F3 federation landed.
- GOAL-0003 archive and `superseded_by` chains for GOAL-0001/GOAL-0002
  unchanged.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo-allow check --profile spec-system --mode audit` | pass | governance PR proof |
| `cargo-allow check --mode no-new` | pass | `target/cargo-allow/check.receipt.json` |
| `parses_current_repository_active_goal_manifest` | pass | GOAL-0004 manifest validation |

## Non-Goals

- Canonical ledger state types (PR 1).
- Diff movement classification (PR 2).
- Revision-note enforcement (PR 4).
- Release authorization.

## Remaining Work

- **Ready:** `ledger-coherence-pr1-canonical-state-model`.
- **Blocked:** PR 2–9 sequenced behind prior slices; ripr and full import mode
  remain blocked on explicit adoption need.
- **Successor context:** GOAL-0003 full history remains in
  `.allow/goals/archive/CARGO-ALLOW-GOAL-0003-portable-governance-substrate.toml`.

## Claim Boundary

Governance registration and issue reconciliation only. Does not implement
ledger-coherence behavior, prove diff semantics, or authorize release cut.
