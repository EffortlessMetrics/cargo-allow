# allow-core

Part of `cargo-allow`, a direct source-tree exception ledger for Rust
repositories.

## What this crate owns

`allow-core` defines the shared domain model used across the workspace:
findings, allow entries, selectors, lifecycle fields, structural identity,
match status, dates, diagnostics, errors, and path/snippet helpers.

It is intentionally small and model-oriented. It does not scan files, parse
policies, match findings to policy, render reports, or run repository code.

## Who should use it

Most users should use the `cargo-allow` binary. Depend on this crate directly
only when building tooling that needs to read, transform, or produce
cargo-allow-compatible source exception data.

## Claim boundary

This crate participates in source-tree/source-syntax governance. It does not
compile repositories, run Clippy, execute build scripts, expand proc macros,
use type information, inspect MIR, or prove correctness.

## Stability

This crate is versioned with the cargo-allow workspace. Public APIs may evolve
while the 0.x series hardens the policy, scanner, and artifact contracts.

## Links

- Binary crate: `cargo-allow`
- Product docs: repository README
- Claim boundaries: `docs/claim-boundaries.md`
