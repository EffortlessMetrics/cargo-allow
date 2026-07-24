---
id: CARGO-ALLOW-CLOSEOUT-0011
kind: closeout
status: accepted
owner: repo-infra
created: 2026-07-24
linked_plan: CARGO-ALLOW-PLAN-0010
linked_spec: CARGO-ALLOW-SPEC-0010
linked_adr: CARGO-ALLOW-ADR-0002
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: Three-Product Dogfood Pipeline (#2558)

## Summary

Land monorepo dogfood proof that one real source-exception change flows through
cargo-intent staged posture, a bridged proof obligation plan, cargo-proof
plan/dry-run, evidence collection, contradiction/repair characterization,
delegated precommit, merge-ready phase gate, and reconciliation. RIPR and Hawk
evidence remain stubbed via adapter parity contracts; no live binaries are
claimed.

## Landed Changes

- `scripts/three-product-dogfood-smoke.sh` — executable pipeline with JSON receipt
- Stage inventory and obligation-bridge fixtures under `tests/fixtures/three-product-dogfood/`
- Offline characterization tests and example receipt
- CI wiring in the `test` job after product binary builds

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test -p cargo-allow three_product_dogfood` | pass | characterization tests |
| `bash scripts/three-product-dogfood-smoke.sh` | pass | CI `test` job receipt artifact |
| `cargo-allow check --mode no-new` | pass | policy allows for new artifacts |

## Non-Goals

- Physical repository extraction
- Live RIPR or Hawk invocation
- Automatic intent→proof obligation export (bridge is explicit)

## Claim Boundary

**Establishes:** end-to-end CLI/process choreography for the three products in an
outside-monorepo consumer using workspace-built binaries; honest stub markers for
unavailable evidence providers.

**Does not establish:** packaged-candidate isolation (see #2605), semantic parity
of obligation bridging, proof execution, or release readiness.

## Remaining Work

- Replace obligation bridge with protocol-native export when available
- Promote RIPR/Hawk evidence from stubbed to live when provider binaries exist

## Rollback

Revert the dogfood script, fixtures, CI step, and policy allows. No runtime
product behavior changes to roll back.
