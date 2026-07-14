---
id: CARGO-ALLOW-CLOSEOUT-0039
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2159
merged_commit: 97a36b4a
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: Refresh Mutation Receipt

## Landed

- `refresh --format json` now embeds the shared
  `cargo-allow.mutation-receipt.v1` envelope.
- The receipt records deterministic canonical before/after fingerprints for
  the refreshed entry, repository/config provenance, write-vs-stdout result,
  and next verification commands.
- The existing refresh payload records drift, lifecycle preservation, and
  whether the operator approved the write; human output remains unchanged.
- The refresh schema, artifact sample, schema expectations, schema index, and
  active GOAL-0004 tracker now describe the third mutation-receipt adopter.

## Acceptance proof

- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p allow-report --all-targets --locked -- -D warnings`:
  passed.
- `cargo clippy -p cargo-allow --all-targets --locked -- -D warnings`: passed.
- `cargo test -p allow-report refresh --locked`: passed.
- `cargo test -p cargo-allow artifact --locked`: 97 passed.
- `cargo test -p cargo-allow refresh --locked`: passed.
- `cargo test -p cargo-allow --test schema_conformance --locked`: passed.
- Current-main required hosted `test` passed on the merged commit. UB Review
  stopped at the known missing `MINIMAX_API_KEY` preflight and emitted no code
  finding.
- `git diff --check`: passed before merge.

## Validation boundary and remaining work

This closes mutation-receipt slice 5C for `refresh`. Prune and migrate remain
open for the remaining mutation-receipt slices; the shared envelope is not yet
claimed as a complete all-command mutation contract.
