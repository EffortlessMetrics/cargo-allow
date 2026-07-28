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

After B1–B6 groundwork, the active execution lane shifts to adoption-substrate
modularization (PRs 2–6). Release automation for `0.1.10` stays dormant until
explicit authorization.

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
- Closeout:
  [CARGO-ALLOW-CLOSEOUT-0002](closeouts/incremental-slices.md)

## Non-Goals

- Do not claim full xtask replacement until side-by-side proof exists per lane.
- Do not broaden scanner identity beyond current source-syntax boundaries.
- Do not publish `0.2.0` or cut patch releases without explicit authorization.
- Do not execute proof providers from cargo-allow's own scan.
- Do not activate `0.1.10` release cut during adoption-substrate work.

## Claim Boundary

This plan sequences migration parity work. It does not prove parity, execute
side-by-side checks, or replace repository-specific xtask evidence.

## Validation Baseline

Every PR should run the narrow useful checks for its blast radius:

- `git diff --cached --check`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md`

Governance registration PRs should also run:

- `cargo run -p cargo-allow -- doctor --profile spec-system --format json`
- `cargo run -p cargo-allow -- check --profile spec-system --mode audit --format json --output target/cargo-allow/spec-system.json`

## Foundation Already Landed

Compat bridges, migration writers, and adoption docs already exist for panic,
unsafe, lint, and non-Rust lanes. This plan governs the next parity-hardening
slices rather than inventing migration from scratch.

## Execution State

| Slice | Status | Evidence |
| --- | --- | --- |
| PR 1 — register migration parity goal | done | #1687; archived GOAL-0001 |
| B1 — gap inventory | done | [gap-inventory.md](gap-inventory.md) (B1r) |
| B2 — no-panic-baseline evidence/lifecycle | done | #1691 (`1cd408e`) |
| B3 — migration fixture matrix | done | #1693 (`cd0ab7b`) |
| B4 — migration closeout routing | done | #1695 (`64832c5`) |
| B5 — panic-baseline dogfood receipt | done | #1697 (`26a6873`) |
| B6 — import/parity disposition | done | #1470 closed (#1699, #1700); #1466 split → adoption-substrate-pr-005 |
| D2 — structural identity refactor pairs | done | #1701 (`2165848`) |
| Release hardening (#1703–#1705) | done, dormant | not active release lane |
| #1478 spec-system hygiene | closed | #1706 |
| **Adoption substrate PR 2** | **next** | `adoption-substrate-pr-002` ready |
| Adoption substrate PRs 3–6 | blocked | sequenced after PR 2 |
| B7 — 0.2.0 migration parity notes | pending | after adoption substrate |

## PR Sequence (completed B1–B6)

### PR 1: Register migration parity goal (done, #1687)

Purpose: archive `CARGO-ALLOW-GOAL-0001`, register proposal/spec/plan/goal
artifacts for the `0.2.0` migration parity lane, and point agents at bounded work
items.

Non-goals: no scanner or compat behavior changes.

Validation: spec-system structural checks plus the validation baseline.

Claim boundary: registers execution state and artifact links only.

### PR B2: Strengthen side-by-side evidence for no-panic-baseline (done, #1691)

Purpose: close the first focused compat gap — preserve legacy metadata and
evidence on `no-panic-baseline` import, keep visible `baseline_debt` when
evidence is absent, and fix lifecycle handling when `review_after` is set without
`expires`.

Status: done (merge `1cd408e`).

Claim boundary: documents observed behavior for the panic-baseline import slice;
does not claim full xtask retirement or full panic-lane parity.

### PR B3: Add fixture matrix for all supported legacy lanes (done, #1693)

Purpose: add `tests/fixtures/migration/` characterization across compat kinds
listed in [pr-queue.md](pr-queue.md).

Status: done (merge `cd0ab7b`).

Claim boundary: fixture-backed observed migration output only.

### PR B4: Improve migration closeout queue routing (done, #1695)

Purpose: make saved `cargo-allow.migrate.v1` summaries easier to turn into owned
closeout work items without chat memory.

Status: done (merge `64832c5`).

Claim boundary: improves closeout routing metadata only.

### PR B5: Add side-by-side dogfood receipt (done, #1697)

Purpose: run migration parity proof against this repository's own legacy surfaces.

Status: done (merge `26a6873`).

Claim boundary: dogfood evidence for this repo only.

### PR B6: Import/parity issue disposition (done)

Purpose: close #1470 foreign-dialect discovery; record #1466 umbrella split for
adoption lane.

Status: done — #1470 closed; #1466 deferred to `adoption-substrate-pr-005`.

Claim boundary: issue disposition only.

## Adoption Substrate Sequence (active)

### PR 2: Migration lane descriptors (next)

Purpose: modular descriptor surface for compat kinds without behavior change.

Active work item: `adoption-substrate-pr-002`.

### PR 3: Evidence/lifecycle helpers

Purpose: refactor-preserving extraction of shared import metadata paths.

Blocked until PR 2 lands.

### PR 4: Closeout queue normalization

Purpose: consistent phased `next_queues` across compat kinds.

Blocked until PR 3 lands.

### PR 5: Split #1466 governance

Purpose: split open umbrella into owned sub-issues with deferral boundaries.

Blocked until PR 4 lands. Links CARGO-ALLOW-SPEC-0004 / allow-import plan.

### PR 6: Advisory occurrence counts

Purpose: advisory ratcheting metadata for baseline debt visibility.

Blocked until PR 5 lands.

### PR B7: Stage 0.2.0 migration parity notes (pending)

Purpose: document milestone claim boundary before any `0.2.0` cut authorization.

Pending after adoption substrate PRs 2–6.

## Dormant Release Lane

`0.1.10` release automation groundwork (#1703–#1705) is complete on `main`.
Cut deferred per [docs/release/0.1.10-readiness.md](../../docs/release/0.1.10-readiness.md).
See [plans/release/0.1.10-implementation-plan.md](../release/0.1.10-implementation-plan.md).
