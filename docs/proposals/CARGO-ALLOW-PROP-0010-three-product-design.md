---
id: CARGO-ALLOW-PROP-0010
kind: proposal
status: accepted
owner: repo-infra
created: 2026-07-22
linked_specs:
  - CARGO-ALLOW-SPEC-0010
linked_adrs:
  - CARGO-ALLOW-ADR-0002
supersedes_product_vision_from:
  - CARGO-ALLOW-PROP-0001
support_tier_impact: advisory
policy_impact:
  - .allow/artifacts/doc-artifacts.toml
  - docs/status/SUPPORT_TIERS.md
---

# Proposal: Three-Product Design Package

## Summary

The repository hosts three independently understandable products inside one
monorepo:

```text
cargo-allow   = source-exception ledger
cargo-intent  = durable authored intent and obligation compiler
cargo-proof   = exact-snapshot evidence orchestration
```

This proposal ratifies the product split decided in #2550, encodes the concrete
crate topology owned by #2612, and becomes the current authority for builders,
reviewers, tools, and future agents. It supersedes the product-vision portions
of [CARGO-ALLOW-PROP-0001](CARGO-ALLOW-PROP-0001-spec-system-profile.md) that
implied the spec-system lives as a permanent cargo-allow-owned product surface.
PROP-0001 remains authoritative for the opt-in `spec-system` governance profile
mechanics.

## Problem

cargo-allow's source-exception ledger, repository-intent compiler, and proof
orchestration currently share implementation namespaces, transitional modules,
and release posture. That coupling makes it easy to mistake:

- a source-exception finding for an intent obligation;
- an inferred candidate for an authored decision;
- a predictive grip receipt for execution proof;
- a convenience CLI wrapper for a durable authority.

Issue discussion (#2544, #2550, #2612, and descendants) now settles the product
boundaries, but the retained repository artifacts still describe a single
cargo-allow-owned spec-system. A fresh builder should not need issue archaeology
to discover product ownership, dependency law, extraction sequencing, or claim
boundaries.

## Users And Surfaces

- Maintainers: need one retained package that names three products, their crate
  families, and what each product may not own.
- Reviewers: need explicit claim boundaries before crate moves land.
- Agent operators: need repository-native authority that survives chat loss.
- Product surfaces: `cargo-allow`, future `cargo-intent`, future `cargo-proof`.
- Repo surfaces: proposal, ADR, spec, plan, support tiers, artifact ledger, and
  the fresh-agent reconstruction fixture under `tests/fixtures/three-product-design/`.

## User Value

The three-product split keeps each product independently releasable while
preserving one documented operator journey:

```text
cargo intent change status --staged --phase precommit
cargo proof plan --phase precommit
cargo proof run --phase precommit
```

Operators gain durable intent compilation and exact-snapshot proof without
coupling the source-exception ledger to experimental intent or proof crates.

## Proposed Shape

### Product definitions

| Product | Definition | Primary authority |
| --- | --- | --- |
| cargo-allow | Direct source-tree exception ledger | `policy/allow.toml`, scanner identity, matching, lifecycle, mutation receipts |
| cargo-intent | Durable authored intent and obligation compiler | proposals, specs, ADRs, initiatives, requirements, change contracts, slices, phase obligations |
| cargo-proof | Exact-snapshot evidence orchestration | proof plans, provider execution, receipt validation, currentness, contradictions, phase gates |

### Crate topology

Crate names and crate count are owned exclusively by #2612. This proposal
references that topology; it does not reopen names or counts.

```text
cargo-allow — existing ten crates, narrowed to source exceptions
shared — repo-protocol, repo-snapshot, repo-edit, rust-source-index
cargo-intent — intent-model, intent-protocol, intent-engine, intent-edit, cargo-intent
cargo-proof — proof-protocol, proof-provider-api, proof-engine,
              proof-adapter-command, proof-adapter-cargo-allow,
              proof-adapter-ripr, proof-adapter-hawk, cargo-proof
```

### Sequencing corrections

These decisions override any earlier implicit ordering:

| Decision | Rule |
| --- | --- |
| Rust subject extraction | Move `rust-source-index` before full `intent-engine` migration |
| Repository editing | Defer `repo-edit` until after read-only cargo-intent vertical and compatibility cutover (#2601) |
| Shared-crate publication | `publish = false` until a published product must depend on it; #2604 must add to publish/package order first |
| Intent migration compatibility | Never introduce `cargo-allow → intent` library dependency; parity window then one-way process delegation |

### Compatibility

Legacy `cargo-allow spec ...` commands delegate one-way to an installed
`cargo-intent` process (#2601). There is no retained cargo-allow evaluator and
no library dependency on intent-engine.

### Repository extraction

Physical repository extraction is **not authorized** by this package. The
monorepo remains the implementation home until Issue #2558 dogfood, Issue #2605
exact-candidate interop, and Issue #2559 extraction-readiness evidence all pass.
Those gates are necessary but do not replace a later explicit
repository-extraction authorization.

## Authority Disposition Map

Explicit classification of existing authority. Do not treat this table as a
second ledger; the artifact registry remains
`.allow/artifacts/doc-artifacts.toml`.

| Artifact / issue | Disposition | Notes |
| --- | --- | --- |
| CARGO-ALLOW-PROP-0001 | CurrentSupporting | Profile mechanics, artifact graph, worklist/doctor remain valid |
| CARGO-ALLOW-PROP-0001 product vision | SupersededWithReplacement | Product ownership replaced by CARGO-ALLOW-PROP-0010 |
| CARGO-ALLOW-SPEC-0001 | CurrentSupporting | Structural profile contract; product commands migrate to cargo-intent |
| CARGO-ALLOW-SPEC-0009 | CurrentSupporting | Runtime-promotion invariant remains bounded self-hosted control-plane law |
| CARGO-ALLOW-ADR-0001 | CurrentSupporting | Federation precedence remains valid for ledger composition |
| plans/spec-system/implementation-plan.md | HistoricalOnly | Pre-three-product sequencing; see plans/three-product-crate-extraction.md |
| allow-policy::spec_system | GeneratedOrDerived | Transitional source location for intent semantics; not cargo-allow ownership |
| cargo-allow spec_system* modules | GeneratedOrDerived | Transitional; delete or delegate after #2601/#2568 parity |
| #2550 | CurrentCanonical | Three-product authority law (encoded here) |
| #2612 | CurrentCanonical | Crate names, counts, stage gates (referenced, not duplicated) |
| #2598 | CurrentSupporting | Move/deletion ledger implementation owner; not started in this PR |
| Chat / issue comments | HistoricalOnly | Provenance only after this package lands |

## Alternatives Considered

| Alternative | Reason not chosen |
| --- | --- |
| Keep spec-system inside cargo-allow permanently | Couples ledger release stability to experimental intent/proof schemas |
| One universal `common` crate | Becomes a second ontology and violates #2612 non-crate list |
| Immediate repository split | Public/process boundaries unproven; extraction deferred per #2559 |
| Library embedding for compatibility | Violates one-way process delegation and cargo-allow independence law |
| One release/version for all products | Independent support tiers and semver required per product |

## Success Criteria

- One retained proposal, ADR, spec, and plan define three products without
  duplicate authority.
- Disposition map classifies prior artifacts without silently rewriting history.
- Sequencing corrections are explicit and match #2612 stage gates.
- Fresh-agent reconstruction fixture answers the #2544 review questions from
  repository sources alone.
- `cargo-allow check --profile spec-system --mode audit` validates the new
  artifact graph.

## Specs To Create

- [CARGO-ALLOW-SPEC-0010](../specs/CARGO-ALLOW-SPEC-0010-three-product-boundaries.md)

## Support-Tier Impact

Adds advisory rows for cargo-intent and cargo-proof experimental posture.
Updates spec-system row to note three-product authority without claiming shipped
intent/proof products.

## Policy Impact

- `.allow/artifacts/doc-artifacts.toml` — register new proposal, ADR, spec, plan.
- `docs/status/SUPPORT_TIERS.md` — product claim boundaries.

## Required Evidence

- `cargo test -p allow-policy spec_system_design_package`
- `cargo test -p cargo-allow spec_design_artifact_links`
- `cargo run -p cargo-allow -- check --profile spec-system --mode audit`

## Non-Goals

- Moving Rust modules or creating crates (#2598 and later issues).
- Beginning #2598 move-ledger implementation on this branch.
- Physical repository extraction.
- Making cargo-intent or cargo-proof prerequisites for cargo-allow.
- Preserving duplicate embedded spec-system evaluators.

## Claim Boundary

This proposal records the three-product architecture, disposition of prior
authority, sequencing corrections, and monorepo extraction posture. It does
not move code, prove semantic parity, certify independent product releases, or
authorize repository extraction.

## Risks

| Risk | Mitigation |
| --- | --- |
| Silent dual authority during migration | #2598 move ledger, #2606 parity receipts, #2607 shim bounds |
| Convenience CLI erases product claims | Separate binaries, support tiers, and one-way delegation only |
| Shared protocols become ontology dump | #2612 forbidden edges and small envelope-only repo-protocol |

## Rollback Or Withdrawal

Withdraw by superseding CARGO-ALLOW-PROP-0010, removing linked ADR/spec/plan
ledger entries, and restoring PROP-0001 as the sole product-vision artifact.
Default cargo-allow source-exception behavior is unaffected.
