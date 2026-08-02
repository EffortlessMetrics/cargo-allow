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

## Artifact Scope

The `cargo-allow.prune.v1` JSON artifact is a cleanup candidate list, not a
full projection of each policy entry. Each `stale_entries` item includes the
entry identity, kind, optional family, owner, classification, effective scope,
and reason. It intentionally omits source `path`, `glob`, and `selector`
details; use `list` or `explain` when you need that source context before
approving removal.

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
