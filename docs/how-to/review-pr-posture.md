# Review PR Posture

Use this guide when a pull request changes source exceptions or policy
receipts and you need to decide what a reviewer should do next.

> Maturity: `diff` is Stable in published `0.1.11` and Stabilizing on current
> main. See the [command maturity table](../status/SUPPORT_TIERS.md#command-maturity).

## Generate the Summary

Run the reviewer-facing Markdown summary against the intended base:

```bash
cargo-allow diff \
  --base origin/main \
  --format markdown \
  --output target/cargo-allow/pr-summary.md
```

For automation, save the JSON report:

```bash
cargo-allow diff \
  --base origin/main \
  --format json \
  --output target/cargo-allow/diff.json
```

## Read Net Posture First

Start with `diff.net_posture` or the Markdown `Net posture` row:

- `worse`: block until current failures or failing policy changes are fixed,
  narrowed, or receipted.
- `review-required`: inspect the listed source or policy changes before
  merging.
- `improved`: verify the cleanup was intentional and keep the narrower posture.
- `unchanged`: no source-exception posture change was detected in the scanned
  source-tree inventory.

The reviewer action is guidance for source-tree ledger posture. It is not a
compiler, build, unsafe-proof, test-adequacy, or coverage claim.

## Check Source Findings

Use `new_findings` and `removed_findings` to separate attention from cleanup.

New source findings mean the PR introduced an unreceipted source exception in
the scanned inventory. Review whether the finding should be removed, narrowed,
or intentionally receipted with owner, reason, classification, lifecycle, and
evidence.

Removed source findings are usually improvements. Confirm they were removed by
the PR and not hidden by a broader ignored path or weaker policy.

When present, `finding_changes[].source_package` is source-derived routing
context. It is not Cargo metadata.

When present, `finding_changes[].line` and `finding_changes[].column` are
review/navigation hints. They are not stable finding identity.

When present, `finding_changes[].identity` shows the source-syntax structural
identity used for posture matching. Human and Markdown summaries show this as a
compact `Identity` column; JSON reports keep the structured object.

## Check Policy Changes

Read policy sections in this order:

1. `Policy Failures`
2. `Policy Review Required`
3. `Policy Improvements`

Failing policy changes include scope broadening, selector precision loss,
occurrence-limit loosening, typed evidence removal, owner or reason removal,
baseline-debt normalization, requirement loosening, and new ignored inventory
scope.

Review-required changes include equal-precision retargeting, metadata changes,
lifecycle extensions, generated scope changes, and weak evidence or
traceability changes that cannot be validated locally.

Improvements include narrowed scope, increased selector precision, tightened
limits, added typed evidence, restored owner or reason fields, and reduced
generated `baseline_debt`.

## Check Structural Deltas

Structural delta counts describe scope and selector movement:

- `scope_broadened`
- `scope_changed`
- `scope_narrowed`
- `selector_changed`
- `selector_precision_decreased`
- `selector_precision_increased`

Use the detailed `policy_changes` rows for before/after scope, selector
identity, and selector precision details. These summary counts are shortcuts for
existing row kinds; they do not replace row-level review.

## Check Evidence Health

Evidence health counts describe the compared head policy:

- `broken_evidence_links`: recognized local evidence or links that do not
  resolve in the source tree.
- `missing_evidence`: retained policy entries that still lack evidence.
- `weak_evidence_references`: unstructured, empty, or unknown-prefix evidence
  and traceability strings.

Evidence delta counts describe what the PR changed:

- `evidence_added`
- `weak_evidence_added`
- `broken_evidence_added`
- `evidence_removed`
- `evidence_removal_failures`
- `evidence_removal_review_items`
- `evidence_removal_improvements`
- `link_added`
- `weak_link_added`
- `broken_link_added`
- `link_removed`
- `link_removal_failures`
- `link_removal_review_items`
- `link_removal_improvements`

Use the detailed `policy_changes` rows for severity, message, and exact
added/removed values. The weak and broken added counts are shortcuts for
review/fail evidence introductions. Removal failure, review, and improvement
counts are shortcuts for existing row severities; they do not replace row-level
review.

## Route Follow-Up Work

When the PR summary reports evidence health or baseline debt, route follow-up
with focused worklists:

```bash
cargo-allow worklist --broken-evidence --format json
cargo-allow worklist --weak-evidence --format json
cargo-allow worklist --missing-evidence --format json
cargo-allow worklist --item-kind baseline_debt --format json
```

For one retained entry:

```bash
cargo-allow explain allow-0042 --format json
cargo-allow worklist --allow-id allow-0042 --format json
```

## Close the Review

Accept the PR posture only when the remaining claim is precise:

```text
No new unreceipted findings were found in scanned source-tree inventory.
```

Do not upgrade that to a claim that no unsafe code, panic-family call, lint
suppression, generated file, or policy exception exists. cargo-allow did not
invoke Cargo metadata, build the project, run rustc or Clippy, expand macros,
analyze types or MIR, execute proof tools, or validate coverage.

Reference: [PR posture](../pr-posture.md).
