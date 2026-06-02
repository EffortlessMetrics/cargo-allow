# Crates

Most users should install and run the `cargo-allow` binary. The `allow-*`
crates are first-party implementation layers behind that product.

`cargo-allow` scans repository files directly and checks syntax-visible source
exceptions against a policy ledger. These crates share that source-tree
boundary: they do not require Cargo metadata, compilation, rustc, Clippy, build
scripts, proc macro expansion, type analysis, MIR, or proof-tool execution for
cargo-allow's own scan.

## Crate Responsibilities

| Crate | Job | README stance |
| --- | --- | --- |
| `cargo-allow` | CLI and product package | Full user entry |
| `allow-core` | Shared domain model | Reference |
| `allow-policy` | Policy loading, validation, rendering, and evidence diagnostics | Reference and integration |
| `allow-policy-legacy` | Legacy policy migration adapters | How-to pointer |
| `allow-inventory` | Root discovery and file inventory | Reference |
| `allow-files` | Non-Rust and generated-file scanning | Explanation |
| `allow-rust` | Rust source-syntax scanning | Explanation and limits |
| `allow-match` | Finding-to-policy matching and lifecycle outcomes | Reference |
| `allow-diff` | PR posture and policy-diff helpers | Explanation |
| `allow-report` | Reports, receipts, and artifact rendering | Reference |

## Namespace Policy

- `cargo-allow` is the product binary and Cargo external subcommand compatible
  package.
- `allow-*` is the canonical namespace for first-party cargo-allow library
  crates.
- New public library crates should prefer `allow-*` and should justify why the
  API cannot remain an internal module of an existing crate.
- Avoid `cargo-allow-*` unless the crate is itself a separately installed
  user-facing binary or service.
- Do not rename existing published `allow-*` crates for branding cleanup.
- Do not create duplicate `cargo-allow-*` wrapper crates around `allow-*`
  crates.

See [Crate Namespace Policy](crate-namespace.md) for the full naming rationale.

## Product Documentation

- Product README: [../README.md](../README.md)
- Claim boundaries: [claim-boundaries.md](claim-boundaries.md)
- Source exception ledger: [source-exception-ledger.md](source-exception-ledger.md)
- JSON schemas: [schemas/README.md](schemas/README.md)
