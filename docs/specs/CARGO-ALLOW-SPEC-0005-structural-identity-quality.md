---
id: CARGO-ALLOW-SPEC-0005
kind: spec
status: accepted
owner: repo-infra
created: 2026-06-17
linked_proposal: CARGO-ALLOW-PROP-0005
linked_adrs: []
support_tier_impact: advisory
policy_impact: none
---

# Spec: Structural Identity Quality

## Summary

This spec defines the quality bar for cargo-allow structural identity: which
fields are required per finding surface, how fixtures prove identity stability,
and what remains outside source-syntax boundaries. It extends
[docs/identity.md](../identity.md) with a gap-inventory and PR queue.

## Identity Fields

Stable identity fields (from `cargo-allow.structural-identity.v1`):

| Field | Role |
| --- | --- |
| `path` | Normalized source-tree path |
| `crate_name` | Optional source-visible package hint |
| `module` / `container` | Source-syntax namespace |
| `ast_kind` | Syntax node kind |
| `symbol` / `callee` / `macro_name` / `lint` | Surface-specific selectors |
| `receiver_fingerprint` / `target_fingerprint` | Normalized operand fingerprints |
| `normalized_snippet_hash` | Stable local text hash; ignores whitespace and Rust comments, but not string/raw-string/numeric/source-token edits |
| `line_hint` / `column_hint` | Review hints only; not identity |
| `kind` / `family` | Finding-level classification |
| occurrence limit / scope selector | Policy weakening signals |

## Quality Bar

For each finding surface (unsafe, panic, lint, index/slice), cargo-allow should:

- emit the strongest source-visible identity available;
- preserve stable keys across line moves within the same source text;
- fail closed when multiple findings remain plausible;
- document scanner limitations with fixture examples.

## Fixture Strategy

| Fixture type | Purpose |
| --- | --- |
| Golden source snippets | Identity field extraction per surface |
| Move/refactor pairs | Stable key across line/container changes |
| Ambiguous contexts | Document fail-closed or tie-break behavior |
| Negative cases | No false identity from macros/types/build |

Fixture location target: `tests/fixtures/structural-identity/`.

## Gap Inventory

Living gaps are tracked in
[plans/structural-identity/gap-inventory.md](../../plans/structural-identity/gap-inventory.md).

## Proof Commands

| Command | Establishes | Does not establish |
| --- | --- | --- |
| Structural identity unit/integration tests | Field extraction and stable keys | Type-aware matching |
| `cargo-allow diff` characterization | Weakening/improvement on identity changes | Semantic refactor safety |
| `cargo-allow check --mode no-new` | No new unmatched findings | Identity completeness |

## Linked Artifacts

- Base identity reference: [docs/identity.md](../identity.md)
- Gap inventory: [plans/structural-identity/gap-inventory.md](../../plans/structural-identity/gap-inventory.md)
- Implementation plan: [plans/structural-identity/implementation-plan.md](../../plans/structural-identity/implementation-plan.md)

## Claim Boundary

This spec governs source-syntax identity quality. It does not claim macro
expansion, type-aware, MIR-level, build-aware, or unsafe soundness proof.
