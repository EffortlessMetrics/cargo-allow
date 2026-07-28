---
id: CARGO-ALLOW-ADR-0002
kind: adr
status: accepted
owner: repo-infra
created: 2026-07-22
updated: 2026-07-28
linked_proposal: CARGO-ALLOW-PROP-0010
linked_specs:
  - CARGO-ALLOW-SPEC-0010
  - CARGO-ALLOW-SPEC-0011
related_adrs:
  - CARGO-ALLOW-ADR-0003
supersedes: none
superseded_by: none
support_tier_impact: advisory
policy_impact:
  - .allow/artifacts/doc-artifacts.toml
---

# ADR: Three-Product Ownership and Dependency Direction

## Context

Cargo-allow began with source-exception policy, repository-intent compilation,
and proof-oriented governance in one implementation tree. Issue #2550 settled
three product authorities inside one monorepo, and Issue #2612 ratified the
logical crate graph.

The full logical crate topology now exists. The remaining failure mode is no
longer missing crate names; it is accidental authority retention:

- a transitional source location can be mistaken for permanent product
  ownership;
- a shared crate can import product ontology and cease to be shared;
- a compatibility facade can retain a second evaluator;
- a provider adapter can import orchestrator or application internals;
- a walking-skeleton binary can be treated as a supported product;
- one workspace can be treated as one release graph.

This ADR owns semantic product boundaries and dependency direction. It does not
own crates.io naming or version policy; CARGO-ALLOW-ADR-0003 owns those identities.

## Decision

Adopt three semantic product owners with a small neutral substrate.

### cargo-allow

Owns:

```text
source inventory and sensor capability
source-syntax and source-presence findings
policy/allow.toml parsing and validation
finding-to-receipt matching, ambiguity and occurrence law
lifecycle, evidence and no-new posture
cargo-allow finding/policy/ledger movement
source-exception mutation plans and receipts
cargo-allow reports, diagnostics and process-provider payloads
cargo-allow CLI and release identity
```

Does not own durable repository intent, the private intent graph, proof-provider
execution, cross-provider currentness, or phase-gate evidence composition.

### cargo-intent

Owns:

```text
authored proposals, specifications, ADRs, initiatives and requirements
authority, source-role and dialect resolution
implementation slices, transactions and semantic effects
private compiled intent graph and indexes
parent/candidate graph comparison and impact closure
phase-aware obligations, inference and posture reconciliation
bounded read-only domain queries and diagnostics
semantic edit plans, approval, recompile and settlement through intent-edit
cargo-intent CLI, rendering and product identity
```

The compiled graph is disposable private IR. Public consumers receive bounded
`intent-protocol` values rather than graph nodes.

### cargo-proof

Owns:

```text
proof-reference and provider identity
obligation-to-proof planning and provider selection
explicit registered provider execution or captured-result ingestion
receipt validation and exact snapshot/config/tool currentness
cache and stale-result handling
cross-provider contradiction composition
phase-gate evidence composition
cargo-proof CLI, rendering and product identity
```

Cargo-proof does not decide authored product direction, cargo-allow matching
semantics, provider-private domain meaning, or final merge policy.

## Shared substrate

```text
repo-protocol
  provider-neutral repository, tool, capability, result, completeness,
  currentness, diagnostic, action and receipt transport only

repo-snapshot
  exact committed tree, staged index, saved worktree and overlay source views;
  source identity, bounded reads, limitations and staleness

repo-edit
  repository-owned target identity, containment, lock sets, generic apply,
  atomicity/rollback classification and filesystem apply receipts

rust-source-index
  structural Rust package, target, module, item and test-subject inventory;
  stable source selectors and explicit parser/cfg limitations
```

A shared crate exists because at least two products need one load-bearing
implementation. “Shared” does not imply stable direct API, publication, or
permission to contain product ontology.

## Dependency direction

In this section, `A → B` means **A depends on B**.

```text
repo-snapshot → repo-protocol
repo-edit → repo-protocol and neutral snapshot identities where required
rust-source-index → repo-protocol and repo-snapshot

intent-model → repo-protocol
intent-protocol → intent-model and repo-protocol
intent-engine → intent-model, intent-protocol, repo-protocol,
                repo-snapshot and rust-source-index
intent-edit → intent-engine, intent-model, intent-protocol,
              repo-protocol, repo-snapshot and repo-edit
cargo-intent → intent-engine, intent-protocol and optional intent-edit

proof-protocol → repo-protocol
proof-provider-api → proof-protocol and repo-protocol
proof-engine → proof-protocol, proof-provider-api, repo-protocol,
               repo-snapshot and intent-protocol
proof-adapter-* → proof-provider-api, proof-protocol, repo-protocol and
                  exact provider public/process contracts
cargo-proof → proof-engine and selected proof-adapter-* crates

cargo-allow implementation crates → neutral shared substrate only where the
                                     moved implementation requires it
```

## Forbidden edges

```text
cargo-allow product → intent-model, intent-engine, proof-engine or cargo-proof
shared substrate → allow, intent or proof product-domain crates
intent-model or intent-protocol → filesystem, Git, process or repo-snapshot I/O
intent-engine → intent-edit, proof-* or cargo-allow application internals
proof-protocol or proof-provider-api → intent-model or intent-engine
proof-engine → intent-engine or cargo-allow private crates
provider adapters → intent-engine, proof-engine internals or cargo-allow private modules
```

A temporary non-final edge is allowed only through an exact move/shim record
with activating feature/target, owner, reason, parity case, latest stage or
release, and deletion condition. An undocumented or expired transition fails
architecture validation.

## Read-only and mutation boundary

```text
intent-engine
  compile, compare, query, infer and produce safe action/edit plans
  never apply filesystem edits

intent-edit
  validate semantic plan and approval
  translate accepted file edits into repo-edit requests
  apply through repo-edit
  recompile through intent-engine
  compare expected and observed movement
  emit semantic settlement

repo-edit
  prove only the bounded filesystem operation attempted and observed
```

A successful repository write is not automatically a successful semantic
settlement.

## Compatibility boundary

Legacy cargo-allow intent operations use one-way process delegation to an
installed `cargo-intent` through provider-neutral envelopes. Cargo-allow:

- validates the selected executable and protocol identity;
- binds the request and response to an exact source subject;
- drains bounded subprocess output safely;
- renders a compatibility projection;
- returns explicit unavailable, incompatible, stale, malformed, timeout and
  instrument-failure results; and
- never falls back to an embedded semantic evaluator after cutover.

Historical generations may remain readable but cannot become current authority.

## Current implementation appendix

| Boundary | Status on 2026-07-28 |
| --- | --- |
| Logical 27-crate topology | landed |
| cargo-allow core | published `0.1.11`; source candidate `0.2.0` |
| cargo-intent | landed experimental staged-precommit vertical |
| cargo-proof | landed experimental planning/dry-run/provider-contract skeleton |
| shared substrate | landed transitional; package/version/dependency convergence incomplete |
| process compatibility | first staged operation landed; hardening and broader cutover incomplete |
| embedded current intent deletion | incomplete |
| independent exact package qualification | incomplete |
| physical repository extraction | not authorized |

This appendix reports implementation state; it does not change ownership law.

## Publication and versioning

CARGO-ALLOW-ADR-0003 owns:

- logical/package/library identity separation;
- `effortless-*` shared package names;
- cargo-allow `0.2.x` versus experimental shared/intent/proof `0.1.x` lines;
- transitive registry visibility versus supported direct use; and
- atomic pre-publication identity migration.

This ADR does not infer support or compatibility from equal workspace membership.

## Repository extraction

Physical repository extraction is not authorized. It may be considered only
after independent package and CI closures, public-boundary external dogfood,
shim/private-path deletion, simplification review, and a later explicit
repository-extraction authorization.

## Consequences

### Positive

- cargo-allow remains independently buildable, installable and releasable.
- Intent ontology and proof-provider integrations can evolve on separate support
  and version lines.
- Read-only consumers do not inherit mutation authority.
- Provider-specific dependencies do not contaminate the proof engine.
- A later repository split is operational rather than another semantic redesign.

### Negative

- The monorepo must maintain explicit package and compatibility contracts during
  convergence.
- Temporary duplication is visible until parity and deletion complete.
- Operators selecting the full journey need multiple binaries.

### Operational

- Every move has one writer and one deletion denominator.
- Dependent branches start from merged `main`.
- Exact product package closures, not the ambient workspace, qualify releases.

## Rejected alternatives

| Alternative | Why rejected |
| --- | --- |
| Permanent spec-system inside cargo-allow | Couples the stable ledger to evolving intent and proof semantics. |
| One universal `common` crate | Becomes a second ontology and hides ownership. |
| Public raw intent graph | Freezes private storage shape and encourages incidental traversal contracts. |
| Mutation inside intent-engine | Gives every read-only consumer filesystem write authority. |
| Provider adapters inside proof-engine | Pulls provider toolchains and private semantics into the core orchestrator. |
| Immediate repository split | Makes atomic parity and deletion harder before boundaries are proven. |

## Required evidence

- generation-2 architecture and package identity validation;
- selected dependency closure with direct and transitive paths;
- parity/cutover receipts for moved semantic operations;
- absence of embedded fallback after cutover;
- independent installed-candidate proof for each selected product closure.

## Claim boundary

This ADR records semantic ownership, dependency direction, side-effect
boundaries, compatibility law, and current implementation posture. It does not
prove current code compliance, define package names or versions beyond its link
to ADR-0003, publish products, authorize cargo-allow 0.2, or authorize physical
repository extraction.
