# allow-match

Part of `cargo-allow`, a direct source-tree exception ledger for Rust
repositories.

## What this crate owns

`allow-match` evaluates current source findings against policy entries and
classifies outcomes such as matched, new, stale, expired, review due,
ambiguous, invalid selector, baseline debt, and occurrence-limit overage.

Matching is structural where possible. Line and column are hints, not the
primary identity.

## Who should use it

Most users should use the `cargo-allow` binary. Use this crate directly only if
you need cargo-allow's ledger evaluation without invoking the CLI.

## Claim boundary

This crate does not scan files, render user-facing reports, compile code, run
Cargo metadata, execute proof tools, or decide source safety. It evaluates
source-syntax findings against policy selectors and lifecycle rules.

## Stability

This crate is versioned with the cargo-allow workspace. Public APIs may evolve
while the 0.x series hardens matching behavior, ambiguity handling, and
selector precision.

## Links

- Binary crate: `cargo-allow`
- Product docs: repository README
- Claim boundaries: `docs/claim-boundaries.md`
