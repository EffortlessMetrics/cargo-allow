---
id: CARGO-ALLOW-CLOSEOUT-0035
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2149
merged_commit: 817ea56b
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: Receipt Result Contracts

## Landed

- The committed artifact sample corpus now includes passed, policy-failed, and
  execution-error receipt outputs from the real renderers.
- The existing nested schema validator runs against all three result classes.
- Error receipts are asserted to preserve `status = error`, `failed = true`,
  and the escaped diagnostic payload.

## Acceptance proof

- `cargo fmt --all`: passed.
- `cargo check --workspace --locked`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test -p cargo-allow artifact_top_level_contract_tests --locked`: 5 passed.
- `cargo test -p cargo-allow receipt_result_classes_preserve_status_and_error_diagnostic --locked`: passed.
- The no-new guard reported `status: passed` and `new: 0`.
- PR #2150 merged to `main` as `817ea56b`; required CI test passed. UB Review
  stopped at the known missing `MINIMAX_API_KEY` preflight and emitted no code
  finding.

## Validation boundary and remaining work

This closes receipt result-class schema coverage only. It does not yet provide
typed serializers for every command, a dedicated broad-add schema, or complete
success/policy-failure/execution-error contracts for every command under #1781.
