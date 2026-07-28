---
id: CARGO-ALLOW-PLAN-0007
kind: implementation_plan
status: draft
owner: repo-infra
created: 2026-06-18
linked_proposal: CARGO-ALLOW-PROP-0007
linked_spec: CARGO-ALLOW-SPEC-0007
linked_adrs:
  - CARGO-ALLOW-ADR-0001
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0003
support_tier_impact: advisory
policy_impact:
  - .allow/artifacts/doc-artifacts.toml
---

# Implementation Plan: Multi-Ledger Federation

## Purpose

Sequence federation work for issue #1473 after portable governance C4 (#1752).
F0 registers design artifacts; F1+ implement runtime federation per
CARGO-ALLOW-SPEC-0007 and CARGO-ALLOW-ADR-0001.

## Linked Artifacts

- Proposal:
  [CARGO-ALLOW-PROP-0007](../../docs/proposals/CARGO-ALLOW-PROP-0007-multi-ledger-federation.md)
- Spec:
  [CARGO-ALLOW-SPEC-0007](../../docs/specs/CARGO-ALLOW-SPEC-0007-multi-ledger-federation.md)
- ADR:
  [CARGO-ALLOW-ADR-0001](../../docs/adr/CARGO-ALLOW-ADR-0001-multi-ledger-federation.md)
- Active goal:
  [CARGO-ALLOW-GOAL-0003](../../.allow/goals/active.toml)
- Gap inventory:
  [plans/migration-parity/gap-inventory.md](../migration-parity/gap-inventory.md)

## Non-Goals

- No runtime federation in F0.
- No silent merge of compat, mirror, or imported ledgers.
- No release cut or support-tier promotion without explicit authorization.
- No cross-repository network federation.

## PR Sequence

| PR | Work item | Scope | Status |
| --- | --- | --- | --- |
| F0 | `portable-governance-f0-federation` | Proposal, spec, ADR, plan, doc-artifact registration, active goal | done (#1755) |
| F1 | `portable-governance-f1-federation` | Ledger registry parse/validate, precedence ordering, doctor/spec-system reporting | done (#1756) |
| F2 | `portable-governance-f2-federation` | Multi-ledger check evaluation + receipt provenance fields | done (#1758) |
| F3 | `portable-governance-f3-federation` | Drain window enforcement + closeout linkage | done (#1759) |

## F0 Validation

Every F0 registration PR should run:

```bash
cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md
cargo run -p cargo-allow -- check --profile spec-system --mode audit --format json --output target/cargo-allow/spec-system.json
cargo test -p allow-policy spec_system::tests::parses_current_repository_active_goal_manifest
```

## F1 Validation (Future)

F1 must add before claiming runtime support:

- Unit tests for precedence tiers and same-tier conflict diagnostics.
- Tests for federation key duplicate detection and dialect skip recording.
- Characterization test that receipts include `ledger_contributors` when multiple
  ledgers participate.
- No-new and spec-system audit remain green.

## Rollback

F0 rollback removes federation artifacts from doc-artifacts and reverts active
goal links to blocked design-first posture. F1 rollback disables federation
code paths behind existing profile/mode gates without deleting F0 docs.

## Claim Boundary

This plan sequences federation work. F0 does not implement federation or prove
multi-lane parity.
