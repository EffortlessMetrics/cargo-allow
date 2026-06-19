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
  - .allow/goals/archive/CARGO-ALLOW-GOAL-0003-portable-governance-substrate.toml
  - .allow/artifacts/doc-artifacts.toml
---

# Closeout: GOAL-0003 Portable Governance Substrate

## Summary

Governance closeout recording portable governance substrate execution on
CARGO-ALLOW-GOAL-0003 after the import graph dogfood receipt closed (#1767;
CARGO-ALLOW-CLOSEOUT-0018). Profile migration (C2–C4), multi-ledger federation
(F0–F3), import graph model plus ecosystem adapters (I1–I2), and import graph
dogfood are done. GOAL-0003 is complete; `.allow/goals/active.toml` retains
blocked follow-ups only. External `ripr` preflight (R0), external ripr
migration, and full import mode (#1466) remain blocked.

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

### Import graph dogfood (I1+I2)

- Import graph dogfood receipt (#1767; CARGO-ALLOW-CLOSEOUT-0018):
  `docs/dogfood/cargo-allow-import-graph.md` with committed spec-system audit
  JSON for main-repo and I2 characterization fixtures.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo-allow check --profile spec-system --mode audit` | pass | governance PR proof |
| `cargo-allow check --mode no-new` | pass | `target/cargo-allow/check.receipt.json` |
| `parses_current_repository_active_goal_manifest` | pass | GOAL-0003 done stub validation |

## Non-Goals

- External `ripr` repository migration or R0 preflight execution.
- Full import mode product behavior (#1466).
- Version bump or `0.1.10` release authorization.

## Remaining Work

- **Completed goal:** CARGO-ALLOW-GOAL-0003 portable governance substrate.
  Full execution history:
  `.allow/goals/archive/CARGO-ALLOW-GOAL-0003-portable-governance-substrate.toml`.
- **Blocked follow-ups in `.allow/goals/active.toml`:**
  `portable-governance-ripr-preflight-r0`, `portable-governance-external-ripr`,
  `portable-governance-full-import`.
- **Ready:** none.

## Claim Boundary

Governance and execution-lane closeout evidence only. Does not prove external
adoption, full import mode, ripr migration, or release readiness.
