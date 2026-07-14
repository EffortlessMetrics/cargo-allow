---
id: CARGO-ALLOW-CLOSEOUT-0041
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2182
merged_commit: 731afb19
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: Migrate Mutation Receipt

## Landed

- `migrate --summary-format json` now embeds the shared
  `cargo-allow.mutation-receipt.v1` envelope.
- The receipt records the migrate operation, repository/output provenance,
  deterministic migrated allow IDs, aligned before/after fingerprint arrays,
  the written result, and post-migration verification commands.
- Migration-specific counts, evidence repair queues, legacy follow-up queues,
  and closeout semantics remain in the existing command payload rather than
  being duplicated in the shared envelope.
- The migrate schema, contract sample, schema expectations, schema index, and
  real subprocess artifact fixture now require and validate the receipt.

## Acceptance proof

- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p allow-report --all-targets --locked -- -D warnings`:
  passed.
- `cargo clippy -p cargo-allow --all-targets --locked -- -D warnings`: passed.
- `cargo test -p allow-report migrate --locked`: 7 passed.
- `cargo test -p cargo-allow migrate --locked`: 32 passed.
- `cargo test -p cargo-allow artifact --locked`: 97 passed.
- Real saved migration subprocess fixture: 1 passed with schema-valid receipt.
- `cargo test --workspace --locked`: 2,056 passed across 45 suites.
- Current-main no-new guard: passed before merge.
- Hosted required `test`: passed on the corrected PR head. UB Review stopped
  at the known missing `MINIMAX_API_KEY` preflight and emitted no code finding.
- PR #2187 merged to `main` as `731afb19`.

## Validation boundary and remaining work

This closes the final mutation-receipt adopter in GOAL-0004 PR 5. The receipt
does not yet provide lossless migration records, canonical policy digests, or a
shared read model. PR 6 read-surface convergence is now the next active lane;
lifecycle consistency, scanner coverage, portability, cross-platform release,
scale, and real-repository adoption gates remain open.
