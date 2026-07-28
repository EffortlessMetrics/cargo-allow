# Fix Broken Evidence

Use this flow when policy points at missing or invalid local evidence.

## Find Broken Evidence

```bash
cargo-allow list --broken-evidence
cargo-allow worklist --broken-evidence --format json
```

For weak or untyped evidence references:

```bash
cargo-allow list --weak-evidence
cargo-allow worklist --weak-evidence --format json
```

When `[requirements] evidence_required = true`, an entry must include at
least one recognized typed reference such as `test:...`, `doc:...`, or
`issue:...`. A non-empty free-form note does not satisfy that gate. Generated
`baseline_debt` entries remain eligible for their explicitly marked migration
placeholder evidence until they are reviewed.

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
