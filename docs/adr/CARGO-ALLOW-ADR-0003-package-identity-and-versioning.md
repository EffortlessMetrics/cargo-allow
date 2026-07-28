---
id: CARGO-ALLOW-ADR-0003
kind: adr
status: accepted
owner: repo-infra
created: 2026-07-28
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

# ADR: Package Identity and Independent Version Lines

## Context

The repository now contains the complete ratified 27-crate logical topology for
`cargo-allow`, `cargo-intent`, `cargo-proof`, and their shared substrate. The
first architecture manifests were created while each workspace directory,
workspace dependency key, Cargo package name, and Rust library name happened to
use the same short string.

That coincidence no longer represents the intended registry or release model.
The four shared crates need durable EffortlessMetrics package identities, while
architecture diagrams and Rust imports should remain concise. The three product
families also have different maturity and support posture. One workspace must
not imply one permanent semver line.

Without an explicit identity contract, a package rename can leave Cargo
selectors, release receipts, move-ledger targets, local-registry candidates,
and Rust imports describing different things while every individual file still
looks plausible.

## Decision

Treat the following as distinct, checked identities:

```text
logical_id
workspace_path
workspace_dependency_alias
cargo_package_name
rust_library_name
product_or_shared_owner
package_version
version_source
publication_state
direct_use_support
candidate_membership
```

`logical_id` is the stable architecture and move-ledger identity. Cargo package
and version are registry identities. Rust library name is the import identity.
A workspace dependency alias is local syntax and must not be serialized as
though it were the registry package name.

### Shared package identities

| Logical ID | Workspace path | Cargo package | Rust library |
| --- | --- | --- | --- |
| `repo-protocol` | `crates/repo-protocol` | `effortless-repo-protocol` | `repo_protocol` |
| `repo-snapshot` | `crates/repo-snapshot` | `effortless-repo-snapshot` | `repo_snapshot` |
| `repo-edit` | `crates/repo-edit` | `effortless-repo-edit` | `repo_edit` |
| `rust-source-index` | `crates/rust-source-index` | `effortless-rust-source-index` | `rust_source_index` |

The workspace may retain concise dependency aliases:

```toml
[workspace.dependencies]
repo-protocol = {
  package = "effortless-repo-protocol",
  path = "crates/repo-protocol",
  version = "0.1.0",
}
```

The alias does not reserve or publish the generic package name.

### Product package identities

Product-owned package names remain native to their product families:

```text
allow-* and cargo-allow
intent-* and cargo-intent
proof-* and cargo-proof
```

They are not renamed to `effortless-*`. The `effortless` suite bootstrap is an
external installer/updater/doctor product and is not a dependency of these
packages.

### Initial version posture

Before first publication of any newly extracted package:

| Family | Initial version posture |
| --- | --- |
| cargo-allow product family | retain `0.2.0` for the 0.2 release train |
| shared `effortless-*` packages | explicit `0.1.0`, experimental or transitive-only |
| cargo-intent family | explicit `0.1.0`, experimental |
| cargo-proof family | explicit `0.1.0`, experimental |

Equal `0.1.0` values are a development cohort, not a permanent lockstep
compatibility promise. Every package version is independently represented and
may diverge when its public compatibility surface requires it.

### Publication and support are separate

A package may be published only because a supported product needs a registry
resolvable transitive dependency. That does not make the library a supported
direct dependency.

The checked package topology must distinguish at least:

```text
UnpublishedInternal
RegistryTransitiveOnly
ExperimentalDirect
SupportedDirect
HistoricalCompatibility
```

No empty compatibility package is published under `repo-protocol`,
`repo-snapshot`, `repo-edit`, or `rust-source-index`. No package is published
merely to reserve a name.

### Serialized identity law

Cross-product and release artifacts must retain independently:

```text
product/tool version
Cargo package name and version
logical architecture ID
protocol/schema generation
provider or orchestrator version where applicable
```

A `cargo-allow 0.2.0` release may contain `effortless-* 0.1.0` transitive
packages. It must not rewrite those rows to `0.2.0` or infer compatibility from
workspace proximity.

### Migration law

The package rename and version split are one atomic pre-publication migration.
The merge commit must leave these sources coherent:

```text
Cargo manifests and Cargo.lock
architecture and package topology authorities
CI and script package selectors
candidate and local-registry fixtures
move, shim, parity, and cutover receipts
support and release documentation
```

Generation-1 artifacts remain readable only as historical migration input.
After cutover, current-generation validation rejects stale generic Cargo
package names.

## Consequences

### Positive

- Registry names identify EffortlessMetrics ownership without polluting Rust
  imports or architecture diagrams.
- cargo-allow can release on `0.2.x` while sibling products remain experimental
  `0.1.x` packages.
- Mixed-version package candidates become explicit rather than accidental.
- A later repository split does not require another package identity redesign.

### Negative

- Cargo package selectors differ from workspace dependency aliases.
- Release and candidate tooling must stop assuming one workspace version.
- The migration touches manifests, lockfiles, scripts, fixtures, and retained
  authority in one reviewed change.

### Operational

- Package identity changes require generation-2 architecture validation first.
- No package publication occurs until the selected product closure is clean and
  independently installable.
- Physical repository extraction remains a separate decision after installed
  interop and dogfood evidence.

## Rejected alternatives

| Alternative | Why rejected |
| --- | --- |
| Publish generic `repo-*` names | Reads as an ecosystem-wide claim and obscures ownership. |
| Rename every product crate to `effortless-*` | Product-native names are already clear and do not represent shared substrate. |
| Keep one workspace version forever | Couples support and compatibility across three products with different maturity. |
| Publish compatibility placeholders | Creates permanent registry identities without useful functionality. |
| Make `effortless` a dependency meta-crate | Cargo does not recursively install dependency binaries; the suite bootstrap must perform real orchestration. |

## Required evidence

- strict generation-2 logical/package/lib/path/alias identity validation;
- exact package closure validation for selected feature and target sets;
- stale generic package-name negative fixtures;
- mixed-version package and isolated local-registry installation evidence;
- no publication side effect during the migration PR.

## Claim boundary

This ADR defines package identity, version-source, publication, and support law
for the landed monorepo. It does not perform the rename, prove dependency
neutrality, publish packages, authorize cargo-allow 0.2, or authorize physical
repository extraction.
