---
id: CARGO-ALLOW-SPEC-0010
kind: spec
status: accepted
owner: repo-infra
created: 2026-07-22
linked_proposal: CARGO-ALLOW-PROP-0010
linked_adrs:
  - CARGO-ALLOW-ADR-0002
support_tier_impact: advisory
policy_impact:
  - .allow/artifacts/doc-artifacts.toml
  - docs/status/SUPPORT_TIERS.md
---

# Spec: Three-Product Boundary Requirements

## Summary

Normative atomic requirements for separating cargo-allow, cargo-intent, and
cargo-proof inside one monorepo. Crate names and counts are referenced from
#2612 only; this spec owns product-boundary law.

## Behavior Contract

The repository must satisfy the following requirements.

### Authority and products

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
statement = "Physical repository extraction is not authorized by the three-product design package; monorepo implementation continues until Issue #2558 dogfood, Issue #2559 extraction-readiness evidence, and Issue #2605 exact-candidate interop all pass."
claim_class = "governance_structure"
```

### Dependency law

```toml cargo-allow-requirements

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
statement = "cargo-intent must not depend on cargo-proof crates; proof-engine may depend on intent-protocol but must not depend on intent-engine or cargo-intent crates."
claim_class = "dependency_law"

[[requirement]]
id = "shared-no-product-ontology"
generation = 1
status = "accepted"
statement = "shared protocol crates contain transport identities and envelopes only; they must not accumulate product-domain ontologies."
claim_class = "dependency_law"
```

### Sequencing

```toml cargo-allow-requirements

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
```

### Compatibility and deletion

```toml cargo-allow-requirements

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

The system must not:

- treat transitional `spec_system` modules as proof that cargo-allow owns intent;
- introduce cargo-allow → intent-engine library dependencies;
- add convenience crates from the #2612 explicit non-crate list without a
  reviewed topology change;
- infer product separation from matching CLI output alone;
- authorize repository extraction from documentation without #2559 evidence.

## Inputs

| Input | Required | Notes |
| --- | --- | --- |
| CARGO-ALLOW-PROP-0010 | Yes | Product definitions and disposition map |
| CARGO-ALLOW-ADR-0002 | Yes | Ownership and dependency direction |
| #2612 topology | Yes | Crate names, counts, stage gates |
| plans/three-product-crate-extraction.md | Yes | Wave sequencing and PR boundaries |

## Outputs

| Output | Required | Notes |
| --- | --- | --- |
| Registered artifact graph | Yes | `.allow/artifacts/doc-artifacts.toml` |
| Support-tier rows | Yes | `docs/status/SUPPORT_TIERS.md` |
| Reconstruction fixture | Yes | `tests/fixtures/three-product-design/` |

## Accepted States

- Three products named with independent claim boundaries in retained artifacts.
- Sequencing corrections recorded and consistent with #2612 stages.
- Artifact ledger links proposal → ADR → spec → plan.
- Fresh-agent fixture answers #2544 reconstruction questions.

## Rejected States

- cargo-allow library dependency on intent or proof crates.
- Undocumented crate additions outside #2612 topology.
- repo-edit before read-only cargo-intent vertical and #2601 cutover.
- Physical repository extraction claimed as authorized.
- Silent rewrite of CARGO-ALLOW-PROP-0001 without supersession link.

## Artifact Links

- Linked proposal: [CARGO-ALLOW-PROP-0010](../proposals/CARGO-ALLOW-PROP-0010-three-product-design.md)
- Linked ADR: [CARGO-ALLOW-ADR-0002](../adr/CARGO-ALLOW-ADR-0002-three-product-ownership.md)
- Linked implementation plan: [plans/three-product-crate-extraction.md](../../plans/three-product-crate-extraction.md)
- Linked support-tier surface: [CARGO-ALLOW-SUPPORT-0001](../status/SUPPORT_TIERS.md)
- Controlling issues: #2544, #2550, #2612, #2598, #2580, #2604, #2606, #2607

## Support-Tier Impact

Defines advisory experimental posture for cargo-intent and cargo-proof until
independent exact-candidate proof exists. Does not promote any product to
stable.

## Policy Impact

Registers spec in doc-artifacts.toml. Future #2580 manifest must encode
forbidden edges from this spec.

## Required Evidence

```bash
cargo test -p allow-policy spec_system_design_package --locked -- --nocapture
cargo test -p cargo-allow spec_design_artifact_links --locked -- --nocapture
cargo-allow check --profile spec-system --mode audit
```

## Acceptance Examples

### Example: Accepted

A documentation PR registers PROP-0010, ADR-0002, SPEC-0010, and PLAN-0010,
updates support tiers, and passes spec-system audit without moving Rust code.

### Example: Rejected

A PR creates `intent-source` as a convenience crate without updating the crate
topology owner (#2612) and architecture manifest (#2580) — violates
`crate-topology-owned-by-2612`.

## Non-Goals

- Implementing crate skeletons or module moves.
- Defining every future cargo-intent or cargo-proof command.
- Machine-enforcing dependency law (owned by #2580 implementation).
- Proving semantic parity (owned by #2606).

## Claim Boundary

This spec defines normative three-product boundary requirements and sequencing
law for repository-native authority. It does not prove code compliance, execute
proof commands, or certify product releases.

## Rollback Or Compatibility

Revert by superseding CARGO-ALLOW-SPEC-0010 and removing ledger links. Prior
PROP-0001/SPEC-0001 profile contracts remain for structural spec-system checks.
