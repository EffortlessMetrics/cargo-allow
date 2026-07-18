---
id: CARGO-ALLOW-SPEC-0009
kind: spec
status: accepted
owner: repo-infra
created: 2026-07-16
linked_proposal:
standalone_reason: The first design-to-proof walking skeleton is a bounded self-hosted control-plane invariant; no separate product proposal is required.
linked_adrs: []
support_tier_impact: advisory
policy_impact:
  - .allow/spec-system/slices/self-hosted-runtime-promotion-v1.toml
  - .allow/artifacts/doc-artifacts.toml
---

# Spec: Design-to-Proof Walking Skeleton

## Summary

This specification defines the first retained normative requirement and
PR-local implementation-claim rule for cargo-allow's design-to-proof system.
Normative acceptance, implementation posture, execution evidence, and support
claims remain independent.

## Normative Requirements

```toml cargo-allow-requirements
schema_version = "1.0"

[[requirement]]
id = "spec-only-runtime-promotion"
generation = 1
status = "accepted"
statement = "A spec-or-policy-only slice cannot publish an implemented runtime claim, current runtime proof, or promoted runtime support without compatible implementation and evidence closure."
claim_class = "runtime_behavior"
```

## Behavior Contract

The system must:

- allow a spec-or-policy slice to reference an accepted runtime requirement
  while implementation and evidence remain explicitly outstanding;
- reject an implemented runtime claim from a spec-or-policy-only slice;
- reject current runtime evidence when no non-empty receipt reference exists;
- reject runtime support promotion unless implementation, receipt-backed
  evidence, and a named support claim are all present;
- derive runtime classification from the normative requirement rather than
  PR-local metadata;
- evaluate the proposed claim without mutating requirement or support state when
  validation fails.

The system must not:

- treat an accepted specification as runtime implementation;
- let a slice override the requirement's claim class;
- treat a receipt reference as current merely because it is named;
- infer support promotion from document status, issue state, or generated output;
- store branch, PR, worktree, head, CI, reviewer, worker, progress, or session
  state in the normative implementation slice.

## Accepted State

A `SpecOrPolicyChange` slice may reference the accepted runtime requirement while
its implementation claim and evidence remain outstanding and support remains
unchanged.

## Rejected States

- Spec-or-policy-only implemented runtime claim.
- Current runtime evidence without a receipt reference.
- Runtime support promotion without implementation and evidence closure.
- Unknown requirement or schema generation.
- PR-local runtime classification or mutable Git basis fields.

## Required Evidence

- Exact owner tests for the accepted state and each rejected promotion.
- A deliberately broad neighboring rejection test retained as weak evidence for
  later RIPR test-grip comparison.
- The PR-local slice at
  `.allow/spec-system/slices/self-hosted-runtime-promotion-v1.toml`.

## Non-Goals

- Complete requirement-status or change-class taxonomy.
- Compiled requirement/evidence graph.
- Test discovery, RIPR integration, proof execution, CLI authoring, or LSP.
- Runtime support promotion from this specification or its first slice.

## Claim Boundary

This specification defines one self-hosted runtime-promotion invariant and its
minimum normative source. It does not prove implementation correctness, test
adequacy, current runtime execution, release readiness, or support promotion.

## Rollback Or Compatibility

The source can be reverted by removing this registered specification and its
independent implementation-slice file. Exact repository/head identity remains a
generated proof concern, so rebasing does not require editing this normative
source.
