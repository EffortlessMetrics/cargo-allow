---
id: CARGO-ALLOW-CLOSEOUT-0012
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
  - .allow/config.toml
  - .allow/goals/active.toml
  - docs/schemas/receipt.schema.json
  - docs/schemas/worklist.schema.json
  - docs/schemas/report.schema.json
---

# Closeout: Multi-Ledger Federation Drain Windows and Mirror Divergence (F3)

## Summary

Execution closeout for CARGO-ALLOW-PLAN-0007 F3. Adds drain-window configuration and
visible canonical-vs-mirror divergence reporting without silent merge:

- `[[drain_windows]]` in `.allow/config.toml` federation config with validation.
- `allow-policy::federation::divergence` compares canonical and mirror policy ledgers
  during active drain windows; emits `mirror_divergence`, `mirror_stale`, and blocking
  `drain_expired` records.
- Doctor, check receipts (`federation.divergence_summary`), worklist (`mirror_divergence`
  item kind), and advisory counts surface divergence; optional `check --deny mirror_divergence`.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test -p allow-policy federation` | pass | #1759 merge `f81a9d2` |
| `cargo test -p cargo-allow` | pass | #1759 merge `f81a9d2` |
| `cargo-allow check --mode no-new` | pass | #1759 merge `f81a9d2` |
| `cargo-allow check --profile spec-system --mode audit` | pass | post-merge proof |

## Non-Goals

- Imported ledger evaluation or external spec adapters (I1+).
- Full import mode (#1466) or external `ripr` migration.
- Release authorization or support-tier promotion.

## Remaining Work

- **Done:** `portable-governance-f3-federation` (F3 drain windows and mirror divergence; #1759).
- **Done:** `portable-governance-i1-import` (generic import-root model; #1761).

## Claim Boundary

F3 drain-window enforcement and mirror divergence reporting only. Does not prove
import adapters, full federation across repositories, or release readiness.
