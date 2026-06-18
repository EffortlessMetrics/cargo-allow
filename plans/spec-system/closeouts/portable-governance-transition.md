---
id: CARGO-ALLOW-CLOSEOUT-0005
kind: closeout
status: done
owner: repo-infra
created: 2026-06-18
linked_plan: CARGO-ALLOW-PLAN-0004
linked_proposal: CARGO-ALLOW-PROP-0004
linked_spec: CARGO-ALLOW-SPEC-0004
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0003
support_tier_impact: advisory
policy_impact:
  - policy/doc-artifacts.toml
  - .codex/goals/active.toml
---

# Closeout: Portable Governance Transition (GOAL-0002 → GOAL-0003)

## Summary

Governance closeout for migration and evidence parity execution
(CARGO-ALLOW-GOAL-0002) after adoption-substrate (CARGO-ALLOW-CLOSEOUT-0003),
import-parity (#1713–#1718, CARGO-ALLOW-CLOSEOUT-0004), post-import D8 docs,
and #1472 `occurrence_headroom` (#1745 merge `301af3ab`).

Advisory ratcheting tracked in #1474 is complete on main: receipt `advisory`
counters (#1720), `check --deny <status>` escalation (#1721), per-lane posture
(#1473), and `occurrence_headroom` outcomes/worklist/deny (#1472 / #1745).

This closeout records governance transition only. It does not implement `.allow`
profile resolution, migrate profile state, or authorize release cut.

## Archived Execution Lanes

### Migration parity incremental slices (CLOSEOUT-0002)

- Goal registration, gap inventory, B2–B6 characterization and dogfood slices.

### Adoption substrate lane (CLOSEOUT-0003)

- Modular compat surfaces, advisory ratcheting, governance split, structural
  identity D1–D8, and in-repository dogfood receipts.

### Import parity execution lane (CLOSEOUT-0004)

- #1713–#1718 characterization slices and ripr-style in-repo dogfood receipt.

### Post-import advisory completion

- #1472 `occurrence_headroom` closed (#1745).
- #1474 advisory counters + `--deny` escalation closed after #1720/#1721/#1745.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo-allow check --profile spec-system --mode audit` | pass | governance PR proof |
| `cargo-allow check --mode no-new` | pass | `target/cargo-allow/check.receipt.json` |
| `parses_current_repository_active_goal_manifest` | pass | GOAL-0003 manifest validation |

## Non-Goals

- `.allow` profile resolution implementation (queued as `portable-governance-c2`).
- P2 multi-ledger federation (#1473).
- Full import mode (#1466) or external `ripr` migration.
- Version bump or `0.1.10` release authorization.

## Claim Boundary

Governance and execution-lane closeout evidence only. `partial` compat rows in
`gap-inventory.md` are not parity claims.

## Remaining Work

- **Active goal:** CARGO-ALLOW-GOAL-0003 portable governance substrate.
- **Ready:** `portable-governance-c2` (`.allow` profile resolution with policy
  fallback per CARGO-ALLOW-PLAN-0004 C2).
- **Blocked:** federation (#1473), external ripr adoption, full import mode
  (#1466).

## Follow-Up Links

- Predecessor goal: CARGO-ALLOW-GOAL-0002 (archived)
- Closeout predecessors: CARGO-ALLOW-CLOSEOUT-0002, -0003, -0004
- Plan: `plans/spec-system/allow-import-plan.md`
- Issues: #1474 (closed), #1472 (closed), #1473 (open, blocked), #1466 (open, blocked)
