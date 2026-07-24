---
id: CARGO-ALLOW-CLOSEOUT-0012
kind: closeout
status: accepted
owner: repo-infra
created: 2026-07-24
linked_plan: CARGO-ALLOW-PLAN-0010
linked_spec: CARGO-ALLOW-SPEC-0010
support_tier_impact: none
policy_impact:
  - policy/three-product-simplification.toml
  - policy/allow.toml
---

# Closeout: Three-Product Simplification Audit (#2208)

## Summary

Classify extraction-wave abstractions with mandatory simplification labels and
remove the `cargo-allow::io` write shim by inlining `repo-edit` helpers into
`command_support`. No crates deleted in this packet.

## Landed Changes

- `policy/three-product-simplification.toml` — classification inventory
- `scripts/three-product-simplification-audit.sh` — inventory + removal proof
- Removed `crates/cargo-allow/src/io.rs`; emission helpers live in `command_support`

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test -p cargo-allow three_product_simplification` | pass | characterization |
| `bash scripts/three-product-simplification-audit.sh` | pass | audit receipt |
| `cargo test -p cargo-allow --bins` | pass | io_tests in command_support |

## Claim Boundary

**Establishes:** simplification taxonomy applied to extraction artifacts; one
concrete shim removal before any repository split.

**Does not establish:** crate deletion, spec-system retirement, or automatic
usage-based pruning.

## Remaining Work

- Retire embedded spec-system modules after #2568 closure
- Reclassify PostMvpUnproven entries after dogfood + interop evidence

## Rollback

Restore `io.rs` module and revert inventory; no policy ledger behavior changes.
