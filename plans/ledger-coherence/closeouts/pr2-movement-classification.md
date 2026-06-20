---
id: CARGO-ALLOW-CLOSEOUT-0021
kind: closeout
status: done
owner: repo-infra
created: 2026-06-20
linked_plan: CARGO-ALLOW-PLAN-0009
linked_proposal: CARGO-ALLOW-PROP-0008
linked_spec: CARGO-ALLOW-SPEC-0008
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0004
support_tier_impact: advisory
policy_impact: []
---

# Closeout: GOAL-0004 PR 2 — Movement Classification in Diff

## Summary

Orthogonal movement and posture_delta on every diff row; dual summary counts in
JSON diff artifacts; consistent projection across human, Markdown, JSON, receipt,
and worklist vocabulary. Implements #1471.

## Landed

- `allow-diff::movement` classification helpers and ledger movement summary counts.
- `DiffFindingChange` / `DiffPolicyChange` row fields and JSON dual-summary blocks.
- Schema updates in `common.v1.json` and `report.schema.json` (additive diff-level
  blocks; row fields required in schema).
- Human and Markdown diff renderers surface movement/posture attribution.

## Validation

| Check | Result |
| --- | --- |
| `cargo test -p allow-diff` | pass |
| `cargo test -p allow-report` | pass |
| `cargo test -p cargo-allow` | pass |
| `cargo-allow check --mode no-new` | pass |
| `cargo-allow check --profile spec-system --mode audit` | pass |

## Remaining

- **Ready:** `ledger-coherence-pr3-revision-contract-design`.
- Revision-note enforcement remains PR 4.

## Claim Boundary

Diff and PR-posture vocabulary alignment with additive JSON fields. Does not
enforce change notes, unify mutation receipts, or authorize release cut.
