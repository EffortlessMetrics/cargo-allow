---
id: CARGO-ALLOW-SPEC-0014
kind: spec
status: draft
owner: repo-infra
created: 2026-08-24
linked_proposal:
standalone_reason: Policy matching is the existing source-exception authorization contract implemented across the core and matching crates; this draft records its identity and fail-closed boundaries without changing matching behavior.
linked_adrs: []
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
  - .allow/artifacts/doc-artifacts.toml
---

# Spec: Policy Matching

## Summary

Policy matching determines whether a parser-visible finding is covered by a
retained ledger entry. Matching combines governed kind and family, normalized
source-tree scope, and the strongest structural selector fields available for
that finding. Location hints help humans review results but are not durable
identity.

This specification formalizes the current matching boundary described in
[Structural Identity V1](../identity.md) and the selector rules in the [Source
Exception Ledger](../source-exception-ledger.md).

## Behavior Contract

The system must:

- normalize repository-relative paths before scope comparison;
- require compatible kind/family identity before a policy entry can match;
- use exact paths and bounded globs only as scope, not as a substitute for
  source-code structural identity;
- compare the strongest available selector fields, including AST kind,
  container, callee/macro/lint/symbol, fingerprints, and normalized snippet
  identity as applicable;
- treat line and column as hints for review and tie-breaking, never as stable
  identity;
- preserve deterministic matching across reports, audit, check, explain, and
  diff;
- fail closed or report ambiguity when multiple entries remain plausible after
  the supported selector comparison;
- retain matched, stale, drifted, invalid, and unmatched outcomes distinctly.

The system must not:

- invoke Cargo, rustc, macro expansion, type checking, MIR, control-flow, or
  data-flow analysis to strengthen a source-syntax match;
- assume two textually similar sites are semantically equivalent;
- let a path-only source-code entry silently authorize repeated or unrelated
  findings outside an explicit migration policy;
- use evidence links, reasons, or human display text as identity fields;
- reinterpret a moved line as a new finding when the stable structural identity
  is unchanged.

## Inputs and Outputs

| Input | Required | Notes |
| --- | --- | --- |
| Finding | yes | Parser-visible finding with kind, family, path, and available identity fields. |
| Validated policy entry | yes | Supplied by the ledger contract. |
| Source-tree normalization rules | yes | Establishes repository-relative path identity. |

| Output | Required | Notes |
| --- | --- | --- |
| Match decision | yes | Matched, unmatched, stale, invalid, or ambiguous. |
| Match explanation | command-dependent | Names compared fields and limitations. |
| Stable identity/receipt fields | when serialized | Excludes line/column hints from durable keys. |

## Accepted States

- A source-code finding matches only a compatible entry with sufficient
  structural selectors for the finding family.
- Moving an unchanged finding without changing its structural fields preserves
  its stable identity while updating location hints.
- A non-Rust file finding uses the bounded file/presence selector contract and
  does not gain source-code semantic claims.
- A stale entry remains visible for repair instead of being silently deleted or
  treated as a current authorization.
- An ambiguous candidate set is surfaced with its competing identities and
  does not become an arbitrary match.

## Rejected States

- Kind/family mismatch treated as a match.
- Absolute, parent-traversing, or unnormalized path identity accepted as a
  repository-relative match.
- A source-code entry with only a broad path/glob accepted as durable identity
  outside an explicit migration mode.
- Parser failure, missing identity, or incomplete inventory represented as a
  confident clean match.

## Artifact Links

- Identity contract: [Structural Identity V1](../identity.md).
- Policy contract: [Source Exception Ledger](../source-exception-ledger.md).
- Diff implications: [Policy Weakening](../policy-weakening.md).
- Registry: [doc-artifacts.toml](../../.allow/artifacts/doc-artifacts.toml).

## Required Evidence

- Refactor-pair fixtures for line movement, function movement, module changes,
  selector changes, and same-looking distinct sites.
- Matching tests for each supported finding family and selector combination.
- Negative fixtures for kind/family mismatch, broad path-only policy, invalid
  paths, ambiguous candidates, and parser/inventory partial state.

## Non-Goals

- Semantic equivalence, type identity, macro expansion, reachability, or proof
  that a matched exception is safe.
- Automatic policy refresh, approval, or evidence generation.
- Replacing the ledger parser, diff classifier, or report renderer.

## Claim Boundary

This spec defines deterministic matching of parser-visible findings to
validated policy entries. A match means only that the finding is covered by the
selected source-syntax policy under the stated identity contract. It does not
prove the finding's runtime behavior or the exception's safety.

## Rollback Or Compatibility

The draft preserves Structural Identity V1 and existing selector vocabulary.
Any future identity generation must remain explicit, keep v1 readable for
historical receipts, and provide a reviewed migration when stable-key fields or
matching precedence change.
