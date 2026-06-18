---
id: CARGO-ALLOW-CLOSEOUT-0014
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

# Closeout: Generic Import Adapters (I2 Narrow Start)

## Summary

Execution closeout for CARGO-ALLOW-PLAN-0004 C10 narrow start (I2). Adds read-only
generic spec import adapters on top of the I1 import-root model:

- `.spec/` and `.rails/` configured import roots with recursive markdown discovery.
- Auto-discovery of repo-specific `.<repo>-spec/` directories at the repository root.
- Front-matter and body `linked_*` normalization into graph nodes, edges, provenance,
  confidence, and diagnostics.
- Spec-system doctor/audit/worklist `import_graph` extended via existing I1 wiring.
- Fixture-backed tests under `tests/fixtures/import/`.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test -p allow-policy import_roots` | pass | I2 PR |
| `cargo-allow check --profile spec-system --mode audit` | pass | I2 PR |
| `cargo-allow check --mode no-new` | pass | I2 PR |

## Non-Goals

- Kiro and Spec Kit full adapters (C8–C9 follow-up PR).
- xtask command registry adapter (C11).
- Full import mode (#1466) or release authorization.

## Remaining Work

- **Done:** `portable-governance-i2-import-adapters` narrow start (generic `.spec`/`.rails`/repo-spec).
- **Ready:** Kiro and Spec Kit adapters (C8–C9 follow-up PR).
- **Ready:** xtask command registry adapter (C11).

## Claim Boundary

Generic `.spec`/`.rails`/repo-spec read-only discovery only. Does not prove Kiro/Spec Kit
adapters, semantic equivalence, xtask registry parsing, or release readiness.
