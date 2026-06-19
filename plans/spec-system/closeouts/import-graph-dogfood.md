---
id: CARGO-ALLOW-CLOSEOUT-0018
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
  - docs/dogfood/cargo-allow-import-graph.md
  - docs/dogfood/receipts/cargo-allow-import-graph-*.json
---

# Closeout: Import Graph Dogfood Receipt (I1+I2)

## Summary

Execution closeout for CARGO-ALLOW-PLAN-0004 import graph dogfood after I1
(#1761) and I2 adapters (#1763–#1765). Records in-repository spec-system
`import_graph` audit evidence on this repository and on committed Kiro, Spec
Kit, and xtask characterization fixtures.

Dogfood receipt: [docs/dogfood/cargo-allow-import-graph.md](../../../docs/dogfood/cargo-allow-import-graph.md).

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo-allow check --profile spec-system --mode audit` | pass | #1767 |
| `cargo-allow check --mode no-new` | pass | #1767 |
| Main-repo import_graph audit JSON | committed | `docs/dogfood/receipts/cargo-allow-import-graph-repo.json` |
| Kiro fixture audit JSON | committed | `docs/dogfood/receipts/cargo-allow-import-graph-kiro.json` |
| Spec Kit fixture audit JSON | committed | `docs/dogfood/receipts/cargo-allow-import-graph-spec-kit.json` |
| xtask fixture audit JSON | committed | `docs/dogfood/receipts/cargo-allow-import-graph-xtask.json` |

## Non-Goals

- External `ripr` repository migration or R0 preflight execution.
- Full import mode product behavior (#1466).
- Release readiness or version bump authorization.

## Remaining Work

- **Done:** `portable-governance-import-dogfood`.
- **Blocked:** `portable-governance-ripr-preflight-r0`, external ripr adoption,
  full import mode (#1466).

## Claim Boundary

In-repository import_graph dogfood only. Does not prove external adoption,
semantic equivalence across ecosystems, or release readiness.
