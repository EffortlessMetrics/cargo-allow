---
id: CARGO-ALLOW-CLOSEOUT-0006
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
  - plans/migration-parity/gap-inventory.md
---

# Closeout: `.allow` Profile Resolution (C2)

## Summary

Execution closeout for CARGO-ALLOW-PLAN-0004 C2 after #1748 merge
`2adb0b5e`. Spec-system checks resolve profile config with `.allow/`
precedence and legacy `policy/<profile>.toml` fallback; doctor and receipts
report `config_provenance`; owned plus legacy configs emit an advisory conflict
diagnostic.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test -p allow-policy profile_resolution` | pass | #1748 CI |
| `cargo test -p cargo-allow profile_resolution` | pass | #1748 CI |
| `cargo-allow check --profile spec-system --mode audit` | pass | #1748 CI |
| `cargo-allow check --mode no-new` | pass | #1748 CI |

## Non-Goals

- `init` writing spec-system state to `.allow/` (queued as `portable-governance-c3`).
- Dogfood profile state migration (C4).
- Import adapters (C8–C11) or full import mode (#1466).

## Remaining Work

- **Active goal:** CARGO-ALLOW-GOAL-0003 portable governance substrate.
- **Ready:** `portable-governance-c3` (`init` writes spec-system state to `.allow/` per C3).
- **Blocked:** federation (#1473), external ripr adoption, full import mode (#1466).

## Claim Boundary

C2 resolver and provenance reporting only. Does not migrate this repository's
dogfood profile paths or authorize release cut.
