# Product Crate Law

Human entry point for the checked three-product architecture. Machine manifests
remain authoritative; this page explains which source answers which question and
where generation-1 assumptions are intentionally transitional.

## Authority map

| Concern | Canonical authority |
| --- | --- |
| Product ownership and dependency direction | `docs/adr/CARGO-ALLOW-ADR-0002-three-product-ownership.md` |
| Logical ID, Cargo package, Rust library and independent version law | `docs/adr/CARGO-ALLOW-ADR-0003-package-identity-and-versioning.md` |
| Logical crate inventory and roles | `policy/product-crates.toml` and Issue #2612 |
| Package/version/publication/candidate/release/CI posture | `policy/product-package-topology.toml` and Issue #2604 |
| Current source disposition, reachability and deletion output | `policy/product-move-ledger.toml` and Issue #2598 |
| Temporary compatibility | `policy/extraction-shims.toml` and Issue #2607 |
| Parity and cutover evidence | `policy/extraction-parity.toml` and Issue #2606 |
| Remaining convergence and release behavior | `docs/specs/CARGO-ALLOW-SPEC-0011-three-product-convergence.md` |

Human tables and diagrams are projections or contract-checked summaries. They do
not become competing ownership or package manifests.

## Landed topology

The workspace contains the full ratified 27-crate logical topology:

```text
cargo-allow family: 10
shared substrate:    4
cargo-intent family: 5
cargo-proof family:  8
```

“Landed” means the crate/package directory and selected walking-skeleton surface
exist. It does not mean the authority move, dependency-neutrality, support,
publication, or release contract is complete.

## Identity law

Generation 1 overloaded one string for several identities. Generation 2 keeps
these separate:

```text
logical_id
workspace_path
workspace_dependency_alias
cargo_package_name
rust_library_name
package_version and version_source
product_or_shared_owner
publication and direct-use support
candidate and CI membership
```

The shared mapping is:

| Logical ID | Cargo package | Rust library |
| --- | --- | --- |
| `repo-protocol` | `effortless-repo-protocol` | `repo_protocol` |
| `repo-snapshot` | `effortless-repo-snapshot` | `repo_snapshot` |
| `repo-edit` | `effortless-repo-edit` | `repo_edit` |
| `rust-source-index` | `effortless-rust-source-index` | `rust_source_index` |

Current generation-1 manifests and Cargo files still use the generic package
names and shared workspace version. Issue #2884 introduces strict identity and
closure authority; Issue #2885 applies the atomic pre-publication rename and
version split. Until those land, the current state is `LandedTransitional`, not
a clean package graph.

## Dependency law

The final architecture requires:

```text
shared substrate
  no production dependency on allow, intent or proof product-domain crates

cargo-allow
  no intent/proof library dependency
  compatibility through installed cargo-intent process protocol only

intent-engine
  one read-only compiler/query implementation
  no intent-edit, proof execution or cargo-allow private dependency

intent-edit
  intent-engine + repo-edit

proof-engine
  bounded intent-protocol obligations
  no private intent graph or cargo-allow internals
```

Temporary reverse edges are visible non-final state. They require exact move,
shim, parity, expiry and deletion records; they are not suppressed to make a
report green.

## Generation-2 validation

Issue #2884 replaces the generation-1 one-name model with strict checked identity
and selected-closure contracts. Validation must:

- require exact schema IDs and numeric generations;
- reject missing generations and unknown current fields;
- account for every workspace member exactly once;
- resolve dependency aliases to real Cargo package identities;
- model normal/dev/build/target/optional/feature/process edges;
- report shortest direct and transitive violation paths;
- distinguish current, transitional, expired, unpublished and malformed states;
- consume Cargo metadata only as an explicit bounded CI artifact, never during
  ordinary cargo-allow source scans; and
- keep generation-1 input readable only as historical migration evidence.

## Package and release closure

The cargo-allow 0.2 candidate is not the ambient 27-crate workspace. It is the
exact package/version/feature closure selected by the generation-2 package
topology, likely combining cargo-allow `0.2.x` packages with selected
`effortless-* 0.1.x` transitive packages.

Cargo-intent and cargo-proof remain independent experimental `0.1.x` families
until their own exact product qualification. Their incomplete product maturity
does not block cargo-allow core unless the selected cargo-allow support matrix
explicitly includes that compatibility surface.

## Validation commands

Generation-1 checks remain useful while the V2 migration is open:

```bash
cargo test -p allow-policy product_crates --locked
cargo test -p cargo-allow product_crate_architecture --locked
```

Generation-2 acceptance additionally requires the strict identity/closure
fixtures, current-workspace V2 manifests, and no-new enforcement described in
Issue #2884.

## Claim boundary

This page explains current and target architecture authority. It does not make
the generation-1 manifests V2, rename packages, prove dependency neutrality,
authorize cargo-allow 0.2, promote sibling products, or authorize physical
repository extraction.
