# Active Goals

This directory records the current Codex execution surface for governed
source-of-truth work.

The active goal manifest is execution state for agents. It points to the
proposal, spec, support-tier map, and implementation plan that define the work.
It does not replace those artifacts and does not become product truth by
itself.

Default cargo-allow checks do not read or enforce `.codex/goals/active.toml`.
The opt-in `spec-system` profile reads it as a repo-native active-goal artifact
for structural graph validation.

## Files

| Path | Purpose |
| --- | --- |
| `active.toml` | Current source-of-truth execution goal for Codex and agents. |
| `archive/` | Completed or superseded goal manifests. |

## Link Boundary

The current manifest links to:

- `CARGO-ALLOW-PROP-0002`
- `CARGO-ALLOW-SPEC-0002`
- `CARGO-ALLOW-SUPPORT-0001`
- `CARGO-ALLOW-PLAN-0002`

The active manifest should keep `linked_plan_status` current. It is `active`
once [CARGO-ALLOW-PLAN-0002](../../plans/migration-parity/implementation-plan.md)
exists.

## Claim Boundary

The manifest may list proof commands that a human or authorized agent should
run for a work item. cargo-allow must not execute those commands as part of a
`spec-system` scan. The profile owns structural graph validation and worklist
routing only.

## Maintenance

Keep `active.toml` focused on the current execution lane. Archive completed or
superseded manifests under `archive/` when a later goal succeeds them.
