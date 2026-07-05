---
id: CARGO-ALLOW-CLOSEOUT-0022
kind: closeout
status: done
owner: repo-infra
created: 2026-07-05
linked_plan: CARGO-ALLOW-PLAN-0009
linked_proposal: CARGO-ALLOW-PROP-0008
linked_spec: CARGO-ALLOW-SPEC-0008
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0004
support_tier_impact: advisory
policy_impact: []
---

# Closeout: GOAL-0004 PR 3+4 — Revision Contract and Change-Note Enforcement (reconciliation)

## Summary

Reconciles the active-goal tracker with what actually landed on `main`. Three
design-only PRs (#1772, #1773, #1774) proposing the `.allow/revisions/`
contract were superseded by issue #2075, which was implemented directly as a
runtime-enforcement PR (#2097, commit `bc1ae6c`) rather than through a separate
design slice. This closeout records that landing; it does not add new
behavior.

## Landed (via #2097)

- `cargo-allow diff --require-change-note`: fails the diff when a policy edit
  with severity `Fail` or `Review` lacks a matching revision note in
  `.allow/revisions/` (default; `--revisions-dir` overrides). Matching is
  structural on `(allow_id, change_kind)` using the canonical
  `policy_change_kind` vocabulary. Improvements are exempt.
- `--revisions-dir` flag (defaults to `.allow/revisions/`); `check_change_notes`
  loads and matches `.toml` revision notes.

## Known gap (not yet landed)

Per #2097's own stated scope, two items from the original design remain open:

- **`after_fingerprint` pinning for repeatable-weakening kinds** — without it, a
  single note authorizing e.g. one `occurrence_limit_loosened` transition can be
  silently reused to justify a *later, independent* increase on the same entry.
  This is the "repeatable-weakening loophole" identified in the closed #1774
  design.
- **A machine-readable `.allow/revisions/*.toml` schema file** — the record
  contract today is documented only in the diagnostic message, not published as
  `docs/schemas/*.json`.

Both remain candidates for a future slice; not addressed here to keep this
closeout a pure reconciliation record.

## Validation

Reconciliation only; no code changes. Existing CI on `main` already covers
#2097's enforcement path (`cargo test -p cargo-allow`, `cargo-allow diff
--require-change-note` acceptance behavior).

## Remaining

- **Ready:** `ledger-coherence-pr5-mutation-receipts` (PR 5A landed alongside
  this reconciliation; see `CARGO-ALLOW-CLOSEOUT-0023`).

## Claim Boundary

Governance reconciliation only. Does not add, remove, or modify runtime
behavior; does not close the fingerprint-pinning or schema-file gaps noted
above.
