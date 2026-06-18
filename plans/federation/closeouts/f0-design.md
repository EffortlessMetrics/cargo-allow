---
id: CARGO-ALLOW-CLOSEOUT-0009
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
  - .allow/artifacts/doc-artifacts.toml
  - .allow/goals/active.toml
---

# Closeout: Multi-Ledger Federation Design (F0)

## Summary

Design closeout for CARGO-ALLOW-PLAN-0007 F0. Federation design artifacts
define how multiple durable ledgers coexist without silent merging:

- [CARGO-ALLOW-PROP-0007](../../docs/proposals/CARGO-ALLOW-PROP-0007-multi-ledger-federation.md)
- [CARGO-ALLOW-SPEC-0007](../../docs/specs/CARGO-ALLOW-SPEC-0007-multi-ledger-federation.md)
- [CARGO-ALLOW-ADR-0001](../../docs/adr/CARGO-ALLOW-ADR-0001-multi-ledger-federation.md)

Topics covered: ledger IDs; canonical/mirror/imported roles; lane ownership;
deterministic precedence; duplicate detection; dialect handling; drain windows;
divergence reporting; receipt provenance; no silent merging.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `parses_current_repository_active_goal_manifest` | pass | F0 PR proof |
| `cargo-allow check --profile spec-system --mode audit` | pass | F0 PR proof |
| `cargo-allow check --mode no-new` | pass | `target/cargo-allow/check.receipt.json` |

## Non-Goals

- Runtime federation resolver or receipt schema changes (F1).
- Full import mode (#1466) or external `ripr` migration.
- Version bump or `0.1.10` release authorization.

## Remaining Work

- **Done:** `portable-governance-f0-federation` (F0 design-only).
- **Blocked:** `portable-governance-f1-federation` (runtime; pending F0 merge).

## Claim Boundary

F0 design registration only. Does not implement federation or prove multi-lane
parity.
