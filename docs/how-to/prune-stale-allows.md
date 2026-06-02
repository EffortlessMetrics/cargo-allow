# Prune Stale Allows

Use `prune` when policy entries no longer match current source-tree findings.

## Preview First

```bash
cargo-allow prune --stale --dry-run
```

Save a machine-readable preview:

```bash
cargo-allow prune \
  --stale \
  --dry-run \
  --format json \
  --output target/cargo-allow/prune.json
```

Review the candidate list before writing. Do not prune entries that are
ambiguous or poorly understood.

## Write

When the preview is correct:

```bash
cargo-allow prune --stale --write
```

Then verify:

```bash
cargo-allow check --mode no-new
```

## Claim Boundary

Prune only edits policy. It does not edit source files, compile code, execute
repository code, or prove removed exceptions were safe.

Reference: [Source exception ledger](../source-exception-ledger.md).
