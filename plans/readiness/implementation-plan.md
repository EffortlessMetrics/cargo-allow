---
id: CARGO-ALLOW-PLAN-0003
kind: implementation_plan
status: active
owner: repo-infra
created: 2026-06-17
linked_proposal: CARGO-ALLOW-PROP-0003
linked_spec: CARGO-ALLOW-SPEC-0003
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
support_tier_impact: advisory
policy_impact:
  - policy/doc-artifacts.toml
---

# Implementation Plan: Self-Hosting Readiness Policy

## Purpose

Record the strict versus provider-tracked readiness decision and refresh the
self-hosting record so `0.1.10` and migration parity work can proceed without
pretending `ripr+` or `unsafe-review+` are zero.

## Linked Artifacts

- Proposal:
  [CARGO-ALLOW-PROP-0003](../../docs/proposals/CARGO-ALLOW-PROP-0003-readiness-policy.md)
- Spec:
  [CARGO-ALLOW-SPEC-0003](../../docs/specs/CARGO-ALLOW-SPEC-0003-readiness-policy.md)
- Primary record:
  [docs/readiness/self-hosting.md](../../docs/readiness/self-hosting.md)

## Non-Goals

- Do not fix ripr or unsafe-review provider gaps in this plan.
- Do not start external repo migration.
- Do not bump workspace version.

## Claim Boundary

This plan sequences readiness documentation only. It does not execute proof
providers or prove zero-gap readiness.

## Validation Baseline

- `cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md`
- `cargo run -p cargo-allow -- check --profile spec-system --mode audit --format json --output target/cargo-allow/spec-system.json`

## PR Sequence

### PR A1: Record provider-tracked readiness policy

Purpose: define strict vs provider-tracked readiness and accept provider-tracked
posture for `0.1.10` / migration work.

Non-goals: no provider fixes, no version bump, no external migration.

Files: `docs/readiness/self-hosting.md`, `docs/specs/CARGO-ALLOW-SPEC-0003-readiness-policy.md`,
`docs/proposals/CARGO-ALLOW-PROP-0003-readiness-policy.md`, `plans/readiness/implementation-plan.md`,
`docs/status/SUPPORT_TIERS.md`, `policy/doc-artifacts.toml`, `CHANGELOG.md`.

Validation: no-new guard; spec-system audit when doc artifacts are registered.

Claim boundary: records policy decision and honest readiness language only.

Rollback: revert readiness docs and artifact registration.

### PR A2: Refresh self-hosting evidence under chosen policy

Purpose: regenerate local proof receipts and provider citation blocks after
post-`0.1.9` maintenance lands.

Non-goals: no provider fixes.

Files: `docs/readiness/self-hosting.md` summary table and validation section.

Validation: rerun cited proof commands; update recorded commit and counts.

Claim boundary: refreshes evidence timestamps only.

Rollback: revert evidence refresh.

### PR A3: Add ripr migration handoff after readiness policy is settled

Purpose: document external ripr adoption prerequisites under provider-tracked
posture.

Non-goals: no external repo migration; no ripr provider implementation.

Files: adoption docs under `docs/how-to/` or `plans/external-dogfood/`.

Validation: spec-system link checks.

Claim boundary: handoff documentation only.

Rollback: revert handoff doc.
