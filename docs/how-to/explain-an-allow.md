# Explain an Allow Entry

Use `explain` when a maintainer or reviewer needs to know why a retained
exception exists.

> Maturity: `explain` and its companion `list` command are Stable in published
> `0.1.11` and Stabilizing on current main. See the [command maturity table](../status/SUPPORT_TIERS.md#command-maturity).

## Human Output

```bash
cargo-allow explain allow-0042
```

The human view shows the allow entry, current match status, owner, reason,
classification, lifecycle, selector details, evidence diagnostics, suggested
actions, proof commands, and claim boundary.

## JSON Output

```bash
cargo-allow explain allow-0042 \
  --format json \
  --output target/cargo-allow/explain-allow-0042.json
```

Use JSON when handing work to an agent or saving audit evidence.

## What to Check

- Is the entry still matched?
- Is the selector narrow enough?
- Is the owner still correct?
- Is local evidence present?
- Is review due or expiry approaching?

## Claim Boundary

`explain` reports source-tree/source-syntax state. It does not prove that the
exception is safe or that tests are adequate.

Reference: [Source exception ledger](../source-exception-ledger.md).
