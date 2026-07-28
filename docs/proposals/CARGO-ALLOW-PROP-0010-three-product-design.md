---
id: CARGO-ALLOW-PROP-0010
kind: proposal
status: accepted
owner: repo-infra
created: 2026-07-22
updated: 2026-07-28
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

The crate families and product shells now exist. The remaining work is not crate
creation. It is convergence: establish correct package identities and version
lines, neutralize shared dependencies, move the canonical spec graph into
`intent-engine`, cut compatibility over without fallback, delete duplicate
authority, and qualify each product from its exact package closure.

This proposal retains the product split decided in Issue #2550 and the logical
crate topology ratified in Issue #2612. CARGO-ALLOW-ADR-0002 owns semantic
product boundaries. CARGO-ALLOW-ADR-0003 owns package identity and versioning.
CARGO-ALLOW-SPEC-0011 owns the remaining convergence and release behavior.

## Problem

Cargo-allow’s source-exception ledger, repository-intent compiler, and proof
orchestration began in one implementation tree. The crate extraction has now
landed enough structure to expose the actual remaining risk:

- a logical crate ID can be mistaken for a crates.io package name;
- all workspace members can accidentally inherit cargo-allow’s release version;
- shared implementation crates can retain reverse dependencies on product
  ontology;
- `cargo-intent` can exist while the real graph compiler remains reachable in
  cargo-allow crates;
- compatibility can appear cut over while an embedded evaluator remains a
  fallback;
- package smoke can pass from the ambient workspace without proving a clean
  registry-resolvable product closure;
- an experimental sibling product can block or contaminate the cargo-allow
  release even when it is not selected.

A fresh builder should be able to reconstruct current ownership, implementation
posture, package identity, remaining deletion work, and release gates from
retained repository authority—not from issue archaeology or matching directory
names.

## Product authorities

| Product | Owns | Does not own |
| --- | --- | --- |
| cargo-allow | source inventory and sensors, source-exception findings, `policy/allow.toml`, matching, lifecycle, no-new/diff posture, ledger mutation and cargo-allow reports | durable repository intent, private intent graph, proof execution or cross-provider currentness |
| cargo-intent | authored proposals/specs/ADRs/requirements/transactions, authority and dialect resolution, private compiled intent graph, phase obligations, bounded domain queries, later semantic edit settlement | source-exception ledger policy, proof-provider execution, merge policy |
| cargo-proof | proof planning, registered provider execution or ingestion, exact-snapshot currentness, receipt composition, contradictions and phase-gate evidence | authored product direction, cargo-allow matching semantics, provider-private domain meaning |

The three products can participate in one operator journey without becoming one
semantic authority or one release unit.

## Current implementation status

| Surface | Current status | Exact boundary |
| --- | --- | --- |
| cargo-allow published channel | `SupportedPublished` | `0.1.11`; registry and documentation evidence apply only to that release |
| cargo-allow source candidate | `SupportedSourceCandidate` | workspace is versioned `0.2.0`; no tag or publication is authorized |
| cargo-intent | `LandedExperimental` | read-only staged-precommit walking skeleton and process protocol exist; canonical graph/compiler cutover is incomplete |
| cargo-proof | `LandedExperimental` | protocol, planner, dry-run, provider contracts and captured-report adapters exist; real provider execution and external cutover are incomplete |
| shared substrate | `LandedTransitional` | all four logical crates exist; package identities, explicit version lines and dependency neutrality are not yet converged |
| legacy spec-system compatibility | `CompatibilityOnly` and `BlockedOnParity` | one staged operation delegates; old semantic modules and historical/current assets are not fully deleted |
| physical repository extraction | `NotStarted` | not authorized by crate existence, package smoke or integrated dogfood |

Use the closed status vocabulary from CARGO-ALLOW-SPEC-0011. Do not describe a
landed crate as “planned,” and do not describe a skeleton or local fixture as
“complete.”

## Logical crate topology

Issue #2612 and `policy/product-crates.toml` own the logical topology. This
summary is a projection, not a second package manifest.

```text
cargo-allow
  allow-core, allow-policy, allow-inventory, allow-files, allow-rust,
  allow-match, allow-report, allow-diff, allow-policy-legacy, cargo-allow

shared substrate
  repo-protocol, repo-snapshot, repo-edit, rust-source-index

cargo-intent
  intent-model, intent-protocol, intent-engine, intent-edit, cargo-intent

cargo-proof
  proof-protocol, proof-provider-api, proof-engine, proof-adapter-command,
  proof-adapter-cargo-allow, proof-adapter-ripr, proof-adapter-hawk, cargo-proof
```

Logical IDs, Cargo package names, and Rust library names are separate identities.
CARGO-ALLOW-ADR-0003 defines the shared package mapping:

```text
repo-protocol      → effortless-repo-protocol      → repo_protocol
repo-snapshot      → effortless-repo-snapshot      → repo_snapshot
repo-edit          → effortless-repo-edit          → repo_edit
rust-source-index  → effortless-rust-source-index  → rust_source_index
```

The short logical IDs remain the architecture, path, move-ledger, and workspace
alias vocabulary. The `effortless-*` names are the Cargo registry identities.

## Version and publication posture

The cargo-allow family retains `0.2.0` for its release train. Shared,
cargo-intent, and cargo-proof packages start on explicit experimental `0.1.0`
lines before first publication. Equal experimental versions are a development
cohort, not a lockstep compatibility guarantee.

Registry visibility, supported direct-library use, supported product behavior,
and physical repository extraction are different decisions. A shared crate may
be published only as a transitive dependency of a supported product without
becoming a supported direct API.

No generic `repo-*` compatibility package is published. No empty package is
published merely to reserve a name. The external `effortless` suite bootstrap
is a functional installer/updater/doctor and does not enter a product dependency
closure.

## Dependency direction

CARGO-ALLOW-ADR-0002 remains authoritative for allowed and forbidden semantic
edges. The important final direction is:

```text
shared substrate
  → no product-domain crate

intent-model
  → neutral protocol types only

intent-engine
  → intent-model + intent-protocol + neutral source/index substrate
  → no intent-edit, proof engine or cargo-allow private implementation

intent-edit
  → intent-engine + repo-edit

proof-engine
  → intent-protocol + proof protocol/provider API + neutral snapshot substrate
  → no intent-engine or cargo-allow private implementation

cargo-allow compatibility
  → installed cargo-intent process protocol
  → no intent/proof library dependency or semantic fallback
```

Temporary reverse edges require an exact move/shim record, parity case, expiry,
and deletion condition. They are visible non-final state, not evidence that the
architecture is clean.

## Convergence sequence

The implementation plan now starts from the landed workspace:

```text
A  retained generation-2 authority
B  strict identity and selected-closure validation
C  effortless-* rename and independent version split
D  shared dependency neutralization
E  canonical intent model/protocol/engine/application wiring
F  hardened process delegation and operation-by-operation compatibility cutover
G  embedded evaluator, current schema, asset and CI deletion
H  topology-selected exact cargo-allow candidate outside the workspace
I  evidence-backed 0.2 release closeout
J  real cargo-proof execution, external dogfood, simplification and later
   repository-extraction decision
```

Every dependent writer starts from merged `main`. A lane continues through
implementation, focused proof, PR review, valid fixes, complete hosted CI,
thread resolution, merge, and merged-main verification. “PR opened,” “CI
started,” and “ready for review” are not completion states.

## Release boundaries

### cargo-allow 0.2.x

Cargo-allow can release independently when its selected package closure is clean,
its required shared transitive packages have final identities, embedded current
intent authority is absent from the selected product, and the release trust
train is complete. Full cargo-proof provider maturity is not a cargo-allow core
release prerequisite.

### cargo-intent 0.1.x

Cargo-intent remains experimental until it owns the sole canonical authored
model, compiler, phase policy and query service; configured many-source graph
compilation works; compatibility parity is accepted; and the old evaluator is
removed.

### cargo-proof 0.1.x

Cargo-proof remains experimental until at least one real provider executes or is
ingested through the selected product, exact snapshot/config/tool currentness is
enforced, and external dogfood replaces fake or stubbed product paths.

## Repository extraction

Physical repository extraction is not authorized by this package. It becomes a
bounded operational option only after:

- product package and CI closures are independently proven;
- public process/protocol boundaries have external dogfood evidence;
- migration shims and private source dependencies are removed;
- a simplification review confirms that retained crate boundaries earn their
  cost; and
- a separate explicit authorization names the repositories and exact source
  state to move.

Passing an integrated smoke or extraction-readiness receipt is necessary input,
not automatic authorization.

## Machine-source law

| Concern | Authority |
| --- | --- |
| logical crate topology and roles | Issue #2612 and `policy/product-crates.toml` |
| package/version/publication/candidate/CI posture | Issue #2604 and `policy/product-package-topology.toml` |
| package and library naming | CARGO-ALLOW-ADR-0003 plus generation-2 identity checks |
| current source disposition and deletion output | Issue #2598 and `policy/product-move-ledger.toml` |
| temporary compatibility | Issue #2607 and `policy/extraction-shims.toml` |
| parity and cutover evidence | Issue #2606 and `policy/extraction-parity.toml` |
| release authorization | Issues #2371, #2501 and #2502 |

Human diagrams and tables consume or are checked against those sources. A new
crate, package rename, version-source change, authority move or candidate change
updates every affected machine authority in one reviewed migration.

## Success criteria

- A fresh builder can identify the current semantic owner, package identity,
  support posture, transitional edges and deletion output for every boundary.
- Shared substrate has no final production dependency on product ontology.
- One intent compiler and one phase evaluator remain after cutover.
- Cargo-allow packages and runs with sibling product source trees absent.
- The cargo-allow candidate is generated from a clean mixed-version selected
  closure rather than the ambient workspace.
- Each product’s support and release claims match the exact package bytes and
  evidence shipped.
- Repository extraction, when later chosen, requires no authority redesign.

## Non-goals

- Changing the accepted three-product split.
- Renaming packages or changing versions in this documentation PR.
- Publishing any package or tagging cargo-allow 0.2.
- Calling cargo-intent or cargo-proof supported because their crates exist.
- Exposing the private compiled graph as a public API.
- Authorizing physical repository extraction.

## Claim boundary

This proposal records the landed product state, package-identity direction,
remaining convergence sequence, independent release boundaries, and deletion
gates. It does not prove code compliance, rename or publish packages, complete
semantic parity, authorize cargo-allow 0.2, establish sibling-product maturity,
or authorize repository extraction.
