---
id: CARGO-ALLOW-CLOSEOUT-0017
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
  - .allow/goals/active.toml
---

# Closeout: GOAL-0003 Partial Progress (C2–C4, F0–F3, I1–I2)

## Summary

Governance closeout recording portable governance substrate progress on
CARGO-ALLOW-GOAL-0003 after the I2 import adapter lane closed (#1761–#1765).
Profile migration (C2–C4), multi-ledger federation (F0–F3), and import graph
model plus ecosystem adapters (I1–I2) are done. External `ripr` preflight (R0),
external ripr migration, and full import mode (#1466) remain blocked.

## Completed Lanes

### Profile migration (C2–C4)

- C2 `.allow` profile resolution with `policy/` fallback (#1748 merge `2adb0b5e`).
- C3 `init` writes spec-system state to `.allow/` (#1750 merge `23ac8376`).
- C4 dogfood migrate profile state to `.allow/` (#1752 merge `651d9c90`).

### Multi-ledger federation (F0–F3)

- F0 design (#1755), F1 config parse (#1756), F2 check evaluation (#1758),
  F3 drain-window enforcement and mirror divergence (#1759 merge `f81a9d2`).

### Import graph model and adapters (I1–I2)

- I1 generic import-root model (#1761 merge `3912baa6`).
- I2 generic `.spec`/`.rails`/repo-spec adapters, Kiro/Spec Kit adapters
  (C8–C9), and xtask command registry adapter (C11 #1765).

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo-allow check --profile spec-system --mode audit` | pass | governance PR proof |
| `cargo-allow check --mode no-new` | pass | `target/cargo-allow/check.receipt.json` |
| `parses_current_repository_active_goal_manifest` | pass | GOAL-0003 manifest validation |

## Non-Goals

- External `ripr` repository migration or R0 preflight execution.
- Full import mode product behavior (#1466).
- Version bump or `0.1.10` release authorization.

## Remaining Work

- **Ready:** `portable-governance-import-dogfood` — in-repository import graph
  dogfood receipt documenting spec-system `import_graph` from I1+I2 adapters.
- **Blocked:** `portable-governance-ripr-preflight-r0`, `portable-governance-external-ripr`,
  `portable-governance-full-import`.

## Claim Boundary

Governance and execution-lane closeout evidence only. Does not prove external
adoption, full import mode, ripr migration, or release readiness.
