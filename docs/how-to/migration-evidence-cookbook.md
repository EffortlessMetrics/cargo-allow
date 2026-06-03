# Migration Evidence Cookbook

Use this guide after `cargo-allow migrate` writes a canonical policy and a
`cargo-allow.migrate.v1` summary.

## Read the Summary

Start with the saved migration summary:

```bash
cargo-allow migrate \
  --repo-policy policy \
  --out target/cargo-allow/migrated-allow.toml \
  --summary-format json \
  --summary-output target/cargo-allow/migrate-summary.json
```

Important fields:

- `summary.entries_with_evidence`: migrated allow entries that have at least
  one evidence value.
- `summary.evidence_entries`: total evidence values carried into the migrated
  policy.
- `summary.broken_evidence_links`: local evidence or link references that no
  longer resolve in the source tree.
- `summary.weak_evidence_references`: unstructured, empty, or unknown-prefix
  evidence and link references.
- `summary.baseline_debt`: generated migration debt that still needs human
  review.

## Classify Evidence

Treat migrated evidence as review context, not proof cargo-allow generated.

Preserved evidence usually comes from legacy `evidence` or `covered_by` fields.
Keep it when the reference still describes the retained exception.

Derived traceability such as `legacy-policy:<id>` points back to the source
policy. It is useful lineage, but it is not proof that the exception remains
correct.

Weak evidence includes unstructured notes and unknown-prefix facts such as
`generator:`, `interpreter:`, `binary:`, `argv_shape:`, `called_by:`,
`destination:`, `lane:`, or `auth_required:`. Replace these with recognized
local evidence or traceability when review closes the item.

Broken evidence uses recognized local prefixes such as `doc:`, `spec:`,
`adr:`, `ripr:`, `unsafe-review:`, or `coverage:`, but the target is missing or
invalid in the scanned source tree.

## Run Closeout Queues

Use the queue that matches the summary signal:

```bash
cargo-allow worklist --item-kind broken_evidence_link --format json
cargo-allow worklist --item-kind weak_evidence_reference --format json
cargo-allow worklist --item-kind baseline_debt --format json
```

For unsafe-specific evidence debt:

```bash
cargo-allow worklist --kind unsafe --item-kind broken_evidence_link --format json
cargo-allow worklist --kind unsafe --item-kind weak_evidence_reference --format json
```

## Close One Item

For each work item:

1. Inspect the retained entry.

```bash
cargo-allow explain allow-0042
cargo-allow list --allow-id allow-0042 --format json
```

2. Choose the smallest honest repair:

- restore or retarget a broken local evidence file.
- replace a weak string with a recognized evidence or traceability prefix.
- keep `baseline_debt` until owner, reason, classification, lifecycle,
  selector, and evidence are reviewed.
- remove the retained exception if the finding is gone.

3. Re-run the focused queue and the no-new gate.

```bash
cargo-allow worklist --allow-id allow-0042 --format json
cargo-allow check --mode no-new
```

## Claim Boundary

Migration evidence closeout is ledger work. cargo-allow does not execute legacy
xtasks, build the project, invoke Cargo metadata, run rustc or Clippy, expand
macros, run unsafe-review, check coverage, or validate external proof tools.

Reference: [Migration from xtask](../migration-from-xtask.md).
