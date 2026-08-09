# CARGO-ALLOW-CLOSEOUT-0050

## Lane

- Work item: `ledger-coherence-pr7-lifecycle-corpus`
- Issue: #2244
- Implementation PR: #2245
- Merged commit: `0fa3313e126b9acb5be225d44630671c49f9a352`
- Support tier: Stabilizing

## Delivered

The lifecycle corpus now includes a generated `baseline_debt` entry and proves
its existing cross-command and mode semantics:

- `list` and `explain` project the entry as `baseline_debt`;
- `worklist` routes it as `baseline_debt`;
- `audit`, `check`, and `diff` preserve the baseline-debt advisory count and
  policy baseline-debt summary;
- `no-new` remains advisory while `strict` blocks the entry.

The tests preserve the current projection boundary: no-new reports a matched
outcome while the entry is advisory, whereas strict reports the lifecycle
baseline-debt status and fails.

## Proof

- `cargo fmt --all -- --check`
- `cargo test -p cargo-allow --test lifecycle_corpus --locked`
- `cargo clippy -p cargo-allow --test lifecycle_corpus --locked -- -D warnings`
- `cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md`
- Hosted PR test passed on #2245; Graphite AI review, GitGuardian, and
  mergeability checks passed.

The repository UB Review check remained blocked at its known missing
`MINIMAX_API_KEY` preflight. CodeRabbit was still processing at merge and was
treated as advisory for this test-only slice; no code finding was reported.

## Claim boundary

This slice proves the existing baseline-debt projections and mode posture. It
does not change baseline-debt policy semantics, add repository policy debt, or
complete the remaining lifecycle movement/posture/repair convergence.

## Policy impact

None. The generated baseline entry is test-local and does not broaden the
repository ledger.

## Follow-up

Continue PR7 with mirror divergence, change notes, and remaining movement,
posture, and repair convergence before opening PR8 dogfood.
