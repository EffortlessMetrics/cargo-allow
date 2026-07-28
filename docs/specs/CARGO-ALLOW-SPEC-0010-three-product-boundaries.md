---
id: CARGO-ALLOW-SPEC-0010
kind: spec
status: superseded
owner: repo-infra
created: 2026-07-22
superseded_on: 2026-07-28
superseded_by: CARGO-ALLOW-SPEC-0011
linked_proposal: CARGO-ALLOW-PROP-0010
linked_adrs:
  - CARGO-ALLOW-ADR-0002
support_tier_impact: advisory
policy_impact:
  - .allow/artifacts/doc-artifacts.toml
  - docs/status/SUPPORT_TIERS.md
---

# Spec: Three-Product Boundary Requirements

## Historical status

This is the accepted generation-1 product-boundary and crate-creation record.
The 27-crate logical topology subsequently landed, so CARGO-ALLOW-SPEC-0011 now
owns convergence, package identity, dependency neutralization, compatibility
cutover, deletion, exact-candidate qualification, and release behavior.

The original requirements are retained below for provenance. They do not
silently become current sequencing authority.

### Exact supersession map

| Generation-1 requirement | Generation-2 replacement |
| --- | --- |
| `crate-topology-owned-by-2612` | `CARGO-ALLOW-SPEC-0011#identity-distinguishes-logical-package-lib` and the checked topology authorities |
| `rust-source-index-before-intent-engine` | `CARGO-ALLOW-SPEC-0011#shared-substrate-dependency-neutral` and `#intent-engine-single-read-only-compiler` |
| `repo-edit-deferred` | `CARGO-ALLOW-SPEC-0011#intent-engine-single-read-only-compiler` plus the read-only/mutation boundary in ADR-0002 |
| `shared-publish-false-until-2604` | `CARGO-ALLOW-SPEC-0011#package-topology-single-authority` and ADR-0003 publication posture |

The remaining generation-1 requirements are supporting historical statements
only where they do not conflict with CARGO-ALLOW-SPEC-0011.

## Original summary

Normative atomic requirements for separating cargo-allow, cargo-intent, and
cargo-proof inside one monorepo. At generation 1, concrete crate names and count
were referenced from Issue #2612 and the implementation plan still sequenced
crate creation.

## Original behavior contract

```toml cargo-allow-requirements
schema_version = "1.0"

[[requirement]]
id = "three-product-authority-split"
generation = 1
status = "accepted"
statement = "cargo-allow, cargo-intent, and cargo-proof each have exactly one primary product owner for load-bearing semantics; no concept has two canonical authorities."
claim_class = "governance_structure"

[[requirement]]
id = "crate-topology-owned-by-2612"
generation = 1
status = "accepted"
statement = "Concrete crate names and crate count are owned exclusively by issue #2612; other artifacts reference that topology and do not invent alternate crate sets."
claim_class = "governance_structure"

[[requirement]]
id = "repository-extraction-not-authorized"
generation = 1
status = "accepted"
statement = "Physical repository extraction is not authorized by the three-product design package; monorepo implementation continues until Issue #2558 dogfood, Issue #2605 exact-candidate interop, and Issue #2559 extraction-readiness evidence all pass, after which a separate explicit authorization is still required."
claim_class = "governance_structure"

[[requirement]]
id = "cargo-allow-no-intent-proof-lib-dep"
generation = 1
status = "accepted"
statement = "cargo-allow must not depend on cargo-intent or cargo-proof library crates; compatibility uses one-way process delegation to installed cargo-intent only."
claim_class = "dependency_law"

[[requirement]]
id = "intent-no-proof-dep"
generation = 1
status = "accepted"
statement = "cargo-intent must not depend on cargo-proof crates; intent-protocol is proof-engine's only cargo-intent dependency, while proof-engine may also depend on its proof and shared-substrate crates."
claim_class = "dependency_law"

[[requirement]]
id = "shared-no-product-ontology"
generation = 1
status = "accepted"
statement = "shared protocol crates contain transport identities and envelopes only; they must not accumulate product-domain ontologies."
claim_class = "dependency_law"

[[requirement]]
id = "rust-source-index-before-intent-engine"
generation = 1
status = "accepted"
statement = "rust-source-index extraction precedes full intent-engine migration so structural Rust subjects resolve without allow-rust."
claim_class = "extraction_sequence"

[[requirement]]
id = "repo-edit-deferred"
generation = 1
status = "accepted"
statement = "repo-edit extraction is deferred until after the read-only cargo-intent vertical and compatibility cutover (#2601)."
claim_class = "extraction_sequence"

[[requirement]]
id = "shared-publish-false-until-2604"
generation = 1
status = "accepted"
statement = "shared crates remain publish=false until #2604 adds them to reviewed publish/package order and a published product requires the dependency."
claim_class = "extraction_sequence"

[[requirement]]
id = "one-way-process-delegation"
generation = 1
status = "accepted"
statement = "Retained legacy cargo-allow intent commands delegate one-way to installed cargo-intent; no private fallback evaluator remains after cutover."
claim_class = "compatibility"

[[requirement]]
id = "no-duplicate-semantic-authority"
generation = 1
status = "accepted"
statement = "No unbounded transition may leave duplicate semantic implementations of intent or proof authority; #2598 records deletion conditions."
claim_class = "compatibility"
```

## Original rejected states

The system was not permitted to:

- treat transitional `spec_system` modules as proof that cargo-allow owns intent;
- introduce cargo-allow to intent-engine library dependencies;
- add convenience crates outside the Issue #2612 topology without a reviewed
  topology change;
- infer product separation from matching CLI output alone; or
- authorize repository extraction solely because the initial dogfood and
  extraction-readiness gates passed.

Those principles remain current where CARGO-ALLOW-SPEC-0011 does not define a
more exact generation-2 contract.

## Original inputs and outputs

| Input | Role at generation 1 |
| --- | --- |
| CARGO-ALLOW-PROP-0010 | Product definitions and disposition map |
| CARGO-ALLOW-ADR-0002 | Ownership and dependency direction |
| Issue #2612 | Logical crate names, count, and initial stage gates |
| Historical Wave-0 extraction-plan content (predecessor of `plans/three-product-crate-extraction.md`) | Original Wave-0 extraction sequence |

The current `plans/three-product-crate-extraction.md` path now contains the
generation-2 convergence plan. This table names its historical predecessor
content rather than treating the rewritten current file as a generation-1 input.

| Output | Role at generation 1 |
| --- | --- |
| `.allow/artifacts/doc-artifacts.toml` | Registered artifact graph |
| `docs/status/SUPPORT_TIERS.md` | Advisory support rows |
| `tests/fixtures/three-product-design/` | Fresh-agent reconstruction fixture |

## Current successor inputs

Generation-2 builders must additionally consume:

- CARGO-ALLOW-ADR-0003 package identity and versioning;
- CARGO-ALLOW-SPEC-0011 convergence and release requirements;
- `policy/product-crates.toml`;
- `policy/product-package-topology.toml`;
- `policy/product-move-ledger.toml`;
- `policy/extraction-shims.toml`; and
- `policy/extraction-parity.toml`.

## Non-goals

This historical spec does not:

- reopen the accepted three-product decision;
- override CARGO-ALLOW-SPEC-0011;
- prove current code compliance;
- authorize package publication or cargo-allow 0.2; or
- authorize physical repository extraction.

## Claim boundary

This file preserves the generation-1 product-boundary and crate-creation
requirements and records their exact successor. Current convergence, package,
cutover, deletion, and release claims come from CARGO-ALLOW-SPEC-0011 and its
linked machine authorities.
