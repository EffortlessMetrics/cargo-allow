# Three-Product Generation-2 Reconstruction Fixture

This fixture verifies that a fresh builder can reconstruct current cargo-allow,
cargo-intent, cargo-proof, shared-package, compatibility, and release authority
from retained repository sources rather than chat or issue archaeology.

`disposition-map.toml` is a parsed snapshot used by tests. It is not a second
architecture, package, move, shim, or parity ledger.

## Authority entry points

| Question | Current answer location |
| --- | --- |
| Why are there three products? | `docs/proposals/CARGO-ALLOW-PROP-0010-three-product-design.md` |
| Who owns each semantic boundary? | `docs/adr/CARGO-ALLOW-ADR-0002-three-product-ownership.md` |
| Which Cargo and Rust identities name shared crates? | `docs/adr/CARGO-ALLOW-ADR-0003-package-identity-and-versioning.md` |
| Which convergence and release requirements are current? | `docs/specs/CARGO-ALLOW-SPEC-0011-three-product-convergence.md` |
| Which requirements are historical? | `docs/specs/CARGO-ALLOW-SPEC-0010-three-product-boundaries.md` and its exact supersession map |
| What is the PR-by-PR sequence? | `plans/three-product-crate-extraction.md` |
| What is currently supported versus experimental? | `docs/status/SUPPORT_TIERS.md` |
| What is the checked logical topology? | `policy/product-crates.toml` and Issue #2612 |
| What is the checked package/release topology? | `policy/product-package-topology.toml` and Issue #2604 |
| Where is source disposition and deletion output recorded? | `policy/product-move-ledger.toml` and Issue #2598 |
| Where are temporary compatibility surfaces bounded? | `policy/extraction-shims.toml` and Issue #2607 |
| What makes parity/cutover evidence meaningful? | `policy/extraction-parity.toml` and Issue #2606 |
| What authorizes cargo-allow 0.2? | Issues #2371, #2501 and #2502; no current authorization exists |

## Product definitions

```text
cargo-allow  = source-exception ledger
cargo-intent = durable authored intent and obligation compiler
cargo-proof  = exact-snapshot evidence orchestration
```

## Current state to reconstruct

```text
cargo-allow
  Published:              0.1.11
  Source candidate:       0.2.0
  Current release state:  blocked; no tag or publication authorization

cargo-intent
  LandedExperimental
  Read-only staged-precommit walking skeleton
  Canonical many-source graph/compiler cutover incomplete

cargo-proof
  LandedExperimental
  Protocol/planner/dry-run/provider-contract skeleton
  Real provider execution and external cutover incomplete

shared substrate
  LandedTransitional
  All four logical crates exist
  effortless-* package rename, explicit 0.1 versions and dependency neutrality
  remain implementation work
```

A crate, binary, fixture, or local smoke is not by itself a support promotion,
authority cutover, or release.

## Shared identity reconstruction

| Logical ID | Current transitional package | Target Cargo package | Rust library |
| --- | --- | --- | --- |
| `repo-protocol` | `repo-protocol` | `effortless-repo-protocol` | `repo_protocol` |
| `repo-snapshot` | `repo-snapshot` | `effortless-repo-snapshot` | `repo_snapshot` |
| `repo-edit` | `repo-edit` | `effortless-repo-edit` | `repo_edit` |
| `rust-source-index` | `rust-source-index` | `effortless-rust-source-index` | `rust_source_index` |

The short names remain logical IDs, workspace paths/aliases, and Rust import
roots. The `effortless-*` values are the future registry package identities.

## Final dependency test

A fresh builder should derive these rules:

```text
shared substrate
  has no final product-domain dependency

intent-engine
  is the sole read-only compiler/query implementation
  does not depend on intent-edit or proof execution

intent-edit
  depends on intent-engine + repo-edit

proof-engine
  consumes intent-protocol, never the private intent graph

cargo-allow compatibility
  invokes installed cargo-intent through bounded process protocol
  has no intent/proof library dependency and no semantic fallback
```

## Current versus compatibility authority

```text
Canonical current intent after cutover
  intent-model + intent-protocol + intent-engine + cargo-intent

Compatibility/deletion targets
  allow-policy::spec_system current semantics
  cargo-allow spec_system* compiler/query/application paths
  current cargo-allow-owned intent schemas/templates/CI claims

Historical readers
  exact original generations retained for migration/provenance only
```

The current source still contains transitional paths; the fixture must classify
them honestly rather than treating the intended final graph as already clean.

## Convergence train

```text
safety repair for merged delegation transport
→ retained generation-2 authority
→ strict logical/package/lib identity and closure validation
→ atomic effortless-* rename and independent version split
→ shared dependency neutralization
→ canonical intent compiler/query wiring
→ operation-by-operation process cutover
→ embedded evaluator/schema/asset/CI deletion
→ topology-selected mixed-version cargo-allow candidate
→ evidence-backed exact 0.2 refreeze, authorization and release
```

Cargo-proof real provider execution and external dogfood can continue on its own
experimental train and does not block cargo-allow core unless the selected 0.2
support matrix explicitly includes an integrated proof claim.

## PR lifecycle

Every dependent lane follows the full lifecycle:

```text
reconstruct from merged main
→ implement
→ run focused proof
→ open/update PR
→ inspect review and hosted CI
→ fix every valid finding
→ rerun complete required checks
→ resolve threads
→ merge
→ verify merged main
→ start the next dependent branch from merged main
```

Never hand off a dependent lane as complete merely because it is opened,
mergeable, ready for review, or partially green.

## Validation commands

```bash
cargo test -p allow-policy --test three_product_design --locked -- --nocapture
cargo test -p cargo-allow spec_design_artifact_links --locked -- --nocapture
cargo test --workspace --locked
cargo run -p cargo-allow --locked -- check --mode no-new
```

The repository’s exact hosted CI remains authoritative for cross-platform,
package, shallow-history, and integrated proof classes.

## Claim boundary

This fixture proves that retained sources are mutually reconstructable and that
the generation-2 status vocabulary and identity mappings are present. It does
not prove dependency neutrality, semantic parity, package publication, release
readiness, sibling-product maturity, or repository-extraction authorization.
