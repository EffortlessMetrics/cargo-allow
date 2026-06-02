# allow-files

Part of `cargo-allow`, a direct source-tree exception ledger for Rust
repositories.

## What this crate owns

`allow-files` identifies source-tree file surfaces that Rust-centric tools often
miss: workflows, scripts, generated files, docs/config, package metadata, and
other tracked non-Rust files.

This crate supports cargo-allow's "no mystery scripts" lane. It reports file
surfaces so policy can require owner, reason, scope, lifecycle, and evidence.

## Who should use it

Most users should use the `cargo-allow` binary. Use this crate directly only if
you are building tooling around cargo-allow's non-Rust and generated-file
inventory.

## Claim boundary

This crate does not execute files, inspect runtime behavior, run shell scripts,
parse CI semantics, or decide whether a file is safe. It classifies
source-tree surfaces for policy governance.

## Stability

This crate is versioned with the cargo-allow workspace. Public APIs may evolve
while the 0.x series hardens file-family classification and report contracts.

## Links

- Binary crate: `cargo-allow`
- Product docs: repository README
- Claim boundaries: `docs/claim-boundaries.md`
