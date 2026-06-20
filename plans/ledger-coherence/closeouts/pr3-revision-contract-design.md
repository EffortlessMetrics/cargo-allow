---
id: CARGO-ALLOW-CLOSEOUT-0022
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

# Closeout: GOAL-0004 PR 3 — Policy Revision Contract Design

## Summary

Design slice for the `.allow/revisions/` policy revision contract (#1475 design
slice). Resolves the five decisions deferred by `CARGO-ALLOW-SPEC-0008` and
lands a parse/validate stub in `allow-policy`. No enforcement: no command
requires or consumes revision notes yet.

## Landed

- `CARGO-ALLOW-ADR-0002` records the contract: which edits require a note
  (governed weakening = `worsened` / `review_required` posture delta),
  multi-entry coverage, structural diff matching on `(allow_id, change_kind)`,
  no merge-time expiry, and append-only records.
- `.allow/revisions/revision.schema.json` — record schema.
- `allow_policy::revision` — `RevisionRecord`, `parse_revision_record[_at]`,
  `RevisionRecord::covers`, `validate_revision_ledger`, and contract constants.
  15 fixture-backed parse/validate tests.
- `.allow/revisions/README.md` — directory contract explainer.

## Validation

| Check | Result |
| --- | --- |
| `cargo test -p allow-policy revision` | pass (15 tests) |
| `cargo fmt --all --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo-allow check --profile spec-system --mode audit` | pass |
| `cargo-allow check --mode no-new` | pass |

## Remaining

- **Ready:** `ledger-coherence-pr4-enforce-change-notes`.
- Enforcement (`diff --require-change-note`,
  `--write-change-note-template`) reads `posture_delta` / `change_kind` from the
  PR 2 diff path and consumes records via `RevisionRecord::covers`.

## Claim Boundary

Design contract plus parse/validate stub only. Does not enforce notes on any
command, mutate policy, prove diff coverage end-to-end, or authorize release.
