---
id: CARGO-ALLOW-CLOSEOUT-0015
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

# Closeout: Kiro and Spec Kit Import Adapters (I2 C8–C9)

## Summary

Execution closeout for CARGO-ALLOW-PLAN-0004 C8–C9 (I2 follow-up). Adds read-only Kiro and
Spec Kit import adapters on top of the I1 import-root model:

- `.kiro/` recursive discovery of `requirements.md`|`bugfix.md`, `design.md`, and `tasks.md`.
- `.specify/` discovery of constitution, feature `spec.md`/`plan.md`/`tasks.md`, and
  `templates/` markdown.
- Front-matter and body `linked_*` normalization into graph nodes, edges, provenance,
  confidence, and diagnostics.
- Spec-system doctor/audit/worklist `import_graph` extended via existing I1 wiring.
- Fixture-backed tests under `tests/fixtures/import/kiro` and `tests/fixtures/import/spec-kit`.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test -p allow-policy import_roots` | pass | C8–C9 PR |
| `cargo-allow check --profile spec-system --mode audit` | pass | C8–C9 PR |
| `cargo-allow check --mode no-new` | pass | C8–C9 PR |

## Non-Goals

- xtask command registry adapter (C11).
- Full import mode (#1466) or release authorization.

## Remaining Work

- **Done:** `portable-governance-i2-kiro-spec-kit-adapters` (C8–C9).
- **Ready:** xtask command registry adapter (C11).

## Claim Boundary

Kiro and Spec Kit read-only path + front-matter discovery only. Does not prove semantic
equivalence, xtask registry parsing, steering-file normalization, or release readiness.
