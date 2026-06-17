---
id: CARGO-ALLOW-PLAN-0002
kind: implementation_plan
status: active
owner: repo-infra
created: 2026-06-16
linked_proposal: CARGO-ALLOW-PROP-0002
linked_spec: CARGO-ALLOW-SPEC-0002
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0002
support_tier_impact: advisory
policy_impact:
  - policy/doc-artifacts.toml
  - policy/allow.toml
---

# Implementation Plan: Migration and Evidence Parity

## Purpose

Sequence PR-sized work that strengthens cargo-allow migration and evidence
parity on the path to the `0.2.0` milestone claim:

```text
cargo-allow can replace bespoke AST/TOML allowlist xtasks.
```

## Linked Artifacts

- Proposal:
  [CARGO-ALLOW-PROP-0002](../../docs/proposals/CARGO-ALLOW-PROP-0002-migration-parity.md)
- Spec:
  [CARGO-ALLOW-SPEC-0002](../../docs/specs/CARGO-ALLOW-SPEC-0002-migration-parity.md)
- Support-tier surface:
  [CARGO-ALLOW-SUPPORT-0001](../../docs/status/SUPPORT_TIERS.md)
- Active goal:
  [CARGO-ALLOW-GOAL-0002](../../.codex/goals/active.toml)
- Migration guide:
  [docs/migration-from-xtask.md](../../docs/migration-from-xtask.md)
- PR queue and gap inventory:
  [pr-queue.md](pr-queue.md),
  [gap-inventory.md](gap-inventory.md)

## Non-Goals

- Do not claim full xtask replacement until side-by-side proof exists per lane.
- Do not broaden scanner identity beyond current source-syntax boundaries.
- Do not publish `0.2.0` or cut patch releases without explicit authorization.
- Do not execute proof providers from cargo-allow's own scan.

## Claim Boundary

This plan sequences migration parity work. It does not prove parity, execute
side-by-side checks, or replace repository-specific xtask evidence.

## Validation Baseline

Every PR should run the narrow useful checks for its blast radius:

- `rtk git diff --cached --check`
- `rtk cargo fmt --all --check`
- `rtk cargo clippy --workspace --all-targets -- -D warnings`
- `rtk cargo test --workspace`
- `rtk cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md`

Governance registration PRs should also run:

- `rtk cargo run -p cargo-allow -- doctor --profile spec-system --format json`
- `rtk cargo run -p cargo-allow -- check --profile spec-system --mode audit --format json --output target/cargo-allow/spec-system.json`

## Foundation Already Landed

Compat bridges, migration writers, and adoption docs already exist for panic,
unsafe, lint, and non-Rust lanes. This plan governs the next parity-hardening
slices rather than inventing migration from scratch.

## PR Sequence

### PR 1: Register migration parity goal

Purpose: archive `CARGO-ALLOW-GOAL-0001`, register proposal/spec/plan/goal
artifacts for the `0.2.0` migration parity lane, and point agents at bounded work
items.

Non-goals: no scanner or compat behavior changes.

Files: `.codex/goals/`, `docs/proposals/`, `docs/specs/`, `plans/migration-parity/`,
`policy/doc-artifacts.toml`.

Validation: spec-system structural checks plus the validation baseline.

Claim boundary: registers execution state and artifact links only.

Rollback: restore prior active goal manifest and doc-artifact ledger entries.

### PR 2: Strengthen side-by-side evidence for one compat lane

Purpose: pick one compat kind with the highest adoption friction and add
fixture-backed characterization for side-by-side delta classification.

Non-goals: no new compat kinds and no release cut.

Validation: targeted tests plus the validation baseline.

Claim boundary: documents observed deltas for one lane; does not claim full
xtask retirement.

Rollback: revert the characterization slice.

### PR 3: Improve migration closeout queue routing

Purpose: make saved `cargo-allow.migrate.v1` summaries easier to turn into owned
closeout work items without chat memory.

Non-goals: no policy broadening and no auto-approval of baseline debt.

Validation: migration summary tests plus the validation baseline.

Claim boundary: improves closeout routing metadata only.

Rollback: revert the routing slice.
