---
id: CARGO-ALLOW-SPEC-0011
kind: spec
status: accepted
owner: repo-infra
created: 2026-07-29
updated: 2026-08-12
linked_proposal: CARGO-ALLOW-PROP-0010
linked_adr: CARGO-ALLOW-ADR-0002
supersedes_requirement_ids:
  - CARGO-ALLOW-SPEC-0010#crate-topology-owned-by-2612
  - CARGO-ALLOW-SPEC-0010#rust-source-index-before-intent-engine
  - CARGO-ALLOW-SPEC-0010#repo-edit-deferred
  - CARGO-ALLOW-SPEC-0010#shared-publish-false-until-2604
support_tier_impact: advisory
policy_impact:
  - policy/product-crates.toml
  - policy/product-package-topology.toml
  - policy/product-move-ledger.toml
  - policy/extraction-shims.toml
  - policy/extraction-parity.toml
---

# Spec: Three-Product Convergence and Release Boundaries

## Summary

This specification starts from the observed 27-package extraction scaffold and
ratifies a 22-package convergence target. It replaces generation-1 crate-creation
sequencing with current identity, semantic ownership, package survival, cutover,
deletion, exact-candidate and release requirements.

CARGO-ALLOW-SPEC-0010 remains the historical generation-1 boundary record.
Requirements not named in `supersedes_requirement_ids` remain supporting
historical authority where they do not conflict with this specification.

## Current implementation state

```text
cargo-allow
  published supported channel: 0.1.11
  source candidate: 0.2.0
  product authority: source-exception ledger

cargo-intent
  landed experimental read-only vertical
  selected staged-precommit operation exists
  canonical graph/compiler cutover: incomplete

cargo-proof
  landed experimental extraction scaffold
  target package family: proof-protocol + proof-engine + cargo-proof
  real product qualification: independent and incomplete

shared substrate
  all four effortless-* crates landed on independent 0.1.0 lines
  dependency neutrality: incomplete

workspace topology
  current packages: 22 (10 allow + 4 shared + 5 intent + 3 proof)
  retained packages: 22
  historical maximum extraction scaffold: 27
```

The existence of a crate, binary, fixture or package smoke does not promote
support, complete a move or ratify a permanent package boundary.

## Normative requirements

```toml cargo-allow-requirements
schema_version = "1.0"

[[requirement]]
id = "observed-and-target-topologies-distinct"
generation = 1
status = "accepted"
statement = "Historical observed packages and the retained topology are represented independently; after the five package-to-module absorptions, the selected current source and ratified retained topology each contain 22 packages: 10 cargo-allow, 4 shared, 5 cargo-intent, and 3 cargo-proof."
claim_class = "runtime_behavior"

[[requirement]]
id = "identity-distinguishes-logical-path-alias-package-lib"
generation = 1
status = "accepted"
statement = "Every component has independently checked logical ID, current and target workspace path, dependency aliases, current and target Cargo package identity, Rust library identity, current and target container, target disposition, owner and role."
claim_class = "runtime_behavior"

[[requirement]]
id = "shared-packages-use-effortless-identities"
generation = 1
status = "accepted"
statement = "The retained shared logical crates use effortless-repo-protocol, effortless-repo-snapshot, effortless-repo-edit and effortless-rust-source-index as Cargo package identities and move to matching crates/effortless-* paths before first publication while retaining concise Rust library names and dependency aliases."
claim_class = "runtime_behavior"

[[requirement]]
id = "product-version-lines-independent"
generation = 1
status = "accepted"
statement = "cargo-allow, shared substrate, cargo-intent and retained cargo-proof package versions are independently represented; equal development versions never imply lockstep compatibility."
claim_class = "runtime_behavior"

[[requirement]]
id = "five-proof-packages-collapse-before-publication"
generation = 1
status = "accepted"
statement = "proof-provider-api and the four proof-adapter packages collapse into proof-engine or cargo-proof modules before first publication; no forwarding package or registry placeholder survives under an old name."
claim_class = "runtime_behavior"

[[requirement]]
id = "shared-substrate-dependency-neutral"
generation = 1
status = "accepted"
statement = "Shared substrate has no production dependency on allow, intent or proof product-domain crates; every temporary reverse edge has an exact owner, parity case, expiry and deletion condition."
claim_class = "runtime_behavior"

[[requirement]]
id = "repo-edit-neutral-selected-closure"
generation = 1
status = "accepted"
statement = "The repo-edit package selected by cargo-allow exposes neutral repository target, containment, lock, precondition, apply, atomicity and receipt contracts and has no product-domain dependency before entering the cargo-allow 0.2 candidate."
claim_class = "runtime_behavior"

[[requirement]]
id = "intent-model-canonical-authored-contracts"
generation = 1
status = "accepted"
statement = "intent-model is the canonical owner of durable authored intent and generation-2 architecture/package/move/shim/parity DTOs; historical cargo-allow readers cannot strengthen current authority."
claim_class = "runtime_behavior"

[[requirement]]
id = "intent-protocol-bounded-public-projections"
generation = 1
status = "accepted"
statement = "intent-protocol owns bounded query, view, obligation, diagnostic, edit-plan and settlement values and directly uses the canonical neutral repository protocol; consumers do not depend on the private graph."
claim_class = "runtime_behavior"

[[requirement]]
id = "intent-engine-single-read-only-compiler"
generation = 1
status = "accepted"
statement = "intent-engine is the sole selected source/authority compiler, private graph, graph comparison, phase policy, domain-query and generation-2 closure/reconciliation implementation; it remains read-only and does not execute proof or apply repository edits."
claim_class = "runtime_behavior"

[[requirement]]
id = "cargo-intent-only-current-intent-evaluator"
generation = 1
status = "accepted"
statement = "cargo-intent is the only selected current intent evaluator after cutover; cargo-allow delegates through a bounded installed-process protocol or fails explicitly and never silently falls back to an embedded evaluator."
claim_class = "runtime_behavior"

[[requirement]]
id = "proof-protocol-data-engine-semantics"
generation = 1
status = "accepted"
statement = "proof-protocol owns versioned DTOs, canonical serialization and local structural validation; proof-engine is the sole selected currentness, cache/reuse, capability, contradiction, aggregation and phase-gate semantic evaluator."
claim_class = "runtime_behavior"

[[requirement]]
id = "proof-consumes-intent-obligations"
generation = 1
status = "accepted"
statement = "proof-engine consumes the canonical intent-protocol obligation plan and owns only provider selection, proof planning and evidence lifecycle; it does not redefine repository obligations."
claim_class = "runtime_behavior"

[[requirement]]
id = "delegation-transport-bounded-and-single-validation"
generation = 1
status = "accepted"
statement = "Compatibility subprocess transport drains stdout and stderr concurrently under independent budgets, bounds timeout and reader settlement, passes OS-native paths, validates one envelope exactly once and retains distinct unavailable, incompatible, over-budget, malformed, stale, timeout and instrument-failure results."
claim_class = "runtime_behavior"

[[requirement]]
id = "move-complete-only-after-deletion-denominator"
generation = 1
status = "accepted"
statement = "A source, authority or package move is complete only when the selected new owner, parity disposition, old-path reachability, active shim state, package assets, docs, CI, rollback and exact deletion output agree."
claim_class = "runtime_behavior"

[[requirement]]
id = "package-topology-single-authority"
generation = 1
status = "accepted"
statement = "One checked package topology owns current and target package identities, explicit versions and version sources, publication/direct-use posture, candidate membership, selected features, release order, assets and CI lane for every observed or retained component."
claim_class = "runtime_behavior"

[[requirement]]
id = "cargo-allow-candidate-derived-from-clean-closure"
generation = 1
status = "accepted"
statement = "The cargo-allow 0.2 candidate is generated from a Complete selected product closure, packages every and only selected logical/package/version rows, contains no unpublished sibling dependency and is not selected by cargo package --workspace plus exclusions."
claim_class = "runtime_behavior"

[[requirement]]
id = "exact-candidate-installed-outside-workspace"
generation = 1
status = "accepted"
statement = "Release qualification installs the exact mixed-version cargo-allow graph from an isolated local registry with source checkout and ambient binaries denied, verifies resolved package names, versions and checksums and runs the selected first-hour/lifecycle journey."
claim_class = "runtime_behavior"

[[requirement]]
id = "cargo-proof-qualification-independent"
generation = 1
status = "accepted"
statement = "Removing obsolete proof package identities is an architecture prerequisite, while the full isolated three-package cargo-proof candidate and provider support evidence are independent cargo-proof publication/support prerequisites unless an integrated cargo-allow claim explicitly selects them."
claim_class = "runtime_behavior"

[[requirement]]
id = "support-visibility-and-extraction-separate"
generation = 1
status = "accepted"
statement = "Registry visibility, direct-library support, product support, integrated dogfood and physical repository extraction are separate decisions with separate evidence and claim boundaries."
claim_class = "runtime_behavior"

[[requirement]]
id = "release-requires-evidence-backed-complete"
generation = 1
status = "accepted"
statement = "No cargo-allow 0.2 tag, publication, attestation or public release is authorized until one exact candidate commit and tree has evidence-backed Complete release validation, explicit maintainer authorization and verified closeout."
claim_class = "runtime_behavior"
```

## Canonical ownership

| Concern | Canonical owner |
| --- | --- |
| source-exception policy and matching | cargo-allow family |
| authored intent and V2 governance DTOs | `intent-model` |
| private intent graph, phase policy and V2 closure/reconciliation | `intent-engine` |
| repository validation entry point and typed receipt | `cargo-intent` / repository CI |
| proof wire contracts and structural validation | `proof-protocol` |
| proof planning/currentness/cache/contradiction/gate semantics | `proof-engine` |
| built-in provider preparation/interpretation and registry | `cargo-proof` provider modules/application |
| safe repository byte application | `repo-edit` |
| semantic authoring settlement | `intent-edit` |

## Required convergence sequence

```text
A1  #2966 current-main normative retained authority
A2  #2967 parsed reconstruction and compatibility contracts
B1  #2921 strict current/target identities in intent-model
B2  #2922 exact observed/target Cargo closures in intent-engine
B3  #2923 current V2 enforcement through cargo-intent/repository CI
C1  #2970 minimum canonical intent cutover and embedded-authority deletion
C2  #2969 neutral repo-edit and clean selected cargo-allow shared closure
D1  #2936 canonical intent obligation input
D2  #2943 proof protocol/engine semantic boundary
D3  #2937 provider API and command-provider absorption
D4  #2938 built-in provider absorption
D5  #2937/#2938 absorb five packages; #2939 deletion-only alternative superseded
E   #2885 move/rename shared survivors and split independent versions
F   #2886 exact cargo-allow package/install/journey candidate
G   mutation, scanner, release-evidence, provenance, registry and support closeout
H   #2501 exact refreeze → explicit authorization → #2502 publication
```

Issue `#2941` may land independently when its allow-report/legacy projection
proof is clean. Issue `#2968` independently qualifies cargo-proof after topology
convergence and does not block cargo-allow core by default.

Each dependent branch starts from merged `main`. Opening a PR or obtaining one
green job is not completion.

## Accepted states

- Current observed and target topology are independently explicit.
- One intent compiler and phase evaluator remain.
- Compatibility has no semantic fallback.
- Shared selected packages are product-neutral.
- Proof protocol cannot decide semantic satisfaction.
- The current workspace and retained package denominator both equal 22 after the
  completed package-to-module absorptions.
- Cargo-allow packages and runs with intent/proof source trees unavailable.
- Experimental sibling-product failures do not block cargo-allow unless its
  selected support matrix includes that integrated surface.

## Rejected states

- Treating all 27 observed packages as permanent release units.
- Implementing canonical V2 governance permanently inside `allow-policy`.
- Publishing generic shared package names or proof adapter placeholders.
- Leaving an old evaluator reachable after claiming cutover.
- Allowing a shared package selected by cargo-allow to depend on cargo-allow
  product ontology.
- Letting process exit or protocol parsing imply proof satisfaction.
- Selecting package membership through shell arrays or exclusions that disagree
  with checked topology.
- Treating integrated dogfood as automatic repository-extraction authorization.

## Required evidence

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo run -p cargo-allow --locked -- check --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

Each stage adds exact negative fixtures, parity/deletion receipts or installed
candidate proof. A stage may not inherit a stronger claim from a narrower one.

## Non-goals

- Changing the current package topology, paths, names, or versions in this
  documentation reconciliation.
- Publishing cargo-allow, cargo-intent, cargo-proof or shared packages.
- Making experimental sibling products stable because their scaffolds exist.
- Defining a public raw graph API.
- Authorizing physical repository extraction.

## Claim boundary

This specification defines the target topology, semantic ownership,
compatibility, package survival, exact candidate and release behavior for the
landed monorepo. It does not prove current implementation compliance, perform a
migration, qualify sibling product maturity, authorize cargo-allow 0.2 or
authorize repository extraction.
