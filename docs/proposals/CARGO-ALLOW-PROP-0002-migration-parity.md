---
id: CARGO-ALLOW-PROP-0002
kind: proposal
status: accepted
owner: repo-infra
created: 2026-06-16
linked_specs:
  - CARGO-ALLOW-SPEC-0002
support_tier_impact: advisory
policy_impact:
  - policy/doc-artifacts.toml
  - policy/allow.toml
---

# Proposal: Migration and Evidence Parity

## Summary

cargo-allow should strengthen migration and evidence parity so repositories can
retire bespoke AST/TOML allowlist xtasks with side-by-side proof, durable
receipts, and documented deltas. This is the execution lane for the `0.2.0`
milestone claim:

```text
cargo-allow can replace bespoke AST/TOML allowlist xtasks.
```

## Problem

Many repositories still enforce source exceptions through bespoke xtasks and
legacy policy files. cargo-allow already provides compat bridges and migration
writers, but adoption still depends on ad hoc side-by-side runs, uneven
evidence, and chat-local memory about remaining deltas.

Maintainers need a governed lane that:

- names which compat surfaces are parity-ready versus still bridging;
- records side-by-side evidence without suppressing findings;
- sequences PR-sized migration work toward durable `policy/allow.toml` shape;
- keeps claim boundaries honest about scanner limits.

## Users And Surfaces

- Maintainers replacing xtask allowlists lane by lane.
- Reviewers judging whether a remaining delta is documented and acceptable.
- Agent operators executing bounded migration work items from the active goal.
- Product surface: existing `cargo-allow migrate`, `check --compat`, and
  migration documentation; no new default scan behavior required for this lane.

## Proposed Shape

Register a new active goal, proposal, spec, and implementation plan for
migration parity. Sequence work in PR-sized slices that:

- strengthen side-by-side evidence for existing compat kinds;
- close documented migration-scope gaps called out in adoption docs;
- improve migration summaries, closeout queues, and receipt traceability;
- keep no-new and strict semantics intact during transition.

## Non-Goals

- Do not claim full xtask replacement until side-by-side proof exists per lane.
- Do not broaden scanner identity beyond current source-syntax boundaries.
- Do not publish `0.2.0` or promote support tiers without explicit release
  authorization.
- Do not execute proof providers from cargo-allow's own scan.

## Claim Boundary

This proposal sequences migration parity work. It does not prove parity,
execute side-by-side checks, or replace repository-specific xtask evidence.
