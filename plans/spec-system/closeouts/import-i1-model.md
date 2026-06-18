---
id: CARGO-ALLOW-CLOSEOUT-0013
kind: closeout
status: draft
owner: repo-infra
created: 2026-06-18
linked_plan: CARGO-ALLOW-PLAN-0004
linked_proposal: CARGO-ALLOW-PROP-0004
linked_spec: CARGO-ALLOW-SPEC-0004
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0003
support_tier_impact: advisory
policy_impact:
  - .allow/profiles/spec-system.toml
  - docs/schemas/spec-system.schema.json
---

# Closeout: Generic Import-Root Model (I1)

## Summary

Execution closeout for CARGO-ALLOW-PLAN-0004 C5–C7 (I1). Adds a generic import-root
abstraction with read-only discovery:

- `[import_roots]` config on the spec-system profile with owned/imported/legacy/generated roles.
- `allow-policy::import_roots` normalizes graph nodes, edges, provenance, confidence, and diagnostics.
- Spec-system doctor/audit/worklist emit `import_graph` and route `broken_import` work items.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test -p allow-policy import_roots` | pass | I1 PR proof |
| `cargo test -p allow-policy spec_system::tests::parses_current_repository_active_goal_manifest` | pass | I1 PR proof |
| `cargo-allow check --profile spec-system --mode audit` | pass | `target/cargo-allow/spec-system.json` |
| `cargo-allow check --mode no-new` | pass | `target/cargo-allow/check.receipt.json` |

## Non-Goals

- Kiro, Spec Kit, generic `.spec`/`.rails`, and xtask adapters (I2+).
- Full import mode (#1466) or external `ripr` migration.
- Release authorization or support-tier promotion.

## Claim Boundary

Generic import-root model and read-only discovery stub only. Does not prove ecosystem
adapters, semantic equivalence, or release readiness.
