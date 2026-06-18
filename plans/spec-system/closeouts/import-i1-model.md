---
id: CARGO-ALLOW-CLOSEOUT-0013
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
| `cargo test -p allow-policy import_roots` | pass | #1761 merge `3912baa6` |
| `cargo test -p allow-policy spec_system::tests::parses_current_repository_active_goal_manifest` | pass | #1761 merge `3912baa6` |
| `cargo-allow check --profile spec-system --mode audit` | pass | post-merge proof |
| `cargo-allow check --mode no-new` | pass | post-merge proof |

## Non-Goals

- Kiro, Spec Kit, generic `.spec`/`.rails`, and xtask adapters (I2+).
- Full import mode (#1466) or external `ripr` migration.
- Release authorization or support-tier promotion.

## Remaining Work

- **Done:** `portable-governance-i1-import` (generic import-root model; #1761).
- **Ready:** `portable-governance-i2-import-adapters` (Kiro/Spec Kit/.rails/xtask adapters per allow-import-plan C8–C11).

## Claim Boundary

Generic import-root model and read-only discovery stub only. Does not prove ecosystem
adapters, semantic equivalence, or release readiness.
