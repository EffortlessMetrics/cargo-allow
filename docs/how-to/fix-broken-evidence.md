# Fix Broken Evidence

Use this flow when policy points at missing or invalid local evidence.

## Find Broken Evidence

```bash
cargo-allow list --broken-evidence
cargo-allow worklist --item-kind broken_evidence_link --format json
```

For weak or untyped evidence references:

```bash
cargo-allow list --weak-evidence
cargo-allow worklist --item-kind weak_evidence_reference --format json
```

## Repair

For each item, choose one action:

- restore the referenced local file.
- replace the evidence reference with a current local path.
- convert weak evidence into a typed reference.
- remove the exception if the finding no longer exists.

Then verify the specific entry:

```bash
cargo-allow explain allow-0042
cargo-allow check --mode no-new
```

## Claim Boundary

cargo-allow validates local evidence paths it can see. It does not run tests,
ripr, unsafe-review, coverage tools, GitHub APIs, or network checks.

Reference: [Source exception ledger](../source-exception-ledger.md).
