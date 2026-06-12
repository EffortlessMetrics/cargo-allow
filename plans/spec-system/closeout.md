---
id: CARGO-ALLOW-CLOSEOUT-0001
kind: closeout
status: draft
owner: repo-infra
created: 2026-06-12
linked_plan: CARGO-ALLOW-PLAN-0001
linked_proposal: CARGO-ALLOW-PROP-0001
linked_spec: CARGO-ALLOW-SPEC-0001
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0001
support_tier_impact: advisory
policy_impact:
  - policy/doc-artifacts.toml
  - policy/spec-system.toml
  - policy/allow.toml
---

# Closeout: Spec-System Profile

## Summary

Draft closeout for [CARGO-ALLOW-PLAN-0001](implementation-plan.md).

The plan is active and not complete. This file reserves the closeout location so
later PRs have a stable target for completed work and final evidence. It must
not be treated as proof that the profile has landed.

## Landed Changes

- Not closed yet.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| Final spec-system advisory check | not run | Profile commands are not implemented yet. |
| Final dogfood worklist | not run | Worklist support is not implemented yet. |
| Final support-tier review | not run | The support-tier row remains advisory. |

## Non-Goals

- Do not claim the implementation plan is complete.
- Do not claim `--profile spec-system` is implemented.
- Do not claim proof commands were executed by cargo-allow.

## Claim Boundary

This draft closeout is a placeholder. It records where final evidence will go;
it does not record final evidence yet.

## Support-Tier Updates

No support-tier promotion yet. `CARGO-ALLOW-SUPPORT-0001` remains advisory for
the spec-system profile.

## Policy Updates

Current source-of-truth artifacts are registered in `policy/doc-artifacts.toml`
and governed as tracked source-tree files by `policy/allow.toml`.

## Remaining Work

- Implement the config model.
- Parse and validate the doc artifact ledger.
- Validate graph identity and edges.
- Validate support-tier proof-command fields.
- Add explicit profile CLI behavior.
- Emit reports, receipts, and worklists.
- Dogfood advisory mode before any shadow or blocking promotion.

## Rollback

If the plan is withdrawn, remove this closeout placeholder, remove its
`policy/doc-artifacts.toml` row, and remove its `policy/allow.toml` entry.

## Follow-Up Links

- Next plan item: PR 9, `policy: add spec-system config model`.
