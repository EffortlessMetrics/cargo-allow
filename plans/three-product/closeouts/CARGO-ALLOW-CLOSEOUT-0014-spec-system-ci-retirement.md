---
id: CARGO-ALLOW-CLOSEOUT-0014
kind: closeout
status: accepted
owner: repo-infra
created: 2026-07-24
linked_plan: CARGO-ALLOW-PLAN-0010
linked_spec: CARGO-ALLOW-SPEC-0010
support_tier_impact: advisory
policy_impact:
  - .allow/compatibility/intent-delegation.toml
  - policy/allow.toml
  - policy/product-move-ledger.toml
  - policy/three-product-simplification.toml
---

# Closeout: Spec-System CI Audit Retirement (#2568)

## Summary

Enable repo-wide `delegate_spec_system` cutover, retire the embedded
`check --profile spec-system --mode audit` CI lane, and replace it with a
cutover receipt that proves fail-closed posture without inventing a cargo-intent
audit vertical.

## Landed Changes

- `.allow/compatibility/intent-delegation.toml` — `delegate_spec_system` and
  `delegate_staged_precommit` enabled
- `scripts/spec-system-cutover-receipt.sh` — CI receipt for cutover state
- CI `test` job rerouted from embedded audit to cutover receipt
- Move ledger, simplification inventory, support tiers, and stage receipt updated

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test -p cargo-allow spec_system_ci_retirement` | pass | characterization |
| `cargo test -p cargo-allow intent_spec_system_cutover` | pass | fail-closed e2e |
| `bash scripts/spec-system-cutover-receipt.sh` | pass | CI receipt |
| `cargo-allow check --mode no-new` | pass | policy allows |

## Claim Boundary

**Establishes:** this repository no longer treats embedded spec-system evaluation
as production authority; legacy commands fail closed when cutover is enabled; CI
proves cutover instead of running the old audit.

**Does not establish:** cargo-intent audit/doctor/worklist parity, deletion of
`spec_system*.rs` modules (retained for external repos without cutover), or
physical repository extraction.

## Residual

- Delete or further narrow `crates/cargo-allow/src/spec_system*.rs` after
  cargo-intent ships audit parity or published deprecation window ends
- Split independent CI lanes per product (#2559 limitation)

## Rollback

Set `delegate_spec_system = false` in `.allow/compatibility/intent-delegation.toml`
and restore embedded audit CI steps only with explicit authorization.
