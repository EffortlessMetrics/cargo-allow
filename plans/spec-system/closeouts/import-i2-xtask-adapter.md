---
id: CARGO-ALLOW-CLOSEOUT-0016
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
---

# Closeout: xtask Command Registry Import Adapter (I2 C11)

## Summary

Execution closeout for CARGO-ALLOW-PLAN-0004 C11 (I2 follow-up). Adds read-only xtask
command registry discovery on top of the I1 import-root model:

- `xtask/` recursive discovery of `commands.toml`, `command-registry.toml`, and
  `registry.toml` registry files.
- `[[commands]]` table normalization into graph nodes, edges, provenance, confidence,
  and diagnostics without Rust dispatch parsing.
- Spec-system doctor/audit/worklist `import_graph` extended via existing I1 wiring.
- Fixture-backed tests under `tests/fixtures/import/xtask`.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test -p allow-policy import_roots` | pass | C11 PR |
| `cargo-allow check --profile spec-system --mode audit` | pass | C11 PR |
| `cargo-allow check --mode no-new` | pass | C11 PR |

## Non-Goals

- Rust xtask dispatch or `main.rs` match-arm parsing.
- Full import mode (#1466) or release authorization.

## Remaining Work

- **Done:** `portable-governance-i2-xtask-adapter` (C11).
- **Blocked:** full import mode (#1466) and external ripr adoption.

## Claim Boundary

xtask command registry TOML discovery only. Does not prove semantic equivalence,
Rust dispatch coverage, steering-file normalization, or release readiness.
