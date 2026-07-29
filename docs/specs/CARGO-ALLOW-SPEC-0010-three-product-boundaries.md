---
id: CARGO-ALLOW-SPEC-0010
kind: spec
status: superseded
owner: repo-infra
created: 2026-07-22
superseded_on: 2026-07-29
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

This is the accepted generation-1 product-boundary and crate-extraction record.
The maximum 27-package scaffold subsequently landed and a simplification review
ratified a 22-package target. CARGO-ALLOW-SPEC-0011 now owns current convergence,
package survival, semantic cutover, exact-candidate and release behavior.

The original requirements are retained below for provenance. They do not
silently remain current sequencing authority.

## Exact supersession map

| Generation-1 requirement | Generation-2 replacement |
| --- | --- |
| `crate-topology-owned-by-2612` | `CARGO-ALLOW-SPEC-0011#observed-and-target-topologies-distinct` and the checked generation-2 authority |
| `rust-source-index-before-intent-engine` | `CARGO-ALLOW-SPEC-0011#shared-substrate-dependency-neutral` and `#intent-engine-single-read-only-compiler` |
| `repo-edit-deferred` | `CARGO-ALLOW-SPEC-0011#repo-edit-neutral-selected-closure` and the ADR-0002 read-only/mutation boundary |
| `shared-publish-false-until-2604` | `CARGO-ALLOW-SPEC-0011#package-topology-single-authority` and ADR-0003 publication posture |

The remaining generation-1 requirements are supporting historical statements
only where they do not conflict with CARGO-ALLOW-SPEC-0011.

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
statement = "Physical repository extraction is not authorized by the three-product design package; monorepo implementation continues until independent package, dogfood, compatibility, support and rollback evidence passes, after which a separate explicit authorization is still required."
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
statement = "cargo-intent must not depend on cargo-proof crates; intent-protocol is proof-engine's only cargo-intent dependency."
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
statement = "repo-edit extraction is deferred until after the first read-only cargo-intent vertical."
claim_class = "extraction_sequence"

[[requirement]]
id = "shared-publish-false-until-2604"
generation = 1
status = "accepted"
statement = "shared crates remain unpublished until package topology adds them to a reviewed product closure and publish order."
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
statement = "No unbounded transition may leave duplicate semantic implementations of intent or proof authority; the move ledger records deletion conditions."
claim_class = "compatibility"
```

## Historical rejected states

The system was not permitted to:

- treat transitional `spec_system` modules as proof that cargo-allow owns intent;
- introduce cargo-allow to intent-engine library dependencies;
- infer product separation from matching CLI output alone;
- authorize repository extraction solely because an integrated smoke passed; or
- leave duplicate semantic authority without an exact deletion denominator.

Those principles remain current where CARGO-ALLOW-SPEC-0011 does not define a
more exact contract.

## Current successor inputs

Generation-2 builders consume:

- CARGO-ALLOW-PROP-0010;
- CARGO-ALLOW-ADR-0002 and ADR-0003;
- CARGO-ALLOW-SPEC-0011;
- the generation-2 architecture and package authorities;
- `policy/product-move-ledger.toml`;
- `policy/extraction-shims.toml`; and
- `policy/extraction-parity.toml`.

## Non-goals

This historical specification does not override CARGO-ALLOW-SPEC-0011, prove
current code compliance, authorize package publication or cargo-allow 0.2, or
authorize physical repository extraction.

## Claim boundary

This file preserves the generation-1 product-boundary and extraction requirements
and records their exact successor. Current topology, convergence, cutover,
package and release claims come from CARGO-ALLOW-SPEC-0011 and its checked
machine authorities.
