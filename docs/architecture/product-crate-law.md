# Product Crate Law

Human entry point for the checked three-product architecture. Machine manifests
remain authoritative; this page explains which source answers which question and
how the observed extraction scaffold differs from the retained target.

## Authority map

| Concern | Canonical authority |
| --- | --- |
| Product ownership and dependency direction | `docs/adr/CARGO-ALLOW-ADR-0002-three-product-ownership.md` |
| Package, path and independent version law | `docs/adr/CARGO-ALLOW-ADR-0003-package-identity-and-versioning.md` |
| Current convergence requirements | `docs/specs/CARGO-ALLOW-SPEC-0011-three-product-convergence.md` |
| Current logical/package identities | `policy/product-crates-v2.toml` under #2923/#3391 |
| Exact current Cargo closure and candidate posture | `policy/product-package-topology-v2.toml` under #2923/#3391 |
| Selected current enforcement and receipt | #2923 through cargo-intent/repository CI |
| Source disposition and retirement output | `policy/product-move-ledger.toml` / #2598 |
| Temporary compatibility | `policy/extraction-shims.toml` / #2607 |
| Parity and cutover evidence | `policy/extraction-parity.toml` / #2606 |
| Historical generation-1 package posture | `policy/product-package-topology.toml` / #2604 |

Human tables and diagrams are generated or contract-checked projections. They do
not become competing ownership or package manifests.

## Current and historical topology

The selected current source contains the retained 22-package topology:

```text
cargo-allow family   10
shared substrate      4
cargo-intent family   5
cargo-proof family    3
```

The former maximum extraction scaffold contained 27 packages. Its five extra
proof boundaries were absorbed into retained modules:

| Historical package | Current module |
| --- | --- |
| `proof-provider-api` | `proof_engine::provider_api` |
| `proof-adapter-command` | `proof_engine::command_adapter` |
| `proof-adapter-cargo-allow` | `cargo_proof::providers::cargo_allow` |
| `proof-adapter-ripr` | `cargo_proof::providers::ripr` |
| `proof-adapter-hawk` | `cargo_proof::providers::hawk` |

Current package-count convergence does not establish final semantic authority,
dependency neutrality, support, publication, or physical repository extraction.

## Identity law

Generation 1 overloaded one string for several identities. Generation 2 keeps
these separate:

```text
logical_id
current_workspace_path
target_workspace_path
workspace_dependency_aliases
current_cargo_package_name
target_cargo_package_name
rust_library_name
current_container
target_container_or_module
target_disposition
package_version and version_source
publication/direct-use/candidate/CI posture
```

The shared migration is:

| Logical ID | Current path | Target path | Cargo package | Rust library |
| --- | --- | --- | --- | --- |
| `repo-protocol` | `crates/repo-protocol` | `crates/effortless-repo-protocol` | `effortless-repo-protocol` | `repo_protocol` |
| `repo-snapshot` | `crates/repo-snapshot` | `crates/effortless-repo-snapshot` | `effortless-repo-snapshot` | `repo_snapshot` |
| `repo-edit` | `crates/repo-edit` | `crates/effortless-repo-edit` | `effortless-repo-edit` | `repo_edit` |
| `rust-source-index` | `crates/rust-source-index` | `crates/effortless-rust-source-index` | `effortless-rust-source-index` | `rust_source_index` |

`logical_id` is stable. A workspace path is checked current physical identity and
changes only through a reviewed migration generation.

## Semantic owner law

```text
cargo-allow family
  source-exception ledger semantics

intent-model
  durable authored intent and generation-2 governance DTOs

intent-engine
  private intent graph, phase policy, domain queries,
  current/target authority and Cargo-closure reconciliation

cargo-intent / repository CI
  validation operation and typed receipt

proof-protocol
  wire DTOs and local structural validation

proof-engine
  planning, execution policy, currentness/cache reuse,
  capability, contradiction, aggregation and phase-gate semantics

cargo-proof
  application and selected built-in provider modules
```

`allow-policy` must not become permanent three-product governance ontology merely
because generation-1 validators currently live there.

## Dependency law

The final architecture requires:

```text
shared substrate
  no production dependency on allow, intent or proof product ontology

cargo-allow
  no intent/proof library dependency
  compatibility through installed cargo-intent only

intent-engine
  one read-only compiler/query implementation
  no intent-edit, proof execution or cargo-allow private dependency

intent-edit
  intent-engine + repo-edit

proof-protocol
  no cache, registry, provider execution or phase policy

proof-engine
  canonical intent-protocol obligations
  no private intent graph or cargo-proof provider-module dependency

cargo-proof provider modules
  proof-engine public provider surface only
```

Temporary reverse edges are visible non-final state. They require exact move,
shim, parity, expiry and retirement records; they are not suppressed to make a
report green.

## Package and release closure

The cargo-allow `0.2.x` candidate is not the ambient workspace. It is the exact
package/version/feature closure selected by generation-2 package topology,
combining cargo-allow `0.2.x` packages with selected product-neutral
`effortless-* 0.1.x` transitive packages.

Cargo-intent and cargo-proof remain independently experimental. Their full
product maturity does not block cargo-allow unless the selected support matrix
explicitly includes that compatibility or integration claim.

## Validation progression

```text
#2966  retained normative authority
#2967  parsed reconstruction and compatibility contracts
#2921  strict current/target identities
#2922  exact observed/target closures
#2923  current V2 enforcement
#2937/#2938  proof packages absorbed; current topology converged to 22
#2939  deletion-only alternative superseded by absorption
#2885  survivor package/path/version migration
```

## Claim boundary

This page explains current architecture authority and historical convergence.
It does not claim semantic-owner or dependency-neutrality completion, authorize
cargo-allow `0.2.x`, promote sibling products, or authorize physical repository
extraction.
