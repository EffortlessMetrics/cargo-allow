---
id: CARGO-ALLOW-CLOSEOUT-0042
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2197
merged_commit: fc1c00f5
support_tier_impact: advisory
policy_impact: []
---

# Closeout: Shared List and Worklist Lifecycle Status

## Landed

- `allow-report::ledger_read_state` is the shared lifecycle projection for
  policy entries and their match outcomes.
- `list` now consumes the shared projection instead of maintaining a private
  status precedence implementation.
- `worklist` uses the same projected status, keeping matched outcomes
  actionable when an entry is expired or review-due while preserving existing
  baseline-debt cardinality.
- The projection retains matched occurrence count and configured occurrence
  limit for the next read-surface convergence slices.
- Worklist outcomes are pre-grouped by allow ID and projected once per entry,
  avoiding a nested outcome scan for every work item.
- Expired, review-due, matched, stale, and baseline-debt regressions are
  covered without changing the existing list/worklist schema vocabulary.

## Acceptance proof

- `cargo fmt --all -- --check`: passed on current `main`.
- `cargo clippy -p cargo-allow --all-targets --locked -- -D warnings`: passed
  on current `main`.
- `cargo test -p allow-report --locked`: 223 passed on current `main`.
- `cargo test -p cargo-allow worklist --locked`: 113 passed on current `main`.
- `cargo test -p cargo-allow list_rows --locked`: 22 passed on current `main`.
- Current-main no-new guard: passed after merge.
- Hosted CI `test`: passed for PR #2203.
- Hosted UB Review: stopped at the known missing `MINIMAX_API_KEY` advisory
  preflight and emitted no code finding; tracked by #2084.
- PR #2203 merged to `main` as `fc1c00f5`.

## Validation boundary and remaining work

The unfiltered `cargo test -p cargo-allow --locked` package run was allowed
ten minutes on current `main` and did not produce a test result while running
subprocess-heavy saved-artifact fixtures. The focused worklist and list-row
targets, allow-report suite, Clippy, formatting, and no-new guard provide the
bounded proof for this slice; the full package run remains a validation and CI
economics gap.

This closes only the first PR6 read-model slice. Movement/posture, evidence,
ledger provenance, repair routing, the complete lifecycle corpus, dogfood,
operator documentation, scanner coverage, migration, portability, release,
scale, and real-repository adoption gates remain open. Umbrella issue #2197
stays in progress.
