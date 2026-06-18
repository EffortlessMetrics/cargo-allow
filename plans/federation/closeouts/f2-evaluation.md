---
id: CARGO-ALLOW-CLOSEOUT-0011
kind: closeout
status: draft
owner: repo-infra
created: 2026-06-18
linked_plan: CARGO-ALLOW-PLAN-0007
linked_proposal: CARGO-ALLOW-PROP-0007
linked_spec: CARGO-ALLOW-SPEC-0007
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0003
support_tier_impact: advisory
policy_impact:
  - .allow/config.toml
  - .allow/goals/active.toml
  - docs/schemas/receipt.schema.json
---

# Closeout: Multi-Ledger Federation Check Evaluation (F2)

## Summary

Execution closeout for CARGO-ALLOW-PLAN-0007 F2. Adds canonical ledger evaluation
on the source-exception `check` path with deterministic precedence and provenance
on findings, work items, and receipts:

- `allow-policy::federation::evaluate` resolves canonical ledgers from
  `.allow/config.toml` for `source-exception` and `spec-system` lanes.
- `check` annotates findings and receipt `federation.ledger_contributors` /
  `precedence_applied` when federation config is present.
- Worklist and spec-system work items carry `ledger_id`, `ledger_path`, `lane`,
  `mode`, and `role` when applicable.
- Dogfood `.allow/config.toml` registers `source-policy` and `doc-artifacts`
  canonical ledgers.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test -p allow-policy federation` | pass | 10 tests (includes 2-ledger fixture) |
| `cargo test -p cargo-allow doctor` | pass | 20 tests |
| `cargo-allow check --profile spec-system --mode audit` | pass | `target/cargo-allow/spec-system.json` |
| `cargo-allow check --mode no-new` | pass | `target/cargo-allow/check.receipt.json` |

## Non-Goals

- Mirror divergence enforcement or drain windows (F3).
- Imported ledger evaluation or release authorization.

## Remaining Work

- **Ready:** `portable-governance-f3-federation` (drain window enforcement; pending F2 merge).

## Claim Boundary

F2 canonical ledger evaluation and receipt provenance only. Does not prove mirror
divergence handling, drain-window blocking, or release readiness.
