---
id: CARGO-ALLOW-SPEC-0012
kind: spec
status: draft
owner: repo-infra
created: 2026-08-24
linked_proposal:
standalone_reason: The source-exception ledger is an existing core product contract that predates the spec series; this draft formalizes the current behavior without introducing a new runtime surface.
linked_adrs: []
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
  - .allow/artifacts/doc-artifacts.toml
---

# Spec: Source-Exception Ledger

## Summary

The source-exception ledger is cargo-allow's authoritative record of retained
source-tree exceptions and the conditions under which they remain reviewable.
It scopes findings, preserves accountability and evidence, supplies lifecycle
pressure, and gives matching and reporting surfaces one stable policy model.

This specification formalizes the current ledger contract described in the
[Source Exception Ledger](../source-exception-ledger.md). It does not authorize
new finding families or change the default command behavior.

## Behavior Contract

The system must:

- treat `policy/allow.toml` (or an explicitly selected compatible ledger) as a
  versioned policy document rather than an unstructured allowlist;
- validate top-level identity and posture fields, including schema, policy,
  owner, and status where present;
- represent each retained exception with a stable ID, governed kind, scope,
  ownership, classification, reason, evidence, lifecycle, and selector data;
- keep exact paths and bounded globs as source-tree scopes, not as proof that a
  whole tree is semantically safe;
- preserve typed evidence and local traceability links as reviewable inputs;
- treat `last_seen` as a review hint, not as durable per-occurrence identity;
- expose invalid, stale, expired, review-due, drifted, debt, and matched states
  without silently converting one state into another;
- reject malformed, ambiguous, out-of-scope, or weakly identified policy
  entries before they can authorize a finding.

The system must not:

- execute Cargo, rustc, tests, proof tools, network calls, or repository code
  while loading or validating the ledger;
- infer ownership, rationale, evidence, or approval from a filename or line;
- use a broad path/glob alone as durable identity for a source-code exception;
- treat `baseline_debt` as reviewed approval or allow it to erase ownership and
  lifecycle distinctions;
- silently merge unrelated ledger dialects or profile/federation configuration;
- claim that a retained entry proves runtime safety, reachability, or semantic
  correctness.

## Inputs and Outputs

| Input | Required | Notes |
| --- | --- | --- |
| Selected ledger path | yes | CLI override, federation-selected path, or documented discovery path. |
| Ledger TOML | yes | Must satisfy the current schema and source-tree scope rules. |
| Source-tree inventory | for matching/check | Used separately to determine whether scoped findings exist. |

| Output | Required | Notes |
| --- | --- | --- |
| Validated ledger state | yes | Shared by matching, audit, list, explain, and diff projections. |
| Diagnostics/worklist | when invalid or actionable | Names the entry and repair route without executing proof. |
| Receipt/report projection | command-dependent | Retains provenance and claim-boundary wording. |

## Accepted States

- A reviewed retained entry has a stable ID, governed kind, bounded scope,
  concrete owner, classification, reason, and appropriate lifecycle metadata.
- A source-code entry includes at least one structural selector field in
  addition to its path or glob scope.
- Evidence and links are non-empty, unique, whitespace-normal, and use the
  accepted typed/local-path vocabulary where applicable.
- A generated `baseline_debt` entry is explicitly classified and may use the
  reserved `unowned` owner only under the generated baseline contract.
- A ledger can be selected through an explicit CLI path or the documented
  discovery order, with the selection provenance preserved in diagnostics.

## Rejected States

- Duplicate IDs, unknown kinds, invalid token metadata, or missing required
  accountability fields.
- Absolute, parent-traversing, whole-tree, or otherwise unsupported scopes.
- Source-code entries whose only selector is a path or glob.
- Local traceability links that escape the source tree, contain wildcards, or
  point to absent files when evidence validation is requested.
- Removal or laundering of owner, reason, classification, evidence, created
  date, or lifecycle pressure without the corresponding diff posture.

## Artifact Links

- Source contract: [Source Exception Ledger](../source-exception-ledger.md).
- Weakening rules: [Policy Weakening](../policy-weakening.md).
- Structural identity: [Structural Identity V1](../identity.md).
- Registry: [doc-artifacts.toml](../../.allow/artifacts/doc-artifacts.toml).

## Required Evidence

- TOML parsing and validation tests for required fields, token normalization,
  scopes, evidence, and generated baseline debt.
- Round-trip tests showing that audit, list, explain, and JSON/Markdown outputs
  preserve the same entry identity and state.
- Negative fixtures for broad scopes, missing selectors, invalid links, and
  dialect/configuration confusion.

## Non-Goals

- Cargo metadata, compilation, type analysis, macro expansion, MIR, or runtime
  reachability.
- Proving that an exception is safe or that its rationale is correct.
- Replacing the separate spec-system profile or creating a generic bypass list.

## Claim Boundary

This spec defines the structure and validation of retained source-exception
policy. It supports the claim that a ledger entry is structurally valid and
reviewable under the selected contract. It does not prove semantic safety,
test adequacy, coverage, or release readiness.

## Rollback Or Compatibility

The specification is documentation-only while `status = "draft"`. Existing
ledger parsing and command behavior remain authoritative. A future accepted
revision must name schema or matching changes explicitly and preserve a
compatibility adapter or migration path for existing ledgers.
