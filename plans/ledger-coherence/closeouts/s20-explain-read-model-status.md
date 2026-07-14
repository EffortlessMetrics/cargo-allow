---
id: CARGO-ALLOW-CLOSEOUT-0043
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2211
merged_commit: 87de4d69
support_tier_impact: advisory
policy_impact: []
---

# Closeout: Explain Read-Model Lifecycle Status

## Landed

- Explain human and JSON summary status now use the shared
  `allow_report::ledger_read_state` projection.
- Expired and review-due matched entries report the same lifecycle status as
  `list` and `worklist`.
- Baseline-debt explain summaries use the canonical `baseline_debt` status
  instead of the former raw-outcome `matched` fallback.
- Raw match-outcome details, artifact field names, and stable `MatchStatus`
  strings remain unchanged.

## Acceptance proof

- `cargo fmt --all -- --check`: passed on current `main`.
- `cargo test -p allow-report --locked`: 225 passed on current `main`.
- `cargo test -p cargo-allow explain --locked`: 35 passed on current `main`.
- `cargo clippy -p cargo-allow --all-targets --locked -- -D warnings`: passed.
- Current-main active-goal parser: 1 passed.
- Current-main no-new guard: passed after merge.
- Hosted CI `test`: passed for PR #2212.
- Hosted UB Review: stopped at the known missing `MINIMAX_API_KEY` advisory
  preflight and emitted no code finding; tracked by #2084.
- PR #2212 merged to `main` as `87de4d69`.

## Validation boundary and remaining work

This closes the PR6B explain-status slice. Explain evidence diagnostics,
movement/posture, ledger provenance, repair routing, occurrence fields in
explain artifacts, the complete lifecycle corpus, and the remaining audit,
check, diff, refresh, and prune read-model convergence remain open under PR6,
PR7, and later completion gates.
