# Historical Goal Artifacts

This directory contains historical spec-system goal artifacts. It is not a
repository-global work queue, current issue pointer, or session store.

Register governed artifacts in `.allow/artifacts/doc-artifacts.toml` so `cargo-allow check --profile spec-system` can validate their source-tree graph links.

## Files

| Path | Purpose |
| --- | --- |
| `archive/` | Completed or superseded goal manifests retained as read-only evidence. |

## Claim Boundary

Historical manifests may list proof commands that a human or authorized agent
ran for a completed work item. cargo-allow does not execute those commands as
part of a `spec-system` scan. Archived goals cannot authorize mutations, select
current work, or promote implementation or support status.

## Maintenance

Keep live work in GitHub issues and PRs, accepted requirements, and PR-local
implementation slices. Archive completed or superseded manifests under
`archive/`; do not recreate `active.toml` as a current pointer.
