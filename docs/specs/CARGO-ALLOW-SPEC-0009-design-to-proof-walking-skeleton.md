---
id: CARGO-ALLOW-SPEC-0009
kind: spec
status: accepted
owner: repo-infra
created: 2026-07-15
linked_proposal:
standalone_reason: The first design-to-proof walking skeleton is a bounded self-hosted control-plane invariant; no separate product proposal is required.
linked_adrs: []
support_tier_impact: advisory
policy_impact:
  - .allow/profiles/spec-system.toml
  - .allow/spec-system/slices/self-hosted-runtime-promotion-v1.toml
  - .allow/artifacts/doc-artifacts.toml
---

# Spec: Design-to-Proof Walking Skeleton

## Summary

This spec defines the first retained normative requirement and PR-local
implementation-slice rule for cargo-allow's design-to-proof system. It keeps
specification acceptance, runtime implementation, execution evidence, and
support claims as separate states.

## Normative Requirements

```toml cargo-allow-requirements
schema_version = "1.0"

[[requirement]]
id = "spec-only-runtime-promotion"
generation = 1
lifecycle = "accepted"
statement = "A spec-or-policy-only slice cannot mark runtime behavior implemented, runtime proof current, or runtime support promoted without compatible implementation and evidence dispositions."
claim_class = "runtime_behavior"
```

## Behavior Contract

The system must:

- allow a spec-or-policy slice to define or amend an accepted runtime
  requirement while implementation and evidence remain explicitly outstanding;
- reject an `Implemented` runtime transition when the slice does not carry a
  compatible implementation disposition;
- reject a current runtime-proof claim when no non-empty receipt reference is
  present;
- reject runtime support promotion unless implementation, receipt-backed
  evidence, and a named support claim are all present;
- evaluate the proposed transition without mutating accepted requirement or
  support state when validation fails.

The system must not:

- treat a merged spec as runtime implementation;
- treat a test or receipt reference as current merely because it is named;
- infer support promotion from document status, issue state, or generated
  output;
- store worker, branch, PR, CI, reviewer, priority, or timeline state in the
  normative implementation slice.

## Accepted States

- `SpecOrPolicyChange` keeps the runtime requirement `Accepted` while
  implementation, evidence, and support promotion remain outstanding.
- A later compatible behavior slice may propose `Implemented` with the required
  implementation and evidence dispositions.

## Rejected States

- Spec-only `Implemented` transition with implementation outstanding.
- Current runtime proof without a receipt reference.
- Runtime support promotion without implementation and evidence closure.
- Unknown requirement or slice schema generation.

## Required Evidence

- Exact owner tests for the positive accepted state and each rejected promotion.
- A deliberately broad neighboring rejection test retained as weak evidence for
  later RIPR test-grip comparison.
- The PR-local slice at
  `.allow/spec-system/slices/self-hosted-runtime-promotion-v1.toml`.

## Non-Goals

- Complete requirement lifecycle or change-class taxonomy.
- Compiled requirement/evidence graph.
- Test discovery, RIPR integration, proof execution, CLI authoring, or LSP.
- Runtime support promotion from this spec or its first implementation slice.

## Claim Boundary

This spec defines one self-hosted runtime-promotion invariant and its minimum
normative source. It does not prove implementation correctness, test adequacy,
current runtime execution, release readiness, or support-tier promotion.

## Rollback Or Compatibility

The first implementation may be reverted by removing the optional slice root,
self-hosted slice, parser modules, and this registered spec. Existing
spec-system profiles remain compatible because the new root is optional and
legacy document artifacts retain their prior behavior.
