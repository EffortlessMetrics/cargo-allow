# allow-policy

Part of `cargo-allow`, a direct source-tree exception ledger for Rust
repositories.

## What this crate owns

`allow-policy` owns `policy/allow.toml` semantics: loading, validation,
rendering, lifecycle checks, selector checks, baseline-debt rules, evidence
reference parsing, local evidence diagnostics, and policy model conversion.

It is the policy layer, not the scanner layer. It does not inventory source
files, scan Rust syntax, match findings, or execute evidence providers.

## Who should use it

Most users should use the `cargo-allow` binary. Use this crate directly only if
you need to parse, validate, render, or inspect cargo-allow policy files outside
the CLI.

## Claim boundary

Evidence diagnostics are local/source-tree checks only. This crate does not run
Cargo, tests, Clippy, ripr, unsafe-review, coverage tools, build scripts, proc
macros, or network lookups.

## Stability

This crate is versioned with the cargo-allow workspace. Public APIs may evolve
while the 0.x series hardens policy validation, rendering, and evidence
contracts.

## Links

- Binary crate: `cargo-allow`
- Product docs: repository README
- Claim boundaries: `docs/claim-boundaries.md`
