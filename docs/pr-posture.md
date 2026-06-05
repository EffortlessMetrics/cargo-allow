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

## What Needs Attention

Common attention signals include:

- new unreceipted source findings.
- selector precision decreases.
- source-tree scope broadening.
- occurrence-limit loosening.
- typed evidence removal.
- added or removed evidence and traceability links.
- owner, reason, or classification removal.
- policy requirement loosening.
- new ignored inventory scopes.
- expiry or review-date extension.

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

## Examples

Use these examples to route review quickly:

| PR change | Typical posture | Reviewer action |
|---|---|---|
| Evidence removed from `allow-0031` | `worse` | Restore typed evidence or explain why the receipt should remain without it. |
| `allow-0042` changed from `path = "src/lib.rs"` to `glob = "src/**/*.rs"` | `worse` | Require a narrower scope or a reviewed reason for the broader source-tree surface. |
| `allow-0017.expires` moved from `2026-07-01` to `2026-12-31` | `review-required` | Confirm the lifecycle extension was deliberate and documented. |
| Selector fields were removed, such as `container` or `callee` | `worse` | Restore structural identity fields or justify the lower selector precision. |
| Selector fields were added, such as `container`, `callee`, or snippet identity | `improved` | Keep the narrower receipt if it still matches the intended finding. |
| A stale `allow-0020` entry was removed | `improved` | Confirm the removal is intentional and no current finding needs that receipt. |
| Generated `baseline_debt` was removed because the finding was fixed | `improved` | Confirm the debt was reduced, not reclassified as reviewed approval. |
| Generated `baseline_debt` was reclassified as reviewed policy | `worse` | Require real owner, reason, lifecycle, and evidence instead of laundering generated debt. |

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
- [Review PR posture](how-to/review-pr-posture.md)
- [Policy weakening](policy-weakening.md)
- [JSON schemas](schemas/README.md)
