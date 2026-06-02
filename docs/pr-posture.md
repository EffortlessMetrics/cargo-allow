# PR Posture

PR posture is cargo-allow's reviewer-facing summary of how a pull request
changes source-exception governance.

It is not a compiler result, a linter result, or a proof of safety. It compares
source-tree/source-syntax findings and policy entries across revisions, then
reports whether the PR added, removed, broadened, weakened, or improved the
ledger.

## Command

```bash
cargo-allow diff \
  --base origin/main \
  --format markdown \
  --output target/cargo-allow/pr-summary.md
```

For machine consumers:

```bash
cargo-allow diff \
  --base origin/main \
  --format json \
  --output target/cargo-allow/diff.json
```

## Net Posture

Diff output classifies the PR as:

- `worse`: current failures or failing policy weakening are present.
- `review-required`: source findings or policy changes need human review.
- `unchanged`: no source exception posture change was detected.
- `improved`: the PR removed findings, narrowed policy, or improved ledger
  health without introducing worse or review-required signals.

These labels are reviewer guidance for the scanned source-tree inventory. They
do not claim macro expansion, type information, build awareness, control-flow
analysis, data-flow analysis, proof adequacy, or coverage.

## What Counts as Worse

Common blocking signals include:

- new unreceipted source findings.
- selector precision decreases.
- source-tree scope broadening.
- occurrence-limit loosening.
- expiry or review-date extension.
- typed evidence removal.
- owner, reason, or classification removal.
- policy requirement loosening.
- new ignored inventory scopes.

See [Policy Weakening](policy-weakening.md) for the detailed weakening model.

## What Counts as Improvement

Common improvement signals include:

- removed source findings.
- removed stale policy.
- narrowed source-tree scope.
- increased selector precision.
- tightened occurrence limits.
- added typed evidence.
- added owner, reason, classification, or lifecycle fields.
- reduced generated `baseline_debt`.

## Claim Boundary

`cargo-allow diff` scans repository files directly. It does not require Cargo
metadata, compilation, rustc, Clippy, build scripts, proc macro expansion,
type analysis, MIR, proof-tool execution, network access, or repository code
execution.

The appropriate claim is:

```text
No new unreceipted findings were found in scanned source-tree inventory.
```

Not:

```text
No unsafe, panic, lint suppression, or policy exception exists.
```

## Related Docs

- [CI](ci.md)
- [Run in CI](how-to/run-in-ci.md)
- [Policy weakening](policy-weakening.md)
- [JSON schemas](schemas/README.md)
