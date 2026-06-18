---
id: CARGO-ALLOW-PROP-0007
kind: proposal
status: accepted
owner: repo-infra
created: 2026-06-18
linked_specs:
  - CARGO-ALLOW-SPEC-0007
linked_adrs:
  - CARGO-ALLOW-ADR-0001
support_tier_impact: advisory
policy_impact:
  - .allow/artifacts/doc-artifacts.toml
  - policy/allow.toml
---

# Proposal: Multi-Ledger Federation (F0 Design)

## Summary

cargo-allow repositories often carry more than one durable policy or governance
ledger: the source-exception ledger, spec-system profile state, legacy xtask
compat files, and read-only import roots. Federation defines how those ledgers
coexist with explicit roles, deterministic precedence, divergence reporting,
and receipt provenance — without silent merging.

This proposal is **design-first (F0)**. It accepts the behavior contract and
architecture decision recorded in CARGO-ALLOW-SPEC-0007 and
CARGO-ALLOW-ADR-0001. Runtime federation (F1) remains blocked until this design
lands.

## Problem

Per-lane posture and policy dialect discovery (#1473, #1470) landed, but
repositories still need a governed model for:

- which ledger is canonical for each graph kind;
- how legacy, mirror, and imported ledgers relate during migration;
- how duplicate IDs and dialect mismatches surface instead of collapsing;
- how receipts prove which ledgers contributed to a check;
- how drain windows retire compatibility paths without chat-local memory.

Without a federation contract, agents and maintainers risk treating compat
bridges, import roots, or legacy fallbacks as silently canonical policy.

## Users And Surfaces

- Maintainers operating repositories with multiple policy files or governance
  roots during migration.
- Reviewers judging whether a remaining delta is documented federation
  divergence versus an undocumented merge.
- Agent operators executing bounded F1 implementation slices after F0 acceptance.
- Product surfaces: default `check`, `--compat`, `--profile spec-system`,
  migration writers, doctor diagnostics, and receipt emission — federation
  extends provenance and reporting; it does not broaden scanner identity.

## Proposed Shape

Register a dedicated federation proposal, spec, ADR, and implementation plan
(F0 design only):

| Artifact | Role |
| --- | --- |
| CARGO-ALLOW-PROP-0007 | Why federation exists and success criteria |
| CARGO-ALLOW-SPEC-0007 | Normative federation behavior contract |
| CARGO-ALLOW-ADR-0001 | Durable precedence and no-silent-merge decision |
| CARGO-ALLOW-PLAN-0007 | F0 design registration; F1+ runtime slices blocked |

Core concepts:

```text
Ledger role     Meaning
canonical       Authoritative write target for a graph kind
mirror          Read-only synchronized view of a canonical ledger
imported        Read-only external graph input; promotion is explicit

Precedence      Explicit CLI > .allow/ owned > policy/ legacy > imported advisory
Duplicates      Same stable ID in multiple ledgers → divergence report, not merge
Dialects        Foreign policy dialects skipped with named diagnostics
Drain windows   Time-bounded legacy compatibility with review/expiry in closeouts
Receipts        Record ledger provenance, federation version, and conflict posture
```

## Relationship To Prior Art

- [CARGO-ALLOW-PROP-0002](CARGO-ALLOW-PROP-0002-migration-parity.md) and
  [CARGO-ALLOW-SPEC-0002](../specs/CARGO-ALLOW-SPEC-0002-migration-parity.md)
  govern migration parity lanes; federation composes with compat bridges rather
  than replacing them.
- [CARGO-ALLOW-PROP-0004](CARGO-ALLOW-PROP-0004-allow-import-profile.md) and
  [CARGO-ALLOW-SPEC-0004](../specs/CARGO-ALLOW-SPEC-0004-allow-import-profile.md)
  define `.allow/` ownership and import-root posture; federation adds multi-ledger
  roles and precedence across those roots.
- C2 profile resolution (#1748) established config provenance and advisory
  conflict diagnostics instead of silent merge; federation generalizes that
  pattern.

## Success Criteria

- Proposal, spec, and ADR are registered in `.allow/artifacts/doc-artifacts.toml`.
- Active goal records F0 design complete and F1 runtime implementation blocked
  pending F0 merge.
- Gap inventory and closeout queues reference the federation artifact IDs.
- Spec-system audit and no-new guard pass after registration.

## Non-Goals (F0)

- No runtime federation resolver, scanner, or receipt schema changes in F0.
- No silent promotion of imported or compat ledgers into canonical policy.
- No claim of macro-expanded, type-aware, MIR-level, or build-aware federation.
- No release or support-tier promotion without explicit authorization.

## Risks

| Risk | Mitigation |
| --- | --- |
| Over-broad federation scope | F0 limits to documented roles, precedence, and reporting contract |
| Silent merge regression | ADR mandates named diagnostics; spec rejects accepted silent-merge states |
| Duplicate ID ambiguity | Stable federation IDs with origin ledger and divergence reporting |
| Legacy path never drains | Drain windows require closeout owner, review_after, and expiry |

## Claim Boundary

This proposal accepts federation design artifacts. It does not implement
federation, execute side-by-side proof across all lanes, or replace
repository-specific xtask evidence.
