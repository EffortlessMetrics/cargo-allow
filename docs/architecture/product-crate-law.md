# Product Crate Law

Human projection of `policy/product-crates.toml` (#2580). The manifest is the
canonical machine source for product/crate ownership and forbidden cross-product
dependency law during extraction.

## Authority

| Artifact | Role |
| --- | --- |
| `policy/product-crates.toml` | Canonical architecture manifest |
| `policy/product-move-ledger.toml` | Move/deletion inventory (#2598) |
| `docs/adr/CARGO-ALLOW-ADR-0002-three-product-ownership.md` | Product ownership ADR |

## Current workspace ownership

Products, shared crates, and planned crates are declared in the manifest. The
validator loads declared workspace dependency edges from each member `Cargo.toml`
without invoking `cargo metadata`, then checks direct edges against product
`forbid_product_dependencies`, `forbidden_crate_dependency`, and shared
`allowed_domain_dependencies`.

## Validation

```bash
cargo test -p allow-policy product_crates --locked
cargo test -p cargo-allow product_crate_architecture --locked
```

## Claim boundary

PR2 (#2580): ownership inventory, workspace drift checks, and workspace
`Cargo.toml` dependency-law diagnostics for forbidden product edges, explicit
forbidden crate edges, and shared-protocol domain leaks. Dev/build dependency
bypasses remain visible in diagnostics. Does not invoke `cargo metadata` or
enforce no-new blocking, transitional shim registry coupling, or independent
binary-closure proofs yet.
