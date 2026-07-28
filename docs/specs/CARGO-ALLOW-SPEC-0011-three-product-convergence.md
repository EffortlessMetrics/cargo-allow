---
id: CARGO-ALLOW-SPEC-0011
kind: spec
status: accepted
owner: repo-infra
created: 2026-07-28
linked_proposal: CARGO-ALLOW-PROP-0010
linked_adrs:
  - CARGO-ALLOW-ADR-0002
  - CARGO-ALLOW-ADR-0003
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

This specification starts from the landed 27-crate workspace. It replaces the
crate-creation sequencing in CARGO-ALLOW-SPEC-0010 with the remaining
convergence, cutover, deletion, packaging, and release requirements.

CARGO-ALLOW-SPEC-0010 remains the historical generation-1 product-boundary
record. Requirements not named in `supersedes_requirement_ids` remain
supporting authority where they do not conflict with this specification.

## Current implementation state

```text
cargo-allow
  supported published channel: 0.1.11
  landed source candidate: 0.2.0
  product authority: source-exception ledger

cargo-intent
  landed experimental read-only walking skeleton
  current command: staged precommit change status
  canonical graph/compiler cutover: incomplete

cargo-proof
  landed experimental planner, dry-run, protocol, and adapter skeletons
  real provider execution and external cutover: incomplete

shared substrate
  all four logical crates landed
  package identities, version lines, and dependency neutrality: incomplete
```

The existence of a crate, binary, fixture, or local smoke does not by itself
promote support or complete an authority move.

## Normative requirements

```toml cargo-allow-requirements
schema_version = "1.0"

[[requirement]]
id = "identity-distinguishes-logical-package-lib"
generation = 1
status = "accepted"
statement = "Every workspace crate has one checked logical ID, workspace path, dependency alias set, Cargo package name, Rust library name, and product or shared owner; those identities are never inferred to be equal merely because they currently use similar text."
claim_class = "runtime_behavior"

[[requirement]]
id = "shared-packages-use-effortless-identities"
generation = 1
status = "accepted"
statement = "The shared logical crates repo-protocol, repo-snapshot, repo-edit, and rust-source-index use effortless-repo-protocol, effortless-repo-snapshot, effortless-repo-edit, and effortless-rust-source-index as their Cargo package identities before first publication while retaining concise Rust library names and workspace aliases."
claim_class = "runtime_behavior"

[[requirement]]
id = "product-version-lines-independent"
generation = 1
status = "accepted"
statement = "cargo-allow, shared substrate, cargo-intent, and cargo-proof package versions are independently represented; equal development versions never imply permanent lockstep compatibility."
claim_class = "runtime_behavior"

[[requirement]]
id = "shared-substrate-dependency-neutral"
generation = 1
status = "accepted"
statement = "Shared substrate has no production dependency on allow, intent, or proof product-domain crates; every temporary reverse edge has an exact owner, parity case, expiry, and deletion condition."
claim_class = "runtime_behavior"

[[requirement]]
id = "intent-model-canonical-authored-contracts"
generation = 1
status = "accepted"
statement = "intent-model is the only canonical owner of durable authored intent contracts after cutover; cargo-allow compatibility readers and historical generations cannot strengthen current intent authority."
claim_class = "runtime_behavior"

[[requirement]]
id = "intent-protocol-bounded-public-projections"
generation = 1
status = "accepted"
statement = "intent-protocol owns bounded query, view, obligation, diagnostic, edit-plan, and settlement transport values; consumers do not depend on the private compiled graph."
claim_class = "runtime_behavior"

[[requirement]]
id = "intent-engine-single-read-only-compiler"
generation = 1
status = "accepted"
statement = "intent-engine is the sole selected source adapter, authority resolver, graph compiler, graph comparison, phase policy, and domain-query implementation after parity; it remains read-only and does not execute proof or apply repository edits."
claim_class = "runtime_behavior"

[[requirement]]
id = "cargo-intent-only-current-intent-evaluator"
generation = 1
status = "accepted"
statement = "cargo-intent is the only current intent evaluator after compatibility cutover; cargo-allow delegates through a bounded process protocol or fails explicitly and never silently falls back to an embedded evaluator."
claim_class = "runtime_behavior"

[[requirement]]
id = "delegation-transport-bounded-and-single-validation"
generation = 1
status = "accepted"
statement = "Compatibility subprocess transport drains stdout and stderr concurrently under independent budgets, settles timeout and error cleanup without indefinite blocking, passes OS-native paths, validates one semantic envelope exactly once, and retains distinct over-budget, malformed, stale, incompatible, timeout, and instrument-failure results."
claim_class = "runtime_behavior"

[[requirement]]
id = "move-complete-only-after-deletion-denominator"
generation = 1
status = "accepted"
statement = "A source or authority move is complete only when the selected new owner, parity disposition, old-path reachability, active shim state, package assets, documentation, CI lane, rollback, and exact deletion output agree."
claim_class = "runtime_behavior"

[[requirement]]
id = "package-topology-single-authority"
generation = 1
status = "accepted"
statement = "One checked package topology owns package names, explicit versions and version sources, publication posture, direct-use support, candidate membership, selected features, release order, default membership, assets, and CI lane for every workspace package."
claim_class = "runtime_behavior"

[[requirement]]
id = "cargo-allow-candidate-derived-from-clean-closure"
generation = 1
status = "accepted"
statement = "The cargo-allow 0.2 candidate is generated from the checked selected product closure, packages every and only selected logical/package/version rows, contains no unpublished sibling dependency, and is not selected by cargo package --workspace plus exclusions."
claim_class = "runtime_behavior"

[[requirement]]
id = "exact-candidate-installed-outside-workspace"
generation = 1
status = "accepted"
statement = "Release qualification installs the exact mixed-version cargo-allow package graph from an isolated local registry with the source checkout and ambient binaries denied, verifies the resolved package names, versions, and checksums, and runs the supported first-hour and lifecycle journey."
claim_class = "runtime_behavior"

[[requirement]]
id = "support-visibility-and-extraction-separate"
generation = 1
status = "accepted"
statement = "Registry visibility, supported direct library use, supported product behavior, integrated dogfood, and physical repository extraction are separate decisions with separate evidence and claim boundaries."
claim_class = "runtime_behavior"

[[requirement]]
id = "release-requires-evidence-backed-complete"
generation = 1
status = "accepted"
statement = "No cargo-allow 0.2 tag, publication, attestation, or public release is authorized until one exact candidate commit and tree has evidence-backed Complete release validation, explicit maintainer authorization, and verified complete closeout."
claim_class = "runtime_behavior"
```

## Machine authorities

| Concern | Canonical source |
| --- | --- |
| Logical topology and ownership | `policy/product-crates.toml` and Issue #2612 |
| Package, version, publication, candidate, and CI posture | `policy/product-package-topology.toml` and Issue #2604 |
| Logical/package/lib naming law | CARGO-ALLOW-ADR-0003 and generation-2 identity checks |
| Current source disposition and deletion output | `policy/product-move-ledger.toml` and Issue #2598 |
| Temporary compatibility | `policy/extraction-shims.toml` and Issue #2607 |
| Parity and cutover meaning | `policy/extraction-parity.toml` and Issue #2606 |
| Exact release authority | Issues #2371, #2501, and #2502 |

Human tables and diagrams are projections or contract-checked summaries. They
do not become competing package or ownership authorities.

## Required convergence sequence

```text
A  retained generation-2 authority and identity law
B  strict generation-2 machine manifests and closure validation
C  atomic effortless-* package rename and independent version split
D  shared dependency neutralization
E  canonical intent model/protocol/engine/application wiring
F  hardened process delegation and operation-by-operation cutover
G  embedded evaluator, current schema, asset, and CI deletion
H  topology-selected exact cargo-allow candidate qualification
I  evidence-backed release closeout and exact 0.2 authorization
J  real cargo-proof execution, external dogfood, simplification, and later
   repository-extraction decision on independent evidence
```

A dependent implementation PR is reconstructed from merged `main` after the
previous gate. Opening a PR or obtaining one green job is not completion.

## Accepted states

- Cargo-allow packages, installs, tests, and releases without intent/proof source
  crates or private paths.
- Cargo-intent compiles many configured authored sources through one private graph
  and returns bounded protocol queries.
- Compatibility has no semantic fallback.
- Shared substrate is dependency-neutral.
- The cargo-allow candidate is a checked mixed-version closure.
- Experimental sibling-product failures do not block cargo-allow unless the
  selected cargo-allow support matrix includes that compatibility surface.

## Rejected states

- Treating all 27 workspace members as one release unit.
- Publishing a generic shared package name as a temporary convenience.
- Calling a walking skeleton a supported product because its binary launches.
- Leaving an old evaluator reachable after claiming cutover.
- Selecting package membership through shell arrays or exclusions that disagree
  with the package topology.
- Claiming a clean release from environment strings, unverified artifacts, or a
  later retry that erases an earlier release incident.
- Treating successful integrated dogfood as automatic repository-extraction
  authorization.

## Required evidence

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo run -p cargo-allow -- check --mode no-new
```

Each implementation stage adds its own exact negative fixtures, installed
candidate proof, parity receipt, or release artifact. A stage may not inherit a
clean claim from a narrower predecessor.

## Non-goals

- Renaming packages in this documentation PR.
- Publishing cargo-allow, cargo-intent, cargo-proof, or shared crates.
- Making cargo-intent or cargo-proof stable merely because their crate families
  exist.
- Defining a public raw graph API.
- Authorizing physical repository extraction.

## Claim boundary

This specification defines the remaining convergence, cutover, deletion,
package, and release behavior for the landed monorepo. It does not prove current
code compliance, publish packages, authorize cargo-allow 0.2, establish sibling
product maturity, or authorize repository extraction.
