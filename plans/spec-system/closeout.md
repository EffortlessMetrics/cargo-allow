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

- Source-of-truth graph docs, templates, proposal, spec, support-tier map,
  active goal manifest, implementation plan, draft closeout, and artifact
  ledger are present.
- Internal spec-system config, doc artifact ledger, artifact identity/link,
  support-tier, report, worklist, doctor, and init support has landed.
- `policy/spec-system.toml` enables the profile in advisory mode for this repo.
- CI uploads advisory `spec-system.json` and `spec-system.md` artifacts through
  the existing `target/cargo-allow/` report bundle.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| First advisory CI artifact | passed | GitHub Actions run `27401018305`, `push` to `main` at `81c41dea899c4a635206b075e293ddd61a3c88e3`, uploaded `cargo-allow-reports` containing `spec-system.json` and `spec-system.md`. |
| First advisory spec-system JSON | passed | `spec-system.json` reported schema `cargo-allow.spec-system.v1`, `command = check`, `mode = advisory`, `status = passed`, `config_source = policy/spec-system.toml`, 6 artifacts, 17 links, 4 support-tier rows, 0 findings, and 0 work items. |
| First advisory Markdown report | passed | `spec-system.md` reported advisory result, 0 findings, 0 work items, and the structural source-tree graph claim boundary. |
| Final dogfood worklist | not final | Current advisory run reports 0 work items, but the profile has not completed shadow or blocking burn-in. |
| Final support-tier review | not final | `CARGO-ALLOW-SUPPORT-0001` remains advisory for the spec-system profile. |

## Non-Goals

- Do not claim the implementation plan is complete.
- Do not claim the profile has completed shadow or blocking burn-in.
- Do not claim proof commands were executed by cargo-allow.

## Claim Boundary

This draft closeout records interim advisory evidence. It does not prove final
profile stability, shadow posture, blocking posture, semantic correctness, or
proof-command execution.

## Support-Tier Updates

No support-tier promotion yet. `CARGO-ALLOW-SUPPORT-0001` remains advisory for
the spec-system profile.

## Policy Updates

Current source-of-truth artifacts are registered in `policy/doc-artifacts.toml`
and governed as tracked source-tree files by `policy/allow.toml`.

## Remaining Work

- Review advisory burn-in over subsequent CI runs.
- Decide whether low-risk structural checks should move to shadow mode.
- Promote only safe structural checks after evidence supports it.
- Keep nuanced checks advisory until they prove low-noise.
- Update this closeout with final dogfood evidence before closing the plan.

## Rollback

If the plan is withdrawn, remove this closeout placeholder, remove its
`policy/doc-artifacts.toml` row, and remove its `policy/allow.toml` entry.

## Follow-Up Links

- Next plan item: review shadow-mode candidates after advisory burn-in.
