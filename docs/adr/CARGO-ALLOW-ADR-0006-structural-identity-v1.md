---
id: CARGO-ALLOW-ADR-0006
kind: adr
status: accepted
owner: repo-infra
created: 2026-08-27
linked_proposal: CARGO-ALLOW-PROP-0005
linked_spec: CARGO-ALLOW-SPEC-0005
supersedes: none
superseded_by: none
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# ADR: Structural Identity V1

## Context

Line numbers are useful for navigation but change whenever source text moves.
Using them as durable identity turns an unchanged exception into new debt and
encourages broad path-only approvals. The scanner needs a stable identity that
can survive ordinary line movement while remaining honest about what source
syntax can establish.

## Decision

Use `cargo-allow.structural-identity.v1` as the source-syntax identity model.
Its stable key is the length-prefixed concatenation of the structural fields:
language, optional source-derived crate name, module, container, AST kind,
symbol, callee, macro name, lint, receiver fingerprint, target fingerprint,
and normalized snippet hash.

Line and column values are hints only and are excluded from the stable key.
Matching must combine kind and path compatibility with the strongest selector
fields available for the finding surface. If multiple findings remain
plausible, strict review contexts fail closed rather than selecting one by
position or guesswork.

The identity remains source-tree and parser-visible. It must not require Cargo
metadata, compilation, type checking, macro expansion, MIR, control-flow, or
data-flow analysis.

## Consequences

### Positive

- Ordinary line movement does not invalidate a structurally unchanged entry.
- Selectors can express the difference between repeated source surfaces.
- Ambiguity remains visible instead of becoming accidental authorization.
- Identity claims stay aligned with the scanner's syntax-only boundary.

### Negative

- A structural identity can still change when meaningful source tokens change.
- Parser-visible names are not resolved type identities or macro-generated
  identities.
- Policy authors and tools must carry more fields than a path and line number.

## Non-Goals

- Proving reachability, type identity, semantic equivalence, or macro expansion.
- Making broad path-only selectors as strong as structural selectors.
- Freezing future identity versions; incompatible additions require a new
  schema version.

## Claim Boundary

This ADR records the durable source-syntax identity and matching posture. It
does not prove semantic stability, compiler behavior, test discrimination, or
that a matched exception remains acceptable.

## Rollback Or Supersession

Supersede V1 only through an explicitly versioned identity contract. Reports
and receipts that rely on this field set must continue to expose the V1 schema
identifier during any migration.
