---
id: CARGO-ALLOW-PROP-0006
kind: proposal
status: accepted
owner: repo-infra
created: 2026-06-17
linked_specs:
  - CARGO-ALLOW-SPEC-0006
support_tier_impact: none
policy_impact: none
---

# Proposal: Adoption-Trust Release 0.1.10

## Summary

`0.1.10` is an adoption-trust patch after `0.1.9`, not a `0.2.0` milestone.
It should improve release automation confidence, migration-parity groundwork,
test hardening, and honest readiness language without claiming full migration
parity, `.allow` imports, or zero-gap provider readiness.

## Target Claim

```text
cargo-allow 0.1.10 improves adoption trust:
release automation, migration-parity groundwork, test hardening,
source-tree governance docs, and profile readiness documentation.
```

## Non-Claims

- Not full `0.2.0` migration parity.
- Not full `.allow`/import support.
- Not AST/type/build-aware analysis.
- Not `ripr+` / `unsafe-review+` zero unless providers are fixed.
- Not stable spec-system support promotion.

## Prerequisites

- Provider-tracked readiness policy recorded.
- Trusted Publishing configured on all 10 crates (or documented token fallback).
- `workflow_dispatch` dry-run evidence on `main`.
- Sufficient post-`0.1.9` improvements under `[Unreleased]`.

## Claim Boundary

This proposal sequences the `0.1.10` cut. It does not authorize version bump
or tag push without explicit release authorization.
