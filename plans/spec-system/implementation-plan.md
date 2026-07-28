---
id: CARGO-ALLOW-PLAN-0001
kind: implementation_plan
status: done
owner: repo-infra
created: 2026-06-12
linked_proposal: CARGO-ALLOW-PROP-0001
linked_spec: CARGO-ALLOW-SPEC-0001
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0001
linked_closeout: CARGO-ALLOW-CLOSEOUT-0001
support_tier_impact: advisory
policy_impact:
  - policy/doc-artifacts.toml
  - policy/spec-system.toml
  - policy/allow.toml
---

# Implementation Plan: Spec-System Profile

## Purpose

Sequence the opt-in `spec-system` profile into PR-sized work that keeps default
cargo-allow behavior unchanged.

The plan turns the accepted proposal and spec into a source-tree graph linter
for governed repo truth: proposal, spec, ADR, implementation plan, active goal,
support-tier, policy-ledger, proof-command, PR-reference, and closeout links.

## Linked Artifacts

- Proposal:
  [CARGO-ALLOW-PROP-0001](../../docs/proposals/CARGO-ALLOW-PROP-0001-spec-system-profile.md)
- Spec:
  [CARGO-ALLOW-SPEC-0001](../../docs/specs/CARGO-ALLOW-SPEC-0001-spec-system-profile.md)
- Support-tier surface:
  [CARGO-ALLOW-SUPPORT-0001](../../docs/status/SUPPORT_TIERS.md)
- Active goal:
  [CARGO-ALLOW-GOAL-0001](../../.codex/goals/active.toml)
- Closeout:
  [CARGO-ALLOW-CLOSEOUT-0001](closeout.md)
- Policy ledgers:
  [policy/doc-artifacts.toml](../../policy/doc-artifacts.toml) and
  [policy/allow.toml](../../policy/allow.toml)

## Non-Goals

- Do not make spec-system checks part of default cargo-allow behavior.
- Do not execute proof commands from cargo-allow's own scan.
- Do not call GitHub APIs or network services from the scanner.
- Do not invoke Cargo, rustc, Clippy, build scripts, proc macros, ripr,
  unsafe-review, or coverage tooling from the scanner.
- Do not lint prose quality, heading style, line length, capitalization, or
  exact Markdown section order.
- Do not create a public `allow-spec` crate before the internal profile model
  proves useful.

## Claim Boundary

This plan sequences work. It does not prove semantic correctness, proof
execution, release readiness, unsafe soundness, test adequacy, coverage, or
support-tier truth.

Future `spec-system` implementation may claim only structural source-tree
facts: ledgers parsed, artifacts found, IDs matched, links resolved, required
fields present or missing, and worklist items emitted.

## Validation Baseline

Every PR should run the narrow useful checks for its blast radius. Source-tree
profile work should use this baseline unless the PR explicitly explains a
smaller or larger proof path:

- `git diff --cached --check`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md`

Once profile commands exist, also run:

- `cargo run -p cargo-allow -- check --profile spec-system --mode audit`

Do not treat those proof commands as commands cargo-allow itself executes
during a source-tree scan.

## Foundation Already Landed

These source-of-truth foundation slices have landed:

| Slice | Artifact |
| --- | --- |
| PR 1 | Source-of-truth stack documentation. |
| PR 2 | Source-of-truth templates. |
| PR 3 | `CARGO-ALLOW-PROP-0001`. |
| PR 4 | `CARGO-ALLOW-SPEC-0001`. |
| PR 5 | `policy/doc-artifacts.toml`. |
| PR 6 | `CARGO-ALLOW-SUPPORT-0001`. |
| PR 7 | `CARGO-ALLOW-GOAL-0001`. |

This PR adds the implementation plan and draft closeout.

## PR Sequence

### PR 9: policy: add spec-system config model

Purpose: add internal `SpecSystemConfig`, roots, requirements, artifact kind,
artifact status, and doc artifact structs.

Non-goals: no CLI behavior and no Markdown file validation.

Files: `crates/allow-policy/src/spec_system/*` and focused tests.

Validation: `cargo test -p allow-policy spec_system` plus the baseline.

Claim boundary: config types parse and validate known enum values; no source
graph scan exists yet.

Rollback: remove the internal module and tests.

### PR 10: policy: parse doc artifact ledger

Purpose: load `policy/doc-artifacts.toml` and validate basic ledger shape.

Non-goals: do not read governed Markdown files yet.

Files: `allow-policy` parser, fixtures, and tests.

Validation: tests for minimal ledger, duplicate IDs, missing owner/path,
unknown kind, and unknown status.

Claim boundary: TOML and row validation only.

Rollback: remove the loader and fixtures.

### PR 11: spec-system: validate artifact files and IDs

Purpose: check registered paths, visible IDs, expected roots, and superseded
targets.

Non-goals: no support-tier parsing and no active-goal validation yet.

Files: `allow-policy` validator tests and fixtures.

Validation: missing file, ID missing from file, kind/path mismatch, and
superseded replacement tests.

Claim boundary: source-tree file and identity checks only.

Rollback: remove the validator slice and tests.

### PR 12: spec-system: validate graph edges

Purpose: validate proposal/spec/ADR/plan/active-goal/closeout references that
can be resolved from the ledger and configured roots.

Non-goals: do not execute proof commands or call GitHub for PR references.

Files: `allow-policy` graph validation tests and fixtures.

Validation: accepted spec without proposal, unknown linked spec, active goal
unknown plan, and standalone reason tests.

Claim boundary: static edge resolution only.

Rollback: remove graph-edge checks and fixtures.

### PR 13: spec-system: validate support-tier edges

Purpose: parse the support-tier table enough to validate required claim and
proof-command fields.

Non-goals: no prose linting and no proof-command execution.

Files: support-tier parser and tests.

Validation: stable/stabilizing rows without proof commands fail; advisory rows
without current proof remain advisory.

Claim boundary: table and field presence only.

Rollback: remove support-tier parser and tests.

### PR 14: cargo-allow: add --profile spec-system advisory check

Purpose: add explicit profile selection for `check` and `audit` and run
spec-system validation only when requested.

Non-goals: no default behavior change.

Files: CLI orchestration and focused integration tests.

Validation: default commands ignore spec-system files; profile commands report
advisory findings.

Claim boundary: structural profile checks only.

Rollback: remove profile CLI path.

### PR 15: report: add spec-system JSON artifact

Purpose: emit `cargo-allow.spec-system.v1` JSON with summary, artifacts,
links, findings, work items, claim boundary, and scanner limitations.

Non-goals: no blocking CI posture yet.

Files: report types and schema/tests where appropriate.

Validation: JSON snapshot or schema tests.

Claim boundary: report content records structural source-tree facts only.

Rollback: remove report output path and schema.

### PR 16: worklist: emit spec-system repair items

Purpose: turn graph findings into bounded repair items with artifact IDs,
paths, owners, messages, suggested actions, and proof commands.

Non-goals: no auto-fix and no policy broadening.

Files: worklist conversion and tests.

Validation: item-kind tests for missing node, unknown target, missing closeout,
and missing proof command.

Claim boundary: routing output only.

Rollback: remove spec-system worklist conversion.

### PR 17: doctor/init: support spec-system readiness

Purpose: add `doctor --profile spec-system` readiness and later
`init --profile spec-system` bootstrap behavior.

Non-goals: do not make profile files required by default.

Files: CLI profile readiness and bootstrap tests.

Validation: missing/found config, ledger, support tiers, active goal, and
template readiness checks.

Claim boundary: readiness and bootstrap only.

Rollback: remove profile doctor/init paths.

### PR 18: dogfood: enable spec-system profile advisory

Purpose: add `policy/spec-system.toml` and run the profile advisory on this
repo.

Non-goals: no blocking CI yet.

Files: profile config, docs/CI notes, and policy receipts.

Validation: `cargo-allow check --profile spec-system --mode audit`.

Claim boundary: advisory graph validation only.

Rollback: remove profile config and advisory CI/doc references.

### PR 19+: dogfood and burn-in

Purpose: fix initial worklist categories, upload advisory artifacts, promote the
repo-local profile to shadow after clean advisory evidence, then promote
low-risk structural checks after shadow burn-in.

Non-goals: do not block nuanced checks immediately.

Validation: configured profile check, worklist JSON, shadow CI artifact, and
standard baseline.

Claim boundary: shadow posture reports failure posture without blocking; block
only safe structural invariants after evidence.

Completion status: the opt-in `spec-system` governance profile preview is
implemented and closed out in [closeout.md](closeout.md). The repo dogfoods the
profile in blocking mode for selected objective structural findings while
nuanced lifecycle checks remain advisory. The implementation includes docs,
templates, proposal/spec/support-tier/active-goal/plan/closeout artifacts,
profile config, doc-artifact ledger parsing, artifact identity and link
validation, support-tier validation, active-goal TOML validation, JSON/Markdown
reports, worklist repair items, doctor/init support, CI artifact upload,
first-hour and CI adoption docs, preview release notes, and
`explain <artifact-id> --profile spec-system`.

The support-tier row remains advisory. Release authorization, package version
bump, tagging, publishing, install-smoke checks, and stable-support promotion
are explicitly out of scope for this plan closeout.

Rollback: demote `policy/spec-system.toml` to shadow, revert selected blocking
checks, remove the preview-readiness notes, or remove the profile-specific
explain path.

## Support-Tier Updates

`CARGO-ALLOW-SUPPORT-0001` records the `spec-system` profile as advisory. Later
PRs should update that row only when implemented behavior and promotion evidence
make a stronger claim true.

## Policy Updates

This plan expects these policy surfaces:

- `policy/doc-artifacts.toml` for source-of-truth artifact registration.
- `policy/spec-system.toml` for opt-in profile roots and requirements.
- `policy/allow.toml` for ordinary tracked-file governance while dogfooding.

## Closeout Requirements

- Closeout path: [plans/spec-system/closeout.md](closeout.md).
- Required validation evidence: each implementation PR's validation plus final
  advisory dogfood evidence.
- Remaining-work format: list unfinished PR slices or state `none`.

## Rollback Path

If the spec-system direction is withdrawn, supersede or remove the linked
proposal and spec, remove profile config and doc artifacts, demote support-tier
claims, archive the active goal, and keep default cargo-allow source-exception
behavior unchanged.
