# Product Crate Law

Human projection of `policy/product-crates.toml` (#2580). The manifest is the
canonical machine source for product/crate ownership and forbidden cross-product
dependency law during extraction.

## Authority

| Artifact | Role |
| --- | --- |
| `policy/product-crates.toml` | Canonical architecture manifest |
| `policy/product-move-ledger.toml` | Move/deletion inventory (#2598) |
| `policy/product-package-topology.toml` | Package/release family classification (#2604) |
| `docs/adr/CARGO-ALLOW-ADR-0002-three-product-ownership.md` | Product ownership ADR |

## Current workspace ownership

Products, shared crates, and planned crates are declared in the manifest. The
validator loads declared workspace dependency edges from each member `Cargo.toml`
without invoking `cargo metadata`, then checks direct edges against product
`forbid_product_dependencies`, `forbidden_crate_dependency`, and shared
`allowed_domain_dependencies`.

PR3 cross-checks the architecture manifest against the linked move ledger and
package topology so all three denominators agree on crate inventory and product
family classification.

## Validation

```bash
cargo test -p allow-policy product_crates --locked
cargo test -p cargo-allow product_crate_architecture --locked
```

## Claim boundary

PR3 (#2580): full workspace crate family inventory plus denominator cross-checks
against #2598 move-ledger target crates and #2604 package topology families.
Builds on PR2 workspace `Cargo.toml` dependency-law diagnostics. Does not invoke
`cargo metadata`, enforce no-new blocking, transitional shim registry coupling,
or independent binary-closure proofs yet.
