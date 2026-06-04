# Close Unsafe Migration Evidence

Use this flow after migrating a legacy unsafe allowlist into canonical
`policy/allow.toml`.

Unsafe migration closeout is stricter than ordinary evidence cleanup. A retained
unsafe exception should end with a specific owner, reason, classification,
lifecycle, selector, and evidence reference that explains the exact unsafe
boundary.

## Read Unsafe Signals

Start from the saved migration summary:

```bash
cargo-allow migrate \
  --repo-policy policy \
  --out target/cargo-allow/migrated-allow.toml \
  --summary-format json \
  --summary-output target/cargo-allow/migrate-summary.json
```

Inspect these fields first:

- `summary.unsafe_entries`: migrated unsafe receipts.
- `summary.unsafe_broken_evidence_links`: unsafe entries with local evidence or
  traceability links that no longer resolve.
- `summary.unsafe_weak_evidence_references`: unsafe entries with unstructured,
  empty, TODO, or unknown-prefix evidence.
- `summary.baseline_debt`: generated migration debt that still needs human
  review before it can become an approval.

## Classify Unsafe Evidence

Preserved review context:

- legacy IDs, paths, families, structural selector fields, and line or column
  hints.
- legacy owner, reason, classification, created, review-after, and expiry
  fields when present.
- legacy `evidence` or `covered_by` values.
- `legacy-policy:<id>` traceability back to the source policy.

Weak unsafe evidence:

- generated TODO evidence such as a missing unsafe-review or boundary-test
  reference.
- unstructured strings or unknown prefixes.
- derived facts that describe migration lineage but do not prove the unsafe
  boundary is still correct.

Broken unsafe evidence:

- local references such as `unsafe-review:`, `doc:`, `spec:`, `adr:`, `ripr:`,
  or `coverage:` where the target path is missing or invalid in the scanned
  source tree.

Generated unsafe baseline debt:

- `classification = "baseline_debt"` remains temporary.
- `owner = "unowned"` or generated reasons remain a review marker, not
  approval.
- do not normalize baseline debt into reviewed unsafe approval without replacing
  owner, reason, lifecycle, selector, and evidence with reviewed values.

## Run Focused Queues

Use the unsafe-specific queues before removing a legacy unsafe gate:

```bash
cargo-allow worklist --kind unsafe --item-kind broken_evidence_link --format json
cargo-allow worklist --kind unsafe --item-kind weak_evidence_reference --format json
cargo-allow worklist --kind unsafe --baseline-debt --format json
```

Use the generic queues only when closing the whole migrated policy:

```bash
cargo-allow worklist --broken-evidence --format json
cargo-allow worklist --weak-evidence --format json
cargo-allow worklist --baseline-debt --format json
```

## Close One Unsafe Item

For each work item:

1. Inspect the retained entry.

```bash
cargo-allow explain allow-0042
cargo-allow list --allow-id allow-0042 --format json
```

2. Choose the smallest honest repair:

- restore or retarget a broken local unsafe evidence file.
- replace TODO or weak evidence with a typed local reference.
- link the entry to the reviewed unsafe boundary, design note, boundary test, or
  equivalent local evidence artifact.
- keep `baseline_debt` until the unsafe receipt has reviewed owner, reason,
  classification, lifecycle, selector, and evidence.
- remove the retained exception if the unsafe finding is gone.

3. Re-run the item queue and the gate.

```bash
cargo-allow worklist --allow-id allow-0042 --format json
cargo-allow check --mode no-new
```

## Claim Boundary

cargo-allow validates source-tree references and routes unsafe evidence work. It
does not run rustc, Clippy, build scripts, proc macros, unsafe-review, coverage
tools, tests, Cargo metadata, GitHub APIs, or network checks, and it does not
prove unsafe code is correct.

Reference: [Migration evidence cookbook](migration-evidence-cookbook.md).
