---
id: CARGO-ALLOW-PROP-0010
kind: proposal
status: accepted
owner: repo-infra
created: 2026-07-22
updated: 2026-08-12
linked_specs:
  - CARGO-ALLOW-SPEC-0010
  - CARGO-ALLOW-SPEC-0011
linked_adrs:
  - CARGO-ALLOW-ADR-0002
  - CARGO-ALLOW-ADR-0003
supersedes_product_vision_from:
  - CARGO-ALLOW-PROP-0001
support_tier_impact: advisory
policy_impact:
  - .allow/artifacts/doc-artifacts.toml
  - docs/status/SUPPORT_TIERS.md
---

# Proposal: Three-Product Design Package

## Summary

The repository contains three independently understandable products inside one
monorepo:

```text
cargo-allow   = source-exception ledger
cargo-intent  = durable authored intent and obligation compiler
cargo-proof   = exact-snapshot evidence orchestration
```

The maximum extraction scaffold has converged to the retained package topology,
and final shared package identities and versions are applied. Remaining work is
semantic and release convergence: select one semantic owner for every concept,
remove reverse dependencies and duplicate evaluators, and qualify each product
from its own exact package closure.

This proposal distinguishes three facts that earlier retained authority blurred:

```text
HistoricalObservedTopologyV1
  the maximum 27-package scaffold that formerly existed

ExtractionScaffoldHistory
  the maximum package graph used to expose and test candidate seams

CurrentTopologyV2
  the 22 retained package boundaries selected on current source
```

Current existence is not permanent package ratification.

## Product authorities

| Product | Owns | Does not own |
| --- | --- | --- |
| cargo-allow | source selection and sensor capability, source-exception findings, `policy/allow.toml`, matching, lifecycle, no-new/diff posture, ledger mutation, cargo-allow reports and process-provider projection | durable repository intent, the private intent graph, proof execution, cross-provider currentness or phase gates |
| cargo-intent | authored proposals/specs/ADRs/requirements/transactions, source-role and dialect resolution, the private compiled intent graph, phase obligations, bounded domain queries and later semantic edit settlement | source-exception ledger policy, proof-provider execution or final merge policy |
| cargo-proof | proof planning, registered provider execution or captured-result ingestion, exact snapshot/config/tool currentness, receipt composition, contradictions and phase-gate evidence | authored product direction, cargo-allow matching semantics or provider-private analyzer meaning |

The products may participate in one operator journey without becoming one
semantic authority, one support tier or one release unit.

## Current implementation state

| Surface | Current status | Exact boundary |
| --- | --- | --- |
| cargo-allow published channel | `SupportedPublished` | `0.1.11`; registry and documentation evidence apply only to that release |
| cargo-allow source candidate | `SupportedSourceCandidate` | workspace source is versioned `0.2.0`; no tag or publication is authorized |
| cargo-intent | `LandedExperimental` | identity and staged-precommit change-status vertical exist; canonical graph/compiler cutover is incomplete |
| cargo-proof | `LandedExperimental` | protocol, planning, dry-run, provider contracts and captured-report adapters exist; real product composition remains incomplete |
| shared substrate | `LandedExperimental` | all four `effortless-*` crates and independent versions exist; dependency neutrality remains incomplete |
| legacy intent compatibility | `CompatibilityOnly` / `BlockedOnParity` | one bounded process route exists; embedded current semantic authority remains to be removed |
| physical repository extraction | `NotStarted` | not authorized by crate existence, package smoke or integrated dogfood |

A crate, binary, fixture or local package smoke does not by itself promote
support, complete an authority move or justify a permanent package boundary.

## Current and historical package topology

The selected current source contains 22 Cargo packages:

```text
cargo-allow family   10
shared substrate      4
cargo-intent family   5
cargo-proof family    3
```

The historical maximum extraction scaffold contained 27 packages, including
eight proof packages. The five extra proof boundaries were absorbed into the
retained three-package proof family:

```text
historical total     27
current total        22
```

The completed package-to-module absorptions are:

```text
proof-provider-api
  → proof_engine::provider_api

proof-adapter-command
  → proof_engine::command_adapter

proof-adapter-cargo-allow
  → cargo_proof::providers::cargo_allow

proof-adapter-ripr
  → cargo_proof::providers::ripr

proof-adapter-hawk
  → cargo_proof::providers::hawk
```

Their provider, process and conformance semantics survive. Their independent
Cargo package identities do not survive unless later evidence establishes an
external consumer, independent compatibility contract, materially different
toolchain/dependency boundary or measured build/distribution benefit.

## Retained package families

```text
cargo-allow
  allow-core, allow-policy, allow-inventory, allow-files, allow-rust,
  allow-match, allow-report, allow-diff, allow-policy-legacy, cargo-allow

shared substrate
  repo-protocol, repo-snapshot, repo-edit, rust-source-index

cargo-intent
  intent-model, intent-protocol, intent-engine, intent-edit, cargo-intent

cargo-proof
  proof-protocol, proof-engine, cargo-proof
```

`policy/product-crates.toml`, its generation-2 successor and the checked package
topology own the machine-readable current and target rows. This summary is a
human projection, not another package manifest.

## Semantic ownership inside the target

### Generation-2 repository governance

Repository-wide architecture/package/move/shim/parity authority is intent, not
cargo-allow policy:

```text
intent-model
  pure authored identity, package, move, shim, parity and cutover DTOs

intent-engine
  exact current/target reconciliation, Cargo-closure validation,
  transition expiry and deletion eligibility

cargo-intent / repository CI
  validation operation and deterministic typed receipt

allow-policy
  policy/allow.toml source-exception semantics
  historical/current compatibility adapters only during migration
```

Cargo-allow release workflows may consume a bounded typed validation receipt.
Cargo-allow packages do not depend on intent libraries.

### Intent model, protocol and engine

```text
intent-model
  durable authored contracts and local validation

intent-protocol
  bounded query, view, obligation, diagnostic, edit-plan and settlement DTOs

intent-engine
  private graph, source/authority compilation, graph comparison,
  phase policy and bounded query implementation

intent-edit
  explicit semantic authoring, repo-edit translation, recompile and settlement
```

The graph is disposable private IR. Public consumers do not traverse it.

### Proof protocol, engine and application

```text
proof-protocol
  wire DTOs, canonical serialization and local structural validation

proof-engine
  provider-neutral planning, execution policy, currentness/cache reuse,
  capability satisfaction, contradiction, aggregation and phase-gate semantics

cargo-proof
  CLI/application plus deterministic feature-gated built-in provider registry
```

Provider-specific payloads remain namespaced. Process success is never sufficient
to claim an obligation is satisfied.

## Shared package identities and physical paths

Logical ID, workspace path, dependency alias, Cargo package and Rust library are
separate checked identities. The approved migration is:

| Logical ID | Historical path | Current path | Cargo package | Rust library |
| --- | --- | --- | --- | --- |
| `repo-protocol` | `crates/repo-protocol` | `crates/effortless-repo-protocol` | `effortless-repo-protocol` | `effortless_repo_protocol` |
| `repo-snapshot` | `crates/repo-snapshot` | `crates/effortless-repo-snapshot` | `effortless-repo-snapshot` | `effortless_repo_snapshot` |
| `repo-edit` | `crates/repo-edit` | `crates/effortless-repo-edit` | `effortless-repo-edit` | `effortless_repo_edit` |
| `rust-source-index` | `crates/rust-source-index` | `crates/effortless-rust-source-index` | `effortless-rust-source-index` | `effortless_rust_source_index` |

`logical_id` is stable. `workspace_path` and Rust library name are exact current
physical/import identities and may change only through a reviewed migration
generation.

## Version and publication posture

Before first publication of newly extracted packages:

```text
cargo-allow family      0.2.0 release train
shared substrate        explicit 0.1.0 experimental/transitive lines
cargo-intent family     explicit 0.1.0 experimental lines
cargo-proof family      explicit 0.1.0 experimental lines
```

Equal experimental versions are a development cohort, not a lockstep
compatibility promise. The five absorbed proof packages received no final
version, publication row, or compatibility placeholder.

Registry visibility, direct-library support, product support and physical
repository extraction remain separate decisions. The external `effortless`
suite bootstrap is a functional installer/updater/doctor and does not enter a
product dependency closure.

## Dependency direction

In the following, `A → B` means **A depends on B**:

```text
shared substrate
  → no allow, intent or proof product-domain crate

intent-engine
  → intent-model + intent-protocol + neutral snapshot/index substrate
  → no intent-edit, proof engine or cargo-allow private implementation

intent-edit
  → intent-engine + intent-protocol + repo-edit

proof-engine
  → proof-protocol + intent-protocol + neutral repository substrate
  → no intent-engine, cargo-proof provider module or cargo-allow private crate

cargo-proof provider modules
  → proof-engine public provider/execution surface
  → exact public/process contract of the selected provider

cargo-allow compatibility
  → installed cargo-intent process protocol
  → no intent/proof library dependency and no semantic fallback
```

The compatibility path is one-way process delegation from cargo-allow to the
installed cargo-intent product; no reverse library dependency or embedded
semantic fallback is part of the target.

Temporary reverse edges require an exact move/shim row, parity case, expiry and
deletion condition. They are visible non-final state, not evidence that the
architecture is clean.

## Convergence sequence

```text
A  rebuild retained generation-2 authority from current main
B  integrate parsed reconstruction and compatibility contracts
C  select strict generation-2 identities and current/target Cargo closures
D  converge one canonical intent model/compiler/evaluator and remove fallback
E  make selected shared packages product-neutral
F  converge proof semantics and providers into the retained three-package family
G  completed via #2937/#2938: absorb five obsolete proof package boundaries
H  completed via #2885: move/rename shared packages and split survivor version lines
I  qualify the exact cargo-allow package/install/journey candidate
J  close mutation, scanner, release-evidence, provenance and registry trust
K  exact refreeze, explicit maintainer authorization and publication
```

Every dependent writer starts from merged `main`. A lane continues through
implementation, focused proof, review, valid repairs, complete hosted CI, thread
resolution, merge and merged-main verification. Opening a PR, reaching “ready
for review” or obtaining one green job is not completion.

## Release boundaries

### cargo-allow 0.2.x

The architecture gate requires truthful V2 authority, one canonical intent
evaluator with no embedded fallback, current topology fixed at 22, a clean
selected shared closure, final survivor identities, and the exact cargo-allow
candidate/release-trust train.

Full cargo-proof product qualification is not a cargo-allow release prerequisite
unless the selected cargo-allow support matrix explicitly includes that
integrated claim.

### cargo-intent 0.1.x

Cargo-intent remains experimental until its own selected compiler/query/product
surface, independent package candidate and support evidence are qualified.
Cargo-allow requires only the minimum canonical intent cutover and installed
compatibility route needed to eliminate duplicate current authority.

### cargo-proof 0.1.x

Package-topology convergence removes obsolete package identities before
cargo-allow 0.2. The full isolated three-package cargo-proof candidate, provider
feature matrix and support/publication evidence remain an independent later gate.

## Repository extraction

For the strict generation-1 compatibility contract and the current retained
position alike, repository extraction is **not authorized**.

Physical repository extraction becomes eligible only after independent
package/CI/support closures, public-boundary external dogfood, shim and
private-path deletion, simplification review and a later explicit authorization
naming the repositories and exact source state to move.

## Machine-source law

| Concern | Canonical source |
| --- | --- |
| retained semantic decision | this proposal, ADR-0002, ADR-0003 and SPEC-0011 |
| current logical topology and roles | `policy/product-crates-v2.toml` under #2923/#3391 |
| exact Cargo closure and candidate posture | `policy/product-package-topology-v2.toml` under #2923/#3391 |
| selected current enforcement and typed receipt | #2923 via cargo-intent/repository CI |
| current source disposition and deletion output | `policy/product-move-ledger.toml` / #2598 |
| temporary compatibility | `policy/extraction-shims.toml` / #2607 |
| parity and cutover evidence | `policy/extraction-parity.toml` / #2606 |
| historical generation-1 package posture | `policy/product-package-topology.toml` / #2604 |
| release authorization | #2371, #2501 and #2502 |

Human diagrams and tables are generated or contract-checked projections. A new
package, package-to-module collapse, path move, package rename, version-source
change or authority move updates every affected machine authority in one
reviewed migration.

## Success criteria

- A fresh builder can distinguish the historical 27-package scaffold from the
  current retained 22 and identify every completed absorption destination.
- One concept has one semantic owner.
- Shared substrate has no production dependency on product ontology.
- One intent compiler and phase evaluator remain after cutover.
- Proof protocol is data-oriented and proof engine is the sole evidence-semantic
  evaluator.
- Cargo-allow packages and runs with sibling product source trees absent.
- Each product is qualified from its exact selected package bytes rather than
  the ambient workspace.
- Repository extraction, if later chosen, requires no semantic redesign.

## Non-goals

- Changing package names, paths, versions or Cargo.lock in this authority PR.
- Moving semantic implementation or deleting packages in this authority PR.
- Promoting cargo-intent or cargo-proof because their scaffolds exist.
- Exposing a public raw intent graph.
- Tagging, publishing or authorizing physical repository extraction.

## Claim boundary

This proposal records the current three-product state, observed and target
package topology, final semantic owners, package-identity direction, independent
release boundaries and convergence order. It does not prove current code
compliance, perform a migration, qualify a product candidate, authorize
cargo-allow 0.2 or authorize repository extraction.
