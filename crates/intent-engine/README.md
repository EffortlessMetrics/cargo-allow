# intent-compiler

Intent compilation, workspace composition, graph comparison, phase-obligation, and bounded-query implementation for cargo-intent.

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) or the `cargo-intent` CLI. `intent-compiler` is an experimental internal cargo-intent package, not a separately supported end-user product.

The first-publication package rename preserves the Rust library import `intent_engine` and the current workspace path `crates/intent-engine`; those source identities may move only through a separate reviewed migration.

## Claim boundary

The package owns selected intent compilation and evaluation mechanics. Authored contracts remain in `intent-model`, stable operation envelopes remain in `intent-protocol`, source-bound settlement remains in `intent-edit`, and proof planning or provider execution remains outside this package.

## PR1 (#2586-A)

Crate skeleton with evaluator packet envelope bound to `intent-protocol` query transport.

## PR2 (#2586-B)

Generic workspace composition and authority compile plan replacing hard-coded four-file paths in `cargo-allow::spec_system_workspace`.

## PR3 (#2586-C)

Graph comparison movement taxonomy and phase obligation compile plans. `cargo-allow` retains paired graph comparison and precommit evaluation runtime during the parity window.

## PR4 (#2586-D)

Bounded domain query catalog returning intent-protocol-shaped responses without proof execution.

## PR5 (#2586-E)

Old/new parity corpus across profiles, selectors, staged movement, diagnostics, and exit posture with recorded dispositions.

## PR6+
