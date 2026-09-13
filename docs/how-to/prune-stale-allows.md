# Prune Stale Allows

Use `prune` when policy entries no longer match current source-tree findings.

> Maturity: `prune` is Stable in published `0.1.11` and Stabilizing on current
> main. See the [command maturity table](../status/SUPPORT_TIERS.md#command-maturity).

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

On current source builds, select one entry for preview or removal:

```bash
cargo-allow prune --allow-id <allow-id> --dry-run
```

Keep the same `--allow-id` when using `--write` after review. The preview,
receipt, and policy change contain only that selected stale entry. An existing
entry that is not stale produces no removal; an unknown ID returns a usage
error. Omitting `--allow-id` keeps the all-stale behavior shown above.
The preview's command-summary apply action also preserves `--include-untracked`
when selected, so the follow-up scan uses the same inventory option.

## Artifact Scope

The `cargo-allow.prune.v1` JSON artifact is a cleanup candidate list, not a
full projection of each policy entry. Each `stale_entries` item includes the
entry identity, kind, optional family, owner, classification, effective scope,
and reason. It intentionally omits source `path`, `glob`, and `selector`
details; use `list` or `explain` when you need that source context before
approving removal. The `removed_toml_blocks` array preserves the rendered TOML
blocks that the candidates would remove, so JSON previews retain the same
reviewable removal context as human output.

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
