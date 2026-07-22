---
id: CARGO-ALLOW-ADR-0002
kind: adr
status: accepted
owner: repo-infra
created: 2026-07-22
linked_proposal: CARGO-ALLOW-PROP-0010
linked_spec: CARGO-ALLOW-SPEC-0010
supersedes: none
superseded_by: none
support_tier_impact: advisory
policy_impact:
  - .allow/artifacts/doc-artifacts.toml
---

# ADR: Three-Product Ownership and Dependency Direction

## Context

Issue #2550 settled three product authorities inside one monorepo. Issue #2612
ratified the
concrete crate graph, stage gates, and forbidden convenience crates. Current
implementation still embeds intent and proof semantics inside cargo-allow
transitional modules (`allow-policy::spec_system`, `cargo-allow::spec_system*`).

The failure mode to prevent is **accidental authority merge**: treating
transitional source locations as evidence that cargo-allow owns intent
compilation or proof orchestration, or introducing library dependencies that
would make cargo-allow require cargo-intent/cargo-proof at build time.

## Decision

Adopt three product owners with fixed dependency direction. Crate names and
counts are owned exclusively by #2612; this ADR owns **ownership** and
**forbidden edges**.

### Product ownership

```text
cargo-allow
  source-exception domain, scanner, matching, lifecycle, mutation, diagnostics,
  and the cargo-allow binary as thin application facade

cargo-intent
  authored repository-intent sources, authority compilation, private graph IR,
  phase obligations, read-only domain queries, and later intent-edit settlement

cargo-proof
  exact-snapshot proof planning, explicit provider execution, receipt validation,
  currentness, contradictions, and phase-gate composition
```

### Shared substrate

```text
repo-protocol     identity and transport envelopes only
repo-snapshot     exact committed/index/worktree source views
repo-edit         safe filesystem mutation (deferred until read-only cutover)
rust-source-index structural Rust subject inventory (before full intent-engine)
```

Shared crates exist because at least two products need the same load-bearing
implementation. Shared does not imply stable public API or crates.io
publication.

### Allowed dependency edges

```text
repo-protocol → repo-snapshot, repo-edit, rust-source-index

intent-model → intent-protocol → intent-engine → cargo-intent, intent-edit

proof-protocol → proof-provider-api → proof-engine, proof-adapter-*, cargo-proof

proof-engine → intent-protocol, repo-snapshot, proof-protocol, proof-provider-api

cargo-allow implementation crates → repo-protocol, repo-snapshot (where moved
  implementation requires it; repo-edit only after parity cutover)
```

### Forbidden dependency edges

```text
cargo-allow product → intent-model / intent-engine / proof-engine / cargo-proof
intent-model / intent-protocol → repo-snapshot / filesystem / Git / process
intent-engine → intent-edit / proof-* / cargo-allow application internals
proof-protocol / proof-provider-api → intent-model / intent-engine
proof-engine → intent-engine / cargo-allow private crates
provider adapters → intent-engine / cargo-allow private modules
shared substrate → any product-domain ontology crate
```

### Compatibility architecture

Legacy cargo-allow intent commands use **one-way process delegation** to an
installed cargo-intent (#2601). cargo-allow must not depend on intent or proof
libraries. Parity window exists; then embedded evaluator deletion (#2568).

### Publication posture

Shared and product-internal crates remain `publish = false` until #2604 records
a reviewed publish/package order and a published product requires the dependency.

### Repository extraction

Repository extraction is **not authorized**. Monorepo boundaries must remain
extractable through public process/protocol contracts and exact-candidate proof
after Issue #2558 dogfood, Issue #2559 extraction-readiness evidence, and Issue
#2605 exact-candidate interop all pass.

## Alternatives Considered

| Alternative | Tradeoff |
| --- | --- |
| Single product with feature flags | Hides independent support tiers and couples release stability |
| cargo-allow library embeds intent-engine | Violates independence; rejected |
| Universal `common` crate | Accumulates product-domain enums; rejected per #2612 |
| Immediate repo split | Boundaries unproven; deferred |
| Raw graph as public API | Couples consumers to internal IR; domain queries required (#2563) |

## Consequences

### Positive

- cargo-allow remains independently buildable, testable, and releasable.
- Intent compilation and proof orchestration can mature on separate support
  tiers.
- #2580 can machine-check dependency law against a ratified manifest.
- Extraction sequencing has an enforceable owner per module/type.

### Negative

- Transitional duplication until #2568 deletes embedded authority.
- Operators need multiple binaries for full intent-to-proof journey.
- More crates to coordinate in CI until #2604 product topology lands.

### Neutral Or Operational

- Historical `spec-system` names remain compatibility/provenance labels.
- Issue bodies remain research/provenance; retained artifacts are authority.

## Support-Tier Impact

Adds advisory experimental rows for cargo-intent and cargo-proof. cargo-allow
source-exception ledger tier unchanged.

## Policy Impact

Registers this ADR in `.allow/artifacts/doc-artifacts.toml`. Future #2580
architecture manifest must cite CARGO-ALLOW-ADR-0002 and #2612 topology.

## Required Evidence

- CARGO-ALLOW-SPEC-0010 atomic requirements
- #2580 ProductCrateArchitectureV1 (report-only first)
- #2606 parity/cutover receipts per stage
- #2607 shim registry bounds

## Non-Goals

- Creating crates or moving modules in this ADR
- Freezing exact crate count (owned by #2612)
- Authorizing physical repository extraction
- Defining every future command surface

## Claim Boundary

This ADR records durable ownership and dependency direction for three products
and their shared substrate inside one monorepo. It does not prove current code
complies, certify releases, or implement the architecture manifest.

## Rollback Or Supersession

Supersede by a new ADR that explicitly replaces CARGO-ALLOW-ADR-0002 and updates
the controlling issues for architecture manifest (#2580) and crate topology
(#2612) in one reviewed topology change. Do not land crate additions without
updating #2612, #2580, #2598, and #2604 together.
