---
id: CARGO-ALLOW-CLOSEOUT-0034
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2146
merged_commit: fb6b3554
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: Broad Add JSON Escaping

## Landed

- The broad `add --glob --summary-format json` renderer now escapes every
  interpolated string field before serialization.
- IDs, kinds, scopes, policy output paths, and actions preserve quotes,
  backslashes, and control characters without corrupting the JSON artifact.
- A regression test parses the rendered summary and verifies the original
  values survive exactly.

## Acceptance proof

- `cargo fmt --all -- --check`: passed.
- `cargo check --workspace --locked`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test -p cargo-allow render_broad_add_summary_json_escapes_string_fields --locked`: passed.
- `cargo test -p cargo-allow artifact_top_level_contract_tests --locked`: 5 passed.
- `cargo test -p allow-report --locked`: 221 passed.
- The no-new guard reported `status: passed` and `new: 0`.
- PR #2147 merged to `main` as `fb6b3554`; required CI test passed. UB Review
  stopped at the known missing `MINIMAX_API_KEY` preflight and emitted no code
  finding.

## Validation boundary and remaining work

This closes only broad add-summary string escaping. The broad summary still
needs a dedicated versioned schema contract, and the larger typed artifact
envelope plus success/policy-failure/execution-error parity remain under #1781.
