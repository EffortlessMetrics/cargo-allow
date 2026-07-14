---
id: CARGO-ALLOW-CLOSEOUT-0045
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2227
merged_commit: 521d60d3
support_tier_impact: advisory
policy_impact: []
---

# Closeout: Report Read-Model Lifecycle Status

## Landed

- `audit`, `check`, and `diff` report renderers now consume lifecycle-projected
  outcomes for known matched findings.
- Expired and review-due matched entries appear with their projected status in
  summaries, output artifacts, receipts, and diff posture/failure counts.
- Only raw `matched` outcomes receive per-entry expiry/review overrides, so a
  mixed occurrence set retains its matched and new cardinalities.
- Evidence accounting continues to use raw matching outcomes, and dedicated
  baseline-debt accounting remains separate from generic outcome summaries.
- Candidate IDs, messages, finding indexes, unknown allow IDs, schemas, and
  stable status strings remain unchanged.

## Acceptance proof

- `cargo fmt --all -- --check`: passed locally and in hosted CI.
- `cargo test -p allow-report read_model --locked`: 3 passed.
- `cargo test -p cargo-allow --test audit_output --locked`: 7 passed.
- `cargo test -p cargo-allow --test e2e_occurrence_limit --locked`: 1 passed.
- `cargo test -p cargo-allow diff_render --locked`: 8 passed.
- `cargo test -p cargo-allow check_lane_posture --locked`: 5 passed.
- `cargo clippy -p cargo-allow --all-targets --locked -- -D warnings`: passed.
- Current-main no-new guard: passed after the implementation merge.
- Hosted PR #2230 `test`: passed, including workspace formatting, Clippy,
  workspace/unit/doc tests, docs, audit, no-new, and spec-system checks.
- Hosted UB Review: stopped at the known missing `MINIMAX_API_KEY` advisory
  preflight and emitted no code finding; tracked by #2084.
- PR #2230 merged to `main` as `521d60d3`.

## Validation boundary and remaining work

This closes the PR6D report-projection slice. Aggregate movement/posture,
evidence/provenance/repair convergence, occurrence fields across every read
artifact, the complete lifecycle corpus, and the later migration, portability,
release, scale, and real-repository gates remain open. The full local
`diff_output` target was bounded by timeout during exploration, but hosted
workspace CI ran its diff output tests successfully.
