# Active Goals

This directory records the current Codex execution surface for the planned
`spec-system` source-of-truth profile.

The active goal manifest is execution state for agents. It points to the
proposal, spec, support-tier map, and implementation plan that define the work.
It does not replace those artifacts and does not become product truth by
itself.

Current cargo-allow releases do not read or enforce `.codex/goals/active.toml`.
The file exists so later `spec-system` profile implementation PRs have a
repo-native active-goal artifact to parse and validate structurally.

## Files

| Path | Purpose |
| --- | --- |
| `active.toml` | Current source-of-truth execution goal for Codex and agents. |
| `archive/` | Future home for completed or superseded goal manifests. |

## Link Boundary

The current manifest links to:

- `CARGO-ALLOW-PROP-0001`
- `CARGO-ALLOW-SPEC-0001`
- `CARGO-ALLOW-SUPPORT-0001`
- `CARGO-ALLOW-PLAN-0001`

The active manifest should keep `linked_plan_status` current. It is `active`
once [CARGO-ALLOW-PLAN-0001](../../plans/spec-system/implementation-plan.md)
exists.

## Claim Boundary

The manifest may list proof commands that a human or authorized agent should
run for a work item. cargo-allow must not execute those commands as part of a
future `spec-system` scan. The planned profile owns structural graph validation
and worklist routing only.

## Maintenance

Keep `active.toml` focused on the current execution lane. Archive completed or
superseded manifests under `archive/` when a later PR adds that workflow.
