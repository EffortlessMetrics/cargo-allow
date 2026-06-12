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
- `policy/spec-system.toml` enables the profile in blocking mode for this repo.
- CI uploads `spec-system.json` and `spec-system.md` artifacts through the
  existing `target/cargo-allow/` report bundle.
- The first merged shadow mainline run uploaded a clean spec-system artifact.
- Blocking mode now fails commands only for selected objective structural
  findings, and cargo-allow's own repo dogfoods that posture.
- Reports and worklists now separate blocking-eligible posture from advisory
  lifecycle posture for reviewer and agent routing.
- First-hour adoption and CI guides document the profile as one opt-in
  governance profile while preserving default source-exception behavior.
- Draft preview release notes describe the opt-in spec-system profile without
  changing package versions, publishing crates, or claiming stable support.
- The support-tier claim map has been reviewed after the preview-release draft;
  `CARGO-ALLOW-SUPPORT-0001` remains advisory.
- The repo-local profile has been promoted from shadow to blocking for selected
  objective structural findings only.
- The opt-in preview release notes have been reviewed for release-authorization
  readiness without changing versions, packaging, tagging, publishing, or
  claiming stable support.
- `explain <artifact-id> --profile spec-system` is implemented as a
  single-artifact graph view over the existing source-tree profile report.
- Active-goal TOML validation now parses `.codex/goals/active.toml`, checks
  manifest links against the doc-artifact ledger, requires proof commands on
  ready/in-progress/done work items, resolves any provided closeout links,
  requires closeouts for done work items, and routes those lifecycle findings
  as advisory repair work.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| First advisory CI artifact | passed | GitHub Actions run `27401018305`, `push` to `main` at `81c41dea899c4a635206b075e293ddd61a3c88e3`, uploaded `cargo-allow-reports` containing `spec-system.json` and `spec-system.md`. |
| First advisory spec-system JSON | passed | `spec-system.json` reported schema `cargo-allow.spec-system.v1`, `command = check`, `mode = advisory`, `status = passed`, `config_source = policy/spec-system.toml`, 6 artifacts, 17 links, 4 support-tier rows, 0 findings, and 0 work items. |
| First advisory Markdown report | passed | `spec-system.md` reported advisory result, 0 findings, 0 work items, and the structural source-tree graph claim boundary. |
| First shadow CI artifact | passed | GitHub Actions run `27403356886`, `push` to `main` at `396f3298bc2dea95295ffd4e3bd5bf96fba4c477`, uploaded `cargo-allow-reports/spec-system.json` with `schema_id = cargo-allow.spec-system.v1`, `mode = shadow`, `status = passed`, `failed = false`, 6 artifacts, 17 links, 4 support-tier rows, 0 findings, and 0 work items. |
| First shadow worklist posture | passed | The same mainline artifact reported 0 work items. Local follow-up `worklist --profile spec-system --format json` also reported `mode = shadow`, `status = passed`, `failed = false`, 0 findings, and 0 work items. |
| Blocking-eligible classification | passed locally | `spec_system_profile` tests cover malformed profile config, duplicate artifact IDs, invalid artifact status/kind, missing registered artifact files, missing declared IDs, unknown links, and a non-blocking missing required edge. |
| Report/worklist output polish | passed locally | `spec_system_profile`, `spec_system_worklist`, and `artifact_schema` tests cover blocking-eligible/advisory summary counts and work-item posture fields. |
| First-hour adoption and CI docs | passed locally | `docs/how-to/adopt-spec-system-profile.md` and `docs/how-to/run-spec-system-in-ci.md` document advisory/shadow adoption, safe structural blocking candidates, and the no-execution claim boundary. |
| Preview release notes | passed locally | `CHANGELOG.md` and `docs/release/0.1.7.md` describe the opt-in spec-system preview and explicitly avoid stable-support, publication, and proof-execution claims. |
| Support-tier claim map review | passed locally | `cargo-allow check --profile spec-system --mode audit` and `worklist --profile spec-system --format json` reported `mode = shadow`, `status = passed`, 0 findings, and 0 work items. `CARGO-ALLOW-SUPPORT-0001` remains advisory, and no repo-local blocking promotion is included in this review. |
| Repo-local blocking promotion | passed locally | `doctor --profile spec-system`, `check --profile spec-system --mode audit`, and `worklist --profile spec-system --format json` reported `mode = blocking`, `status = passed`, 0 findings, and 0 work items. The profile blocks only selected objective structural findings; nuanced lifecycle checks remain advisory. |
| Preview release readiness review | passed locally | `docs/release/0.1.7.md` records current version/tag/release state, main CI run `27412948507`, spec-system blocking-mode checks, and the default no-new guard. No package, tag, publish, install-smoke, or stable-support claim is included. |
| Single-artifact explain view | passed locally | `cargo-allow explain CARGO-ALLOW-SPEC-0001 --profile spec-system --format json` reported `command = explain`, `mode = blocking`, `status = passed`, one explained artifact, 5 related links, 0 findings, 0 work items, 3 proof commands, and the structural no-execution claim boundary. |
| Active-goal TOML validation | passed locally | `cargo test -p allow-policy spec_system`, `cargo test -p cargo-allow spec_system_profile`, `cargo test -p cargo-allow spec_system_worklist`, and `cargo test -p cargo-allow artifact_schema_spec_system` passed. The tests cover the current repo active-goal manifest, unknown linked plans, missing work-item proof commands, done work items without closeouts, optional closeout links on non-done work items, advisory profile findings, advisory worklist routing, and schema enum coverage. |

## Non-Goals

- Do not claim the implementation plan is complete.
- Do not claim the profile has completed stable support or final blocking
  burn-in.
- Do not claim proof commands were executed by cargo-allow.

## Claim Boundary

This draft closeout records interim advisory, shadow, and repo-local blocking
promotion evidence. It does not prove final profile stability, stable support,
semantic correctness, or proof-command execution.

## Support-Tier Updates

No support-tier promotion yet. `CARGO-ALLOW-SUPPORT-0001` remains advisory for
the spec-system profile.

## Policy Updates

Current source-of-truth artifacts are registered in `policy/doc-artifacts.toml`
and governed as tracked source-tree files by `policy/allow.toml`.

## Remaining Work

- Watch repo-local blocking mode during normal CI runs before treating it as
  stable support.
- Keep nuanced checks advisory until they prove low-noise.
- Obtain explicit release authorization before package, tag, publish,
  install-smoke, public install-doc update, or release-record finalization work.
- Run the final completion audit before closing the plan.

## Rollback

If the plan is withdrawn, remove this closeout placeholder, remove its
`policy/doc-artifacts.toml` row, and remove its `policy/allow.toml` entry.

## Follow-Up Links

- Next plan item: complete the final spec-system profile audit and closeout.
