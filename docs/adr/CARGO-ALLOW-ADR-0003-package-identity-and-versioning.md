---
id: CARGO-ALLOW-ADR-0003
kind: adr
status: accepted
owner: repo-infra
created: 2026-07-29
updated: 2026-08-12
linked_proposal: CARGO-ALLOW-PROP-0010
linked_spec: CARGO-ALLOW-SPEC-0011
supersedes: none
superseded_by: none
support_tier_impact: advisory
policy_impact:
  - policy/product-crates.toml
  - policy/product-package-topology.toml
  - .allow/artifacts/doc-artifacts.toml
---

# ADR: Package Identity, Physical Paths and Independent Version Lines

## Context

The repository previously contained a 27-package extraction scaffold across
cargo-allow, shared substrate, cargo-intent and cargo-proof. It now contains the
retained 22-package topology. The first
architecture and package manifests were created while workspace directory,
dependency alias, Cargo package name, Rust library name and workspace version
often used the same short string.

That coincidence is not the intended registry or release contract:

- the four shared packages need owned `effortless-*` registry identities;
- the three product families have different maturity and release posture;
- workspace paths are current physical facts, not permanent semantic identity;
- five proof packages were absorbed into retained modules without receiving
  publication identities; and
- cargo-allow must qualify a mixed-version selected package graph rather than
  treating the entire workspace as one release unit.

## Decision

Treat the following as distinct checked identities:

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
product_or_shared_owner
package_version
version_source
publication_state
direct_use_support
candidate_membership
```

`logical_id` is the stable architecture and move-ledger identity. Workspace path
is exact current physical repository identity and may change only through a
reviewed migration generation. Cargo package/version are registry identities.
Rust library name is the import identity. Dependency aliases are local manifest
syntax and are not serialized as though they were registry package names.

## Shared package identities

| Logical ID | Historical path | Current path | Cargo package | Rust library |
| --- | --- | --- | --- | --- |
| `repo-protocol` | `crates/repo-protocol` | `crates/effortless-repo-protocol` | `effortless-repo-protocol` | `effortless_repo_protocol` |
| `repo-snapshot` | `crates/repo-snapshot` | `crates/effortless-repo-snapshot` | `effortless-repo-snapshot` | `effortless_repo_snapshot` |
| `repo-edit` | `crates/repo-edit` | `crates/effortless-repo-edit` | `effortless-repo-edit` | `effortless_repo_edit` |
| `rust-source-index` | `crates/rust-source-index` | `crates/effortless-rust-source-index` | `effortless-rust-source-index` | `effortless_rust_source_index` |

The former concise dependency aliases remain historical migration evidence, not
current workspace identities:

```toml
[workspace.dependencies]
repo-protocol = {
  package = "effortless-repo-protocol",
  path = "crates/effortless-repo-protocol",
  version = "0.1.0",
}
```

The historical alias did not reserve or publish the generic package name.

## Product package identities

Surviving product-owned package names remain native to their product families:

```text
allow-* and cargo-allow
intent-* and cargo-intent
proof-protocol, proof-engine and cargo-proof
```

They are not renamed to `effortless-*`. The `effortless` suite bootstrap is an
external functional installer/updater/doctor and is not a dependency of these
packages.

## Package-to-module dispositions

The following historical packages received no final publication/version identity:

| Historical package | Current container/module | Disposition |
| --- | --- | --- |
| `proof-provider-api` | `proof_engine::provider_api` | `CompletedAbsorption` |
| `proof-adapter-command` | `proof_engine::command_adapter` | `CompletedAbsorption` |
| `proof-adapter-cargo-allow` | `cargo_proof::providers::cargo_allow` | `CompletedAbsorption` |
| `proof-adapter-ripr` | `cargo_proof::providers::ripr` | `CompletedAbsorption` |
| `proof-adapter-hawk` | `cargo_proof::providers::hawk` | `CompletedAbsorption` |

No empty forwarding package, symlinked package, compatibility placeholder or
crates.io reservation is created under those names. Git and retained move/parity
history preserve the extraction provenance.

A later package re-extraction requires a new reviewed target generation and
evidence of an external consumer, independent compatibility contract, unique
heavy/native/toolchain dependency or measured build/distribution value.

## Initial version posture

Before first publication of newly extracted packages:

| Family | Initial version posture |
| --- | --- |
| cargo-allow product family | retain `0.2.0` for the 0.2 release train |
| shared `effortless-*` packages | explicit `0.1.0`, experimental or transitive-only |
| cargo-intent family | explicit `0.1.0`, experimental |
| retained cargo-proof family | explicit `0.1.0`, experimental |

Equal `0.1.0` values are a development cohort, not a permanent lockstep
compatibility promise. Every package version is independently represented and
may diverge when its public compatibility surface requires it.

## Publication and support are separate

The checked package topology distinguishes at least:

```text
UnpublishedInternal
RegistryTransitiveOnly
ExperimentalDirect
SupportedDirect
HistoricalCompatibility
CollapsedHistorical
```

A package may be published only because a supported product needs a
registry-resolvable transitive dependency. That does not make the library a
supported direct dependency. No package is published merely to reserve a name.

## Serialized identity law

Cross-product and release artifacts retain independently:

```text
product/tool version
Cargo package name and version
logical architecture ID
current and target path/container identity where relevant
protocol/schema generation
provider or orchestrator version where applicable
```

A `cargo-allow 0.2.0` release may contain selected `effortless-* 0.1.0`
transitive packages. It must not rewrite those rows to `0.2.0`, infer
compatibility from workspace proximity or include an experimental sibling simply
because the source shares a workspace.

## Atomic migration law

The surviving package rename, physical move and version split are one atomic
pre-publication migration after strict generation-2 authority is selected and
the five package collapses are complete.

The merge commit leaves coherent:

```text
Cargo manifests and Cargo.lock
workspace members, dependency aliases and paths
architecture and package topology authorities
move, shim, parity and cutover records
CI and script package selectors
candidate/local-registry/release-order fixtures
support and release documentation
```

After cutover:

- current-generation validation rejects stale generic shared package names;
- old shared paths are historical-only;
- old proof package names are historical-only;
- no selected package depends on an unpublished or deleted sibling path; and
- package filenames and release subjects use each row's own version.

## Consequences

### Positive

- Registry names identify EffortlessMetrics ownership without polluting Rust
  imports or architecture diagrams.
- cargo-allow can release on `0.2.x` while sibling products remain experimental.
- The physical shared layout and Cargo package names agree.
- Mixed-version candidate graphs become explicit.
- Five scaffolding packages are removed before compatibility/publication debt is
  created.
- A later repository split needs no package identity redesign.

### Negative

- Cargo package selectors differ from dependency aliases.
- Release and candidate tooling must stop assuming one workspace version.
- The atomic migration touches manifests, paths, lockfile, scripts, fixtures and
  retained authority together.

## Rejected alternatives

| Alternative | Why rejected |
| --- | --- |
| Publish generic `repo-*` names | Reads as an ecosystem-wide claim and obscures ownership. |
| Retain generic physical paths permanently | Preserves avoidable path/package identity mismatch. |
| Rename every product package to `effortless-*` | Product-native names are already clear. |
| Keep one workspace version forever | Couples products with different support and compatibility. |
| Publish proof adapter placeholders | Creates permanent registry identities without independent consumers. |
| Make `effortless` a dependency meta-crate | Cargo does not recursively install dependency binaries; the suite manager must perform real orchestration. |

## Required evidence

- strict current/target logical/path/alias/package/lib identity validation;
- current 22-package closure reconciliation against the retained topology;
- completed package-to-module absorption and stale-package negative fixtures;
- clean selected cargo-allow shared closure;
- mixed-version package and isolated local-registry installation evidence;
- no publication side effect during migration.

## Claim boundary

This ADR defines package, path, version-source, publication, support and
package-survival law. It does not prove remaining semantic-owner or dependency-
neutrality convergence, publish packages, authorize cargo-allow 0.2, or authorize
physical repository extraction.
