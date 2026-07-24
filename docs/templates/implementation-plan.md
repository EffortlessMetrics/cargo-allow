---
id: CARGO-ALLOW-PLAN-0000
kind: implementation_plan
status: draft
owner: repo-infra
created: YYYY-MM-DD
linked_proposal:
linked_spec:
linked_adr:
support_tier_impact: none
policy_impact: none
---

# Implementation Plan: Title

## Purpose

Describe the concrete outcome this plan is intended to land.

## Linked Artifacts

- Proposal:
- Spec:
- ADR:
- Support-tier surface:
- Policy ledger:

## Non-Goals

- Non-goal:
- Non-goal:

## Claim Boundary

State what the plan sequences and what it does not prove. Plans do not prove
semantic correctness, proof execution, release readiness, unsafe soundness, test
adequacy, or coverage.

## Required Evidence

- Evidence expected before the plan can close:
- Evidence expected before the plan can close:

## Validation Baseline

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md`

Add or remove commands only when the linked spec requires it. Do not list proof
commands that cargo-allow itself must execute during source-tree scanning.

## PR Sequence

### PR 1: Title

Purpose:

Non-goals:

Files:

Validation:

Claim boundary:

Rollback:

### PR 2: Title

Purpose:

Non-goals:

Files:

Validation:

Claim boundary:

Rollback:

## Support-Tier Updates

Name any support-tier rows that must be added, changed, or checked by this
plan.

## Policy Updates

Name any policy ledger, config, schema, or source-tree governance update this
plan must make.

## Closeout Requirements

- Closeout path:
- Required validation evidence:
- Remaining-work format:

## Rollback Path

Describe how to revert the plan if the linked spec or proposal is withdrawn.
