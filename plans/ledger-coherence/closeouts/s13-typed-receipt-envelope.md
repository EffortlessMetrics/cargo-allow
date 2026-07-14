---
id: CARGO-ALLOW-CLOSEOUT-0036
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2152
merged_commit: e4ad303b
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: Typed Receipt Envelope

## Landed

- Receipt success, policy-failure, and execution-error renderers now build a
  typed `serde_json::Value` object before serialization.
- Receipt provenance, inventory completeness, counts, advisory signals,
  federation context, evidence repair queues, and source inventory retain their
  existing semantic fields.
- Ordered JSON maps provide deterministic object-key output; typed source
  inventory rows avoid embedding hand-built JSON fragments.
- Receipt-only manual metadata and federation string builders were removed from
  the shared JSON helper module.
- `serde_json` is now a runtime dependency of `allow-report`; it was already
  present in the workspace lockfile.

## Acceptance proof

- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p allow-report --all-targets --locked -- -D warnings`: passed.
- `cargo test -p allow-report --locked`: 222 passed.
- `cargo check --workspace --locked`: passed.
- `cargo test -p cargo-allow --test artifact_output --locked`: passed.
- `cargo test -p cargo-allow --test receipt_output --locked`: passed.
- `cargo test -p cargo-allow --test schema_conformance --locked`: passed.
- `cargo test -p cargo-allow --test first_hour_adoption --locked`: passed.
- The no-new guard produced a passing receipt with no new findings.
- The receipt golden test compares parsed JSON semantics, and a regression test
  verifies quotes, backslashes, newline, tab, and NUL diagnostics remain valid
  JSON.
- PR #2153 merged to `main` as `e4ad303b`; required CI `test` passed. UB Review
  stopped at the known missing `MINIMAX_API_KEY` preflight and emitted no code
  finding.

## Validation boundary and remaining work

This closes the typed receipt-envelope slice of #1781. Other command renderers
remain manually assembled and are not claimed converted. Mutation receipt slices
5B–5E, shared read-model convergence, and broader success/error schema coverage
remain open.
