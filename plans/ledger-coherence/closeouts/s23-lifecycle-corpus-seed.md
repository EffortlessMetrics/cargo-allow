---
id: CARGO-ALLOW-CLOSEOUT-0046
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2232
merged_commit: b6ecccfc
support_tier_impact: advisory
policy_impact: []
---

# Closeout: Lifecycle Corpus Seed

## Landed

- Added `crates/cargo-allow/tests/lifecycle_corpus.rs` as the first PR7
  subprocess corpus slice.
- Expired and review-due matched entries are checked by allow ID across the
  `list`, `explain`, `worklist`, `audit`, `check`, and `diff` JSON artifacts.
- A separate review-due-only fixture proves that `no-new` remains advisory and
  `strict` is blocking.
- Temporary Git fixture commits disable ambient GPG signing, and the test
  argument tables avoid introducing new source-tree scan debt.

## Acceptance proof

- `cargo fmt --all -- --check`: passed locally and in hosted CI.
- `cargo test -p cargo-allow --test lifecycle_corpus --locked`: 2 passed.
- `cargo clippy -p cargo-allow --test lifecycle_corpus --locked -- -D warnings`:
  passed.
- Current-main no-new guard: passed after the merged test landed.
- Hosted PR #2233 test: passed, including workspace formatting, Clippy,
  workspace/unit/doc tests, docs, audit, no-new, and spec-system checks.
- Hosted UB Review: stopped at the known missing `MINIMAX_API_KEY` advisory
  preflight and emitted no code finding; tracked by #2084.
- PR #2233 merged to `main` as `b6ecccfc`.

## Validation boundary and remaining work

This closes only the expired/review-due PR7 seed. The full matched, stale,
location-drift, headroom, evidence, baseline-debt, mirror-divergence, and
change-note corpus remains open, as do aggregate movement/posture,
evidence/provenance/repair convergence, operator dogfood, migration,
portability, release, scale, and real-repository gates.
