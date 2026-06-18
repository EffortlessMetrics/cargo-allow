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
and no-panic-baseline evidence/lifecycle import (B2, #1691).

This closeout records landed planning and characterization work only. It does
not claim full xtask replacement, side-by-side dogfood parity, or the `0.2.0`
milestone.

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

## Remaining Work

- B3 fixture matrix under `tests/fixtures/migration/`.
- B5 side-by-side dogfood receipts.
- B6 import/parity issue disposition (#1466, #1470).
- B7 `0.2.0` migration parity release notes.

## Claim Boundary

Incremental slice evidence only. `partial` lane status in the gap inventory is
not a parity claim.
