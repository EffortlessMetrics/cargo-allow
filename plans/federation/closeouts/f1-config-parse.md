---
id: CARGO-ALLOW-CLOSEOUT-0010
kind: closeout
status: done
owner: repo-infra
created: 2026-06-18
linked_plan: CARGO-ALLOW-PLAN-0007
linked_proposal: CARGO-ALLOW-PROP-0007
linked_spec: CARGO-ALLOW-SPEC-0007
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0003
support_tier_impact: advisory
policy_impact:
  - .allow/goals/active.toml
  - docs/schemas/doctor.schema.json
---

# Closeout: Multi-Ledger Federation Config Parse (F1)

## Summary

Execution closeout for CARGO-ALLOW-PLAN-0007 F1. Adds federation registry
parse/validate for `[[ledgers]]` in `.allow/config.toml` and doctor/spec-system
reporting without changing check-time policy evaluation:

- `allow-policy::federation` load, role/mode defaults, precedence ordering, and
  validation (duplicate IDs/paths, mirror targets, canonical lane collisions,
  dialect rules).
- `cargo-allow doctor` and spec-system readiness surface configured ledgers and
  federation diagnostics; `doctor.schema.json` gains a `federation` section.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test -p allow-policy federation` | pass | #1756 PR proof |
| `cargo test -p cargo-allow doctor` | pass | #1756 PR proof |
| `cargo-allow check --profile spec-system --mode audit` | pass | #1756 PR proof |
| `cargo-allow check --mode no-new` | pass | #1756 PR proof |
| `parses_current_repository_active_goal_manifest` | pass | #1756 PR proof |

## Non-Goals

- Multi-ledger check evaluation or lane matching (F2).
- Receipt `ledger_contributors` provenance fields (F2).
- Drain window enforcement (F3).
- Version bump or `0.1.10` release authorization.

## Remaining Work

- **Done:** `portable-governance-f1-federation` (F1 config parse/validate; #1756).
- **Ready:** `portable-governance-f2-federation` (multi-ledger check evaluation and receipt provenance).
- **Blocked:** `portable-governance-f3-federation` (drain window enforcement; pending F2).

## Claim Boundary

F1 config parse/validate and doctor/spec-system registry reporting only. Does not
evaluate findings against multiple ledgers or prove receipt provenance fields.
