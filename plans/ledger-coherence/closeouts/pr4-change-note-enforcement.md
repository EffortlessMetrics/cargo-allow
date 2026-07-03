---
id: CARGO-ALLOW-CLOSEOUT-0022
kind: closeout
status: done
owner: repo-infra
created: 2026-07-03
linked_plan: CARGO-ALLOW-PLAN-0009
linked_proposal: CARGO-ALLOW-PROP-0008
linked_spec: CARGO-ALLOW-SPEC-0008
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0004
support_tier_impact: advisory
policy_impact:
  - .allow/artifacts/doc-artifacts.toml
  - .allow/revisions/
  - policy/allow.toml
---

# Closeout: GOAL-0004 PR 3+4 — Policy Revision Contract and `diff --require-change-note`

## Summary

Consolidates the revision-contract design (originally PR 3) with runtime
enforcement (PR 4) per issue #2075. Records the `.allow/revisions/` contract in
`CARGO-ALLOW-ADR-0002`, lands the `allow_policy::revision` parser, and enforces
`diff --require-change-note`: a governed weakening edit fails the diff unless a
matching revision record covers it. Implements #1475.

## Landed

- `CARGO-ALLOW-ADR-0002` — contract: note required for `worsened` /
  `review_required` posture deltas; canonical `policy_change_kind` vocabulary;
  structural `(allow_id, change_kind)` matching; durable, append-only records;
  transition-fingerprint guard for repeatable weakening kinds.
- `allow_core::POLICY_CHANGE_KIND_TOKENS` — canonical token list shared by
  `allow-policy` (validation) and `allow-diff` (bound by a parity test), resolving
  the `allow-diff` → `allow-policy` dependency direction without a parallel taxonomy.
- `allow_policy::revision` — `RevisionRecord`, `parse_revision_record[_at]`,
  `load_revision_records`, `covers` / `covers_transition`, `is_repeatable_change_kind`,
  `validate_revision_ledger`; 20 parse/validate/coverage tests.
- `.allow/revisions/revision.schema.json` (change_kinds as canonical enum;
  before/after fingerprints) and `.allow/revisions/README.md`.
- `cargo-allow diff --require-change-note` and `--write-change-note-template`:
  fold uncovered weakening cells into the diff failure decision; render a
  coverage section in human/markdown output; write a starter record template.
  7 enforcement tests, including the three #2075 acceptance scenarios.

## Validation

| Check | Result |
| --- | --- |
| `cargo test -p allow-policy revision` | pass (20) |
| `cargo test -p allow-diff policy_change_kind_tokens` | pass (parity) |
| `cargo test -p cargo-allow change_note` | pass (7) |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo fmt --all --check` | pass |
| `cargo-allow check --mode no-new` | pass |
| `cargo-allow check --profile spec-system --mode audit` | pass |

## Acceptance (#2075)

- Broaden/loosen a governed entry without a note → `diff --require-change-note`
  fails.
- Add a record covering `(allow_id, change_kind)` (+ fingerprint for repeatable
  kinds) → passes.
- Reuse a note across a second, later increase of a repeatable kind → fails
  (transition fingerprint no longer matches).

## Remaining

- **Ready:** `ledger-coherence-pr5-mutation-receipts`.
- JSON/SARIF structural surfacing of the change-note section and repository
  dogfood of the control loop (plan PR 8) are follow-ups.

## Claim Boundary

Lands the revision-record contract and `diff` enforcement. Does not mutate policy
automatically, prove end-to-end dogfood adoption, or authorize release.
