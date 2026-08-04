# Adopt No-New-Debt

Use this flow when a repository already has exception history and you want to
stop new unreceipted findings before fixing all existing debt.

> Maturity: `propose` is Stable in published `0.1.11` and Stabilizing on
> current main. See the [command maturity table](../status/SUPPORT_TIERS.md#command-maturity).

## Steps

Audit the current source-tree surface:

```bash
cargo-allow audit
```

Generate a starting policy:

```bash
cargo-allow propose \
  --write policy/allow.toml \
  --summary-format json \
  --summary-output target/cargo-allow/propose.json
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

When `propose` writes a JSON summary, use `follow_up_queues` to route the next
work. Generated unsafe baseline entries are also routed toward weak-evidence
cleanup because the TODO evidence placeholder is not proof.

Close baseline debt by removing the finding, narrowing the selector, adding
owner/reason/lifecycle/evidence, or deleting stale policy. Do not convert
`baseline_debt` into a permanent approval just to pass CI.

## Claim Boundary

A passing no-new check means no new unreceipted findings were found in scanned
source-tree inventory. It does not prove the project is safe, buildable,
type-checked, or free of all possible exceptions.

Next: [Getting Started](../getting-started.md).
