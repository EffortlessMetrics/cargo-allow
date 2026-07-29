---
id: CARGO-ALLOW-ADR-0002
kind: adr
status: accepted
owner: repo-infra
created: 2026-07-22
updated: 2026-07-29
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

Cargo-allow began with source-exception policy, repository-intent compilation
and proof-oriented governance in one implementation tree. The monorepo now
contains all 27 packages from the maximum extraction scaffold, but source
location and package existence do not establish permanent semantic ownership.

The failure modes to prevent are:

- a transitional source location becoming permanent product authority;
- a shared crate importing product ontology;
- a compatibility facade retaining a second evaluator;
- an application/provider package leaking into a read-only engine;
- a walking-skeleton binary being treated as supported;
- one workspace being treated as one package or release graph; and
- extraction-only packages becoming permanent compatibility units without an
  external consumer or independent dependency/toolchain need.

This ADR owns semantic product boundaries, side-effect boundaries and dependency
direction. ADR-0003 owns package, path, version, publication and support identity.

## Decision

Adopt three semantic products with a small neutral substrate and a 22-package
retained target.

## Product ownership

### cargo-allow

Owns:

```text
source inventory and sensor capability
source-syntax and source-presence findings
policy/allow.toml parsing, validation and federation
finding-to-entry matching, ambiguity and occurrence law
lifecycle, evidence and no-new posture
cargo-allow finding/policy/ledger movement
source-exception mutation plans and receipts
cargo-allow reports, diagnostics and process-provider payloads
cargo-allow CLI/application product identity
```

Cargo-allow does not own durable repository intent, the private intent graph,
proof-provider execution, cross-provider currentness or phase-gate composition.

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

The graph is disposable private IR. Public consumers receive bounded
`intent-protocol` values rather than graph nodes.

Repository-wide generation-2 governance is intent authority:

```text
intent-model
  authored architecture/package/move/shim/parity/cutover DTOs

intent-engine
  current/target reconciliation, exact Cargo-closure validation,
  transition expiry and deletion eligibility

cargo-intent / repository CI
  validation entry point and deterministic typed receipt
```

`allow-policy` retains source-exception ledger semantics and only temporary
historical/current adapters while the governance move completes.

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
semantics, provider-private analyzer meaning or final merge policy.

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
implementation. “Shared” does not imply stable direct API, publication or
permission to contain product ontology.

## Retained target packages

```text
cargo-allow family
  allow-core, allow-policy, allow-inventory, allow-files, allow-rust,
  allow-match, allow-report, allow-diff, allow-policy-legacy, cargo-allow

shared substrate
  repo-protocol, repo-snapshot, repo-edit, rust-source-index

cargo-intent family
  intent-model, intent-protocol, intent-engine, intent-edit, cargo-intent

cargo-proof family
  proof-protocol, proof-engine, cargo-proof
```

Five proof packages in the observed 27-package scaffold collapse into modules:

```text
proof-provider-api        → proof_engine::provider
proof-adapter-command     → cargo_proof::providers::command
proof-adapter-cargo-allow → cargo_proof::providers::cargo_allow
proof-adapter-ripr        → cargo_proof::providers::ripr
proof-adapter-hawk        → cargo_proof::providers::hawk
```

Provider semantics remain independently modular and feature-selectable. Package
re-extraction later requires evidence of an external consumer, independent
compatibility, materially different toolchain/dependencies or measured build and
distribution benefit.

## Dependency direction

In this section, `A → B` means **A depends on B**.

```text
repo-snapshot → repo-protocol
repo-edit → repo-protocol and neutral snapshot identities where required
rust-source-index → repo-protocol and repo-snapshot

intent-model → neutral identity/protocol primitives only
intent-protocol → intent-model and repo-protocol
intent-engine → intent-model, intent-protocol, repo-snapshot,
                rust-source-index and repo-protocol
intent-edit → intent-engine, intent-model, intent-protocol,
              repo-snapshot, repo-edit and repo-protocol
cargo-intent → intent-engine, intent-protocol and optional intent-edit

proof-protocol → repo-protocol
proof-engine → proof-protocol, intent-protocol, repo-protocol
               and repo-snapshot
cargo-proof → proof-engine, proof-protocol and selected built-in provider modules

cargo-allow implementation crates → neutral shared substrate only where the
                                     moved implementation requires it
```

## Forbidden edges

```text
cargo-allow product → intent-model, intent-engine, proof-engine or cargo-proof
shared substrate → allow, intent or proof product-domain crates
intent-model or intent-protocol → filesystem, Git, process or repo-snapshot I/O
intent-engine → intent-edit, proof-* or cargo-allow application internals
proof-protocol → intent-model, intent-engine, cache, registry or phase policy
proof-engine → intent-engine, cargo-proof provider modules or cargo-allow private crates
cargo-proof provider modules → intent-engine, proof-engine internals or provider-private implementation crates
```

A temporary non-final edge is allowed only through an exact move/shim record
with activating feature/target, owner, reason, parity case, latest stage or
release and deletion condition. An undocumented or expired transition fails.

## Read-only and mutation boundary

```text
intent-engine
  compile, compare, query and infer
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

## Proof data and semantic boundary

```text
proof-protocol
  versioned DTOs, canonical serialization and local structural validation

proof-engine
  semantic currentness, cache reuse, capability satisfaction,
  contradiction, aggregation and phase-gate evaluation

cargo-proof provider modules
  provider-specific preparation and namespaced interpretation
```

A raw process exit, valid JSON envelope or provider receipt cannot establish
phase satisfaction inside `proof-protocol`.

## Compatibility boundary

Legacy cargo-allow intent operations use one-way process delegation to an
installed `cargo-intent` through provider-neutral envelopes. Cargo-allow:

- validates executable, product, protocol, request and source identity;
- drains bounded subprocess output safely;
- renders a compatibility projection;
- returns explicit unavailable, incompatible, stale, malformed, over-budget,
  timeout and instrument-failure results; and
- never falls back to an embedded semantic evaluator after cutover.

Historical generations may remain readable but cannot become current authority.

## Current implementation appendix

| Boundary | Status on 2026-07-29 |
| --- | --- |
| observed 27-package extraction scaffold | landed |
| retained 22-package target | accepted; package collapse not yet applied |
| cargo-allow core | published `0.1.11`; source candidate `0.2.0` |
| cargo-intent | landed experimental staged-precommit vertical; canonical cutover incomplete |
| cargo-proof | landed experimental scaffold; real composition incomplete |
| shared substrate | landed transitional; package/version/dependency convergence incomplete |
| bounded process transport | landed through #2901 |
| embedded current intent deletion | incomplete |
| independent exact product candidates | incomplete |
| physical repository extraction | not authorized |

This appendix reports implementation state; it does not change ownership law.

## Consequences

### Positive

- cargo-allow remains independently buildable, installable and releasable.
- Intent ontology and proof providers can evolve on separate support/version lines.
- Read-only consumers do not inherit mutation authority.
- Provider-specific semantics do not contaminate the proof engine.
- Extraction scaffolding can be removed before first publication.
- A later repository split is operational rather than another semantic redesign.

### Negative

- The monorepo must maintain explicit current and target package identities during
  convergence.
- Temporary duplication remains visible until parity and deletion complete.
- Operators selecting the full journey need multiple binaries.

## Rejected alternatives

| Alternative | Why rejected |
| --- | --- |
| Permanent spec-system inside cargo-allow | Couples the stable ledger to evolving intent and proof semantics. |
| One universal `common` crate | Becomes a second ontology and hides ownership. |
| Public raw intent graph | Freezes private implementation shape. |
| Mutation inside intent-engine | Gives every read-only consumer write authority. |
| Permanent one-package-per-provider scaffold | Creates version and publication boundaries without independent consumers. |
| Provider modules inside proof-engine | Pulls application/provider dependencies into the provider-neutral engine. |
| Immediate repository split | Makes parity and atomic deletion harder before boundaries are proven. |

## Required evidence

- strict current/target identity and selected-closure validation;
- semantic parity and exact deletion receipts for moved operations;
- no shared-to-product reverse dependency in selected package closures;
- absence of embedded intent fallback;
- observed package denominator converged from 27 to 22;
- independent installed-candidate proof for selected product closures.

## Claim boundary

This ADR records semantic ownership, dependency direction, package-survival,
side-effect boundaries and compatibility law. It does not prove current code
compliance, apply the package collapse, publish products, authorize cargo-allow
0.2 or authorize physical repository extraction.
