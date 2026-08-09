# CARGO-ALLOW-CLOSEOUT-0049

## Lane

- Work item: `ledger-coherence-pr7-lifecycle-corpus`
- Issue: #2241
- Implementation PR: #2242
- Merged commit: `7e6bc37a0de04ed724c57c3decc2282971bce3bb`
- Support tier: Stabilizing

## Delivered

The lifecycle corpus now includes one missing-evidence entry, one broken local
evidence reference, and one weak untyped reference. The subprocess proof keeps
these signals distinct across the existing read artifacts:

- `list` preserves matched entries and reports evidence counts, broken links,
  and weak references;
- `explain` preserves zero evidence and reports `local_file_missing` versus
  `unstructured` diagnostics;
- `worklist` routes `missing_evidence`, `broken_evidence_link`, and
  `weak_evidence_reference` separately;
- `audit`, `check`, and `diff` report `policy_missing_evidence`,
  `broken_evidence_links`, and `weak_evidence_references`.

This records the current contract without conflating evidence health with
match status.

## Proof

- `cargo fmt --all -- --check`
- `cargo clippy -p cargo-allow --test lifecycle_corpus --locked -- -D warnings`
- `cargo test -p cargo-allow --test lifecycle_corpus --locked`
- `cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md`
- Hosted PR test passed on #2242; CodeRabbit, Graphite reviews, GitGuardian,
  and mergeability checks passed.

The repository UB Review check remained blocked at its known missing
`MINIMAX_API_KEY` preflight; it produced no code finding.

## Claim boundary

This slice proves the existing evidence-health projections and their distinct
vocabulary. It does not redesign evidence validation, add evidence types, or
complete the remaining lifecycle movement/posture/repair convergence.

## Policy impact

None. The fixture evidence is test-local and does not broaden the repository
ledger.

## Follow-up

Continue PR7 with baseline debt, mirror divergence, change notes, and the
remaining cross-command convergence cases before opening PR8 dogfood.
