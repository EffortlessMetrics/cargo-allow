---
id: CARGO-ALLOW-CLOSEOUT-0040
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2162
merged_commit: 30729290
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: Prune Mutation Receipt

## Landed

- `prune --stale --format json` now embeds the shared
  `cargo-allow.mutation-receipt.v1` envelope.
- The receipt records deterministically ordered removed allow IDs, canonical
  before-fingerprints, null after-fingerprints, repository/config provenance,
  preview-vs-write result, and repository-relative diff/check recovery
  commands.
- Empty previews emit empty receipt arrays with `result = stdout`; successful
  writes emit `result = written`.
- Human output remains unchanged. The prune schema, contract sample, schema
  tests, schema index, and active GOAL-0004 tracker describe the fourth
  mutation-receipt adopter.

## Acceptance proof

- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p allow-report --all-targets --locked -- -D warnings`:
  passed.
- `cargo clippy -p cargo-allow --all-targets --locked -- -D warnings`: passed.
- `cargo test -p allow-report prune --locked`: 4 passed.
- `cargo test -p cargo-allow prune --locked`: 29 passed.
- `cargo test -p cargo-allow artifact --locked`: 97 passed.
- `cargo test --workspace --locked`: 2,056 passed across 45 suites before the
  path-normalization follow-up; hosted required `test` passed on the corrected
  current head.
- Current-main no-new guard passed after merge.
- PR #2173 merged to `main` as `30729290`. UB Review stopped at the known
  missing `MINIMAX_API_KEY` preflight and emitted no code finding. CodeRabbit
  remained queued without an actionable finding.

## Validation boundary and remaining work

This closes the prune portion of mutation-receipt slice 5D. Migrate remains
the final mutation-receipt adopter; the shared read model, lifecycle corpus,
scanner coverage, migration fidelity, portability, cross-platform release,
scale, and real-repository gates remain open.
