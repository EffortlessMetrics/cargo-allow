---
id: CARGO-ALLOW-CLOSEOUT-0044
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2214
merged_commit: d0bff084
support_tier_impact: advisory
policy_impact: []
---

# Closeout: Check Read-Model Lifecycle Status

## Landed

- `check` now projects each known allow entry through the shared lifecycle
  read model before applying the selected check mode.
- Expired matched entries fail `no-new` instead of passing as raw `matched`.
- Review-due matched entries remain advisory in `no-new` and fail `strict`,
  matching the existing `CheckMode` contract.
- `check` and `worklist` now consume one shared status projection; unknown
  allow IDs retain their raw outcome behavior.
- Projected allow IDs are borrowed from policy state rather than copied into
  a second owned key set.

## Acceptance proof

- `cargo fmt --all -- --check`: passed locally and in hosted CI.
- `cargo test -p cargo-allow check_lane_posture --locked`: 5 passed.
- `cargo test -p allow-match --locked`: 70 passed.
- `cargo clippy -p cargo-allow --all-targets --locked -- -D warnings`: passed.
- Current-main no-new guard: passed after the implementation merge.
- Hosted PR #2222 `test`: passed, including workspace formatting, Clippy,
  workspace/unit/doc tests, docs, audit, no-new, and spec-system checks.
- Hosted UB Review: stopped at the known missing `MINIMAX_API_KEY` advisory
  preflight and emitted no code finding; tracked by #2084.
- PR #2222 merged to `main` as `d0bff084`.

## Validation boundary and remaining work

This closes the PR6C check-status slice. Aggregate audit/check summaries,
movement and posture convergence, evidence/provenance/repair routing,
occurrence fields across every read artifact, the complete lifecycle corpus,
and the remaining diff/refresh/prune read-model convergence remain open under
PR6, PR7, and later completion gates. The disputed review suggestion to keep
raw `matched` for review-due entries was not applied because it would violate
the established `no-new`/`strict` mode contract.
