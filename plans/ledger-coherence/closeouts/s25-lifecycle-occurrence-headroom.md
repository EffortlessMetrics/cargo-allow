# CARGO-ALLOW-CLOSEOUT-0048

## Lane

- Work item: `ledger-coherence-pr7-lifecycle-corpus`
- Issue: #2238
- Implementation PR: #2239
- Merged commit: `b9d96ecef0105255e53d980608c82229db05fd87`
- Support tier: Stabilizing

## Delivered

The occurrence-headroom lifecycle slice now has a fixture with two matched
occurrences under `occurrence_limit = 3`. The shared helper reports the
remaining capacity (`limit - current_matches`) rather than the configured
limit. The worklist advisory message includes the configured limit, current
matches, and remaining capacity.

The subprocess corpus verifies the same entry through the existing read
artifacts:

- `list` exposes two current matches;
- `explain` exposes the configured limit and current match count;
- `worklist` emits `occurrence_headroom` with one remaining slot;
- `audit`, `check`, and `diff` expose the occurrence-headroom advisory count.

At-limit and unused entries remain excluded by the focused allow-report
coverage.

## Proof

- `cargo fmt --all -- --check`
- `cargo test -p allow-report occurrence_headroom_entries_count_matched_entries_below_limit --locked`
- `cargo test -p cargo-allow --bin cargo-allow occurrence_headroom_work_item_reports_remaining_capacity --locked`
- `cargo test -p cargo-allow --test lifecycle_corpus --locked`
- `cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md`
- Hosted PR test passed on #2239; CodeRabbit, GitGuardian, and mergeability checks passed.

The repository UB Review check remained blocked at its known missing
`MINIMAX_API_KEY` preflight; it produced no code finding.

## Claim boundary

This slice proves the existing occurrence-headroom projections and corrects
their remaining-capacity value. It does not redesign artifact schemas, add
per-entry headroom fields to every read artifact, or complete the remaining
lifecycle states and cross-command movement/posture/evidence convergence.

## Policy impact

None. The fixture policy entry is test-local and does not broaden the
repository ledger.

## Follow-up

Continue PR7 with evidence health, baseline debt, mirror divergence, change
notes, and movement/posture convergence. Keep broader shared read-model
contract changes in separately reviewable slices.
