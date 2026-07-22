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

## Current workspace ownership (report-only)

All current workspace members are owned by the `cargo-allow` product until
extraction PRs land new crates. Planned intent/proof/shared crates are listed as
`planned_crate` entries and are not required to exist yet.

## Validation

```bash
cargo test -p allow-policy product_crates --locked
cargo test -p cargo-allow product_crate_architecture --locked
```

## Claim boundary

Report-only ownership inventory and workspace drift checks for #2580 PR1. Does
not invoke `cargo metadata` or enforce dependency edges yet.
