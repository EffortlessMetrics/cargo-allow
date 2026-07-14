---
id: CARGO-ALLOW-CLOSEOUT-0038
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2156
merged_commit: ad2c8a46
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: Propose Mutation Receipt

## Landed

- `propose --summary-format json` now embeds the shared
  `cargo-allow.mutation-receipt.v1` envelope.
- Generated allow IDs and canonical `sha256:v1` after-fingerprints are emitted
  in deterministic policy order; new entries carry null before-fingerprints.
- The receipt records repository/config provenance, write-vs-stdout result, and
  baseline-debt/check follow-up commands.
- The propose schema, artifact samples, schema expectations, schema index, and
  GOAL-0004 active tracker now describe the second mutation-receipt adopter.

## Acceptance proof

- `cargo fmt --all -- --check`: passed.
- `cargo check --workspace --locked`: passed.
- `cargo clippy -p allow-report --all-targets --locked -- -D warnings`: passed.
- `cargo clippy -p cargo-allow --all-targets --locked -- -D warnings`: passed.
- `cargo test -p allow-report --locked`: 222 passed.
- `cargo test -p cargo-allow --locked`: 97 artifact-contract tests passed.
- `cargo test -p cargo-allow propose --locked`: 13 passed.
- `cargo test --workspace --locked`: 2,056 passed across 45 suites.
- First-hour adoption and schema-conformance integration tests passed.
- The current-main no-new guard passed with no new findings.
- PR #2157 merged to `main` as `ad2c8a46`; required CI `test` passed. UB
  Review stopped at the known missing `MINIMAX_API_KEY` preflight and emitted
  no code finding.

## Validation boundary and remaining work

This closes mutation-receipt slice 5B for `propose`. Refresh, prune, and migrate
remain intentionally open for slices 5C–5D; the shared envelope is not yet
claimed as a complete all-command mutation contract.
