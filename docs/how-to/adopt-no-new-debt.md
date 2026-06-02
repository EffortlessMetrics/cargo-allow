# Adopt No-New-Debt

Use this flow when a repository already has exception history and you want to
stop new unreceipted findings before fixing all existing debt.

## Steps

Audit the current source-tree surface:

```bash
cargo-allow audit
```

Generate a starting policy:

```bash
cargo-allow propose --write policy/allow.toml
```

Run the gate:

```bash
cargo-allow check --mode no-new
```

## Review the Generated Policy

Generated entries are adoption scaffolding, not approval. Review entries with:

```bash
cargo-allow list --baseline-debt
cargo-allow worklist --baseline-debt --format json
```

Close baseline debt by removing the finding, narrowing the selector, adding
owner/reason/lifecycle/evidence, or deleting stale policy. Do not convert
`baseline_debt` into a permanent approval just to pass CI.

## Claim Boundary

A passing no-new check means no new unreceipted findings were found in scanned
source-tree inventory. It does not prove the project is safe, buildable,
type-checked, or free of all possible exceptions.

Next: [Getting Started](../getting-started.md).
