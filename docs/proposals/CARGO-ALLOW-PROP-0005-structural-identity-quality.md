---
id: CARGO-ALLOW-PROP-0005
kind: proposal
status: accepted
owner: repo-infra
created: 2026-06-17
linked_specs:
  - CARGO-ALLOW-SPEC-0005
support_tier_impact: advisory
policy_impact: none
---

# Proposal: Structural Identity Quality

## Summary

cargo-allow should match source exceptions structurally enough to survive normal
refactors without pretending to be rustc. This proposal sequences fixture-backed
identity hardening for unsafe, panic, lint, and index findings.

## Problem

Structural identity v1 exists ([docs/identity.md](../identity.md)) but adoption
and diff posture still depend on knowing which identity fields are strong enough
per finding surface and which gaps remain.

## Proposed Shape

Inventory gaps by finding kind, add fixture matrices, and improve container,
receiver/target, and lint-target identity in narrow PRs. Keep the
source-syntax/no-build claim boundary.

## Non-Goals

- No Cargo metadata requirement.
- No macro expansion, type analysis, MIR, or build-aware claims.
- No broad scanner rewrite.

## Claim Boundary

This proposal sequences identity quality work. It does not prove semantic
correctness or rustc-level precision.
