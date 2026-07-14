---
id: CARGO-ALLOW-CLOSEOUT-0047
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2235
merged_commit: 8713ee72
support_tier_impact: advisory
policy_impact: []
---

# Closeout: Lifecycle Stale and Location Drift Corpus

## Landed

- Extended `crates/cargo-allow/tests/lifecycle_corpus.rs` with stale and
  location-drift policy entries.
- Asserted both statuses by allow ID across `list`, `explain`, `worklist`,
  `audit`, `check`, and `diff` JSON artifacts.
- Added an isolated mode fixture proving stale is advisory in `no-new` but
  blocking in `strict`, while location drift remains advisory in both modes.
- Preserved raw outcome identity and diagnostic details through the assertions.

## Acceptance proof

- `cargo fmt --all -- --check`: passed locally and in hosted CI.
- `cargo test -p cargo-allow --test lifecycle_corpus --locked`: 3 passed.
- `cargo clippy -p cargo-allow --test lifecycle_corpus --locked -- -D warnings`:
  passed.
- Current-main no-new guard: passed before merge.
- Hosted PR #2236 test: passed, including workspace formatting, Clippy,
  workspace/unit/doc tests, docs, audit, no-new, and spec-system checks.
- Hosted UB Review: stopped at the known missing `MINIMAX_API_KEY` advisory
  preflight and emitted no code finding; tracked by #2084.
- PR #2236 merged to `main` as `8713ee72`.

## Validation boundary and remaining work

This closes only the stale/location-drift PR7 corpus slice. Evidence health,
occurrence headroom, baseline debt, mirror divergence, change notes, refresh,
prune, aggregate movement/posture/provenance/repair convergence, and later
dogfood, migration, portability, release, scale, and real-repository gates
remain open.
