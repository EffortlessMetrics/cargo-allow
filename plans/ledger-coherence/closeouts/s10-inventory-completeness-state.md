---
id: CARGO-ALLOW-CLOSEOUT-0033
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2143
merged_commit: 8f57270b
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: Inventory Completeness State

## Landed

- `allow-inventory` now classifies each inventory as `complete`, `scoped`,
  `fallback`, or `partial`.
- The classification propagates through `InventoryFacts` and the shared
  `InventoryContext` into JSON, human, Markdown, HTML, and doctor output.
- All committed artifact schemas publish the same enum, while the field stays
  optional for compatibility with older artifacts.
- Tests cover complete, scoped, fallback, and partial inventory states,
  including doctor and JSON disclosure.

## Acceptance proof

- `cargo fmt --all --check`: passed.
- `cargo test -p allow-inventory --locked`: 29 passed.
- `cargo test -p allow-report --locked`: 221 passed.
- `cargo test -p cargo-allow doctor --locked`: 24 passed.
- `cargo test -p cargo-allow artifact_schema_shared --locked`: 5 passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- The no-new guard passed with `completeness: scoped` and `new: 0` in its
  receipt.
- PR #2144 merged to `main` as `8f57270b`; required CI test passed. UB Review
  stopped at the known missing `MINIMAX_API_KEY` preflight and emitted no code
  finding.

## Validation boundary and remaining work

The full `cargo test --workspace --locked` command did not produce a result
within the ten-minute local tool limit. This slice does not add resource caps,
complete partial-scan controls, or replace manually assembled artifact
envelopes; those remain separate follow-ups under #1783 and the completion
roadmap.
