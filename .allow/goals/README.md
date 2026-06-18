# Active Goals

This directory contains spec-system artifacts for agent execution state that points at repo truth.

Register governed artifacts in `.allow/artifacts/doc-artifacts.toml` so `cargo-allow check --profile spec-system` can validate their source-tree graph links.

## Files

| Path | Purpose |
| --- | --- |
| `active.toml` | Current source-of-truth execution goal for Codex and agents. |
| `archive/` | Completed or superseded goal manifests. |

## Claim Boundary

The manifest may list proof commands that a human or authorized agent should
run for a work item. cargo-allow must not execute those commands as part of a
`spec-system` scan. The profile owns structural graph validation and worklist
routing only.

## Maintenance

Keep `active.toml` focused on the current execution lane. Archive completed or
superseded manifests under `archive/` when a later goal succeeds them.
