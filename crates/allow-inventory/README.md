# allow-inventory

Part of `cargo-allow`, a direct source-tree exception ledger for Rust
repositories.

## What this crate owns

`allow-inventory` resolves which files cargo-allow should inspect. It supports
explicit roots, nearest git-root discovery, current-directory fallback,
git-tracked file inventory, and symlink-safe filesystem traversal when git
inventory is unavailable.

It treats `Cargo.toml` and `Cargo.lock` as source-tree files to classify, not
as required build metadata.

## Who should use it

Most users should use the `cargo-allow` binary. Use this crate directly only if
you are integrating with cargo-allow internals or need the same source-tree
inventory behavior in another tool.

## Claim boundary

This crate does not compile repositories, run Cargo metadata, execute build
scripts, expand proc macros, parse Rust syntax, or prove anything about source
safety. It only identifies source-tree files for later policy scanning.

## Stability

This crate is versioned with the cargo-allow workspace. Public APIs may evolve
while the 0.x series hardens root discovery and inventory contracts.

## Links

- Binary crate: `cargo-allow`
- Product docs: repository README
- Claim boundaries: `docs/claim-boundaries.md`
