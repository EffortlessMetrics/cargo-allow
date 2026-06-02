# allow-rust

Part of `cargo-allow`, a direct source-tree exception ledger for Rust
repositories.

## What this crate owns

`allow-rust` scans Rust source text for syntax-visible exception surfaces:
unsafe constructs, unsafe attributes, panic-family calls/macros,
indexing/slicing, and lint suppression attributes.

Findings include structural identity fields so policy matching can be more
stable than line-only baselines.

## Who should use it

Most users should use the `cargo-allow` binary. Use this crate directly only if
you need cargo-allow-compatible Rust source findings for an integration or
scanner experiment.

## Claim boundary

This crate is source-syntax scoped. It does not compile code, expand macros,
evaluate cfg predicates, run Clippy, use type information, inspect MIR, prove
unsafe correctness, or prove test adequacy.

## Stability

This crate is versioned with the cargo-allow workspace. Public APIs and finding
identity fields may evolve while the 0.x series hardens scanner precision.

## Links

- Binary crate: `cargo-allow`
- Product docs: repository README
- Claim boundaries: `docs/claim-boundaries.md`
