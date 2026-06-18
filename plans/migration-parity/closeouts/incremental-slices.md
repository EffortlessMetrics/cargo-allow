---
id: CARGO-ALLOW-CLOSEOUT-0002
kind: closeout
status: done
owner: repo-infra
created: 2026-06-18
linked_plan: CARGO-ALLOW-PLAN-0002
linked_proposal: CARGO-ALLOW-PROP-0002
linked_spec: CARGO-ALLOW-SPEC-0002
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0002
support_tier_impact: advisory
policy_impact:
  - policy/doc-artifacts.toml
  - .codex/goals/active.toml
---

# Closeout: Migration Parity Incremental Slices

## Summary

Slice closeout for goal registration (#1687), gap inventory reconciliation (B1),
no-panic-baseline evidence/lifecycle import (B2, #1691), migration fixture matrix
(B3, #1693), closeout routing (B4, #1695), panic-baseline dogfood receipt (B5,
#1697), and import/parity disposition (B6).

This closeout records landed planning and characterization work only. It does
not claim full xtask replacement, side-by-side dogfood parity across all lanes,
or the `0.2.0` milestone.

## Landed Slices

### Goal registration (migration-parity-pr-001)

- Archived CARGO-ALLOW-GOAL-0001 and registered CARGO-ALLOW-GOAL-0002 with
  migration parity proposal, spec, plan, and PR queue artifacts (#1687).

### Gap inventory (migration-parity-b1)

- Reconciled `plans/migration-parity/gap-inventory.md` from
  `allow-policy-legacy` characterization tests and open issues #1466/#1470.

### No-panic baseline import (migration-parity-b2)

- Preserved owner, reason, evidence, and `covered_by` on no-panic-baseline
  migration (#1691).
- Entries without evidence keep visible `baseline_debt` markers.
- Lifecycle fix: `review_after` without `expires` no longer inherits an
  unintended expires value.

### Migration fixture matrix (migration-parity-b3)

- Added `tests/fixtures/migration/` characterization across supported compat
  kinds (#1693, merge `cd0ab7b`).
- Table-driven `migration_fixture_matrix_tests.rs` covers parse preservation,
  metadata, evidence, lifecycle, occurrence limits, deterministic reruns, and
  policy-dir batch import.

### Closeout routing (migration-parity-b4)

- `cargo-allow.migrate.v1` summaries route baseline_debt, evidence repair, and
  phased `next_queues` (#1695, merge `64832c5`).
- Documented in `docs/how-to/migration-evidence-cookbook.md`.

### Panic-baseline dogfood receipt (migration-parity-b5)

- In-repository side-by-side receipt for one scoped panic-baseline slice
  (#1697, merge `26a6873`).
- Artifacts under `docs/dogfood/`.

### Import/parity disposition (migration-parity-b6)

- #1470 foreign-dialect discovery closed (#1699 merge `53ea19aa`, #1700).
- #1466 bespoke semantic-selector import remains open umbrella; governance
  split deferred to `adoption-substrate-pr-005`.

## Related Landed Work (outside B1–B6 scope)

- D2 structural identity refactor-pair matrix (#1701, merge `2165848`).
- Release automation hardening (#1703–#1705) — dormant; not active release lane.
- #1478 spec-system profile hygiene closed (#1706).

## Remaining Work

- Structural identity D1–D8 complete (`adoption-substrate-pr-010` through
  `adoption-substrate-pr-014`, plus D8 docs in CARGO-ALLOW-CLOSEOUT-0003); see
  [docs/identity.md](../../docs/identity.md).
- B7 `0.2.0` migration parity release notes — after remaining parity proof.
- Per-lane `partial` rows in gap inventory — side-by-side dogfood and full lane
  acceptance still open.

## Claim Boundary

Incremental slice evidence only. `partial` lane status in the gap inventory is
not a parity claim. `0.1.10` release cut is deferred pending adoption/cleanup
lane completion and explicit release authorization.

