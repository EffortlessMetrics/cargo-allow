---
id: CARGO-ALLOW-CLOSEOUT-0007
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

# Closeout: `init` Writes Spec-System State to `.allow/` (C3)

## Summary

Execution closeout for CARGO-ALLOW-PLAN-0004 C3 after #1750 merge
`23ac8376`. `cargo-allow init --profile spec-system` bootstraps owned profile
state under `.allow/` (profile config, artifact ledger, goals, archive, imports
stub) while keeping `policy/allow.toml` as the source-exception ledger. Legacy
`policy/` profile paths remain supported via C2 resolution fallback.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test -p cargo-allow init` | pass | #1750 CI |
| `cargo-allow check --profile spec-system --mode audit` | pass | #1750 CI |
| `cargo-allow check --mode no-new` | pass | #1750 CI |

## Non-Goals

- Dogfood profile state migration (queued as `portable-governance-c4`).
- Import-root config (C5) or import adapters (C8–C11).
- Full import mode (#1466) or external `ripr` migration.

## Remaining Work

- **Active goal:** CARGO-ALLOW-GOAL-0003 portable governance substrate.
- **Ready:** `portable-governance-c4` (dogfood migrate profile state to `.allow/` per C4).
- **Blocked:** federation (#1473), external ripr adoption, full import mode (#1466).

## Claim Boundary

C3 init bootstrap only. Does not migrate this repository's dogfood profile paths
or authorize release cut.
