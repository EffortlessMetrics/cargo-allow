# cargo-proof

Exact-snapshot evidence orchestration shell (#2589-B).

Most users should use `cargo proof` through the cargo subcommand alias; this crate is the cargo-proof product shell.

## Claim boundary

Product identity, config entrypoint, renderer framework, process exit mapping, and proof-engine plan/dry-run wiring only. Process execution and provider adapters land in follow-on packets.

Dependency boundary: the `intent-protocol` dependency is the accepted public
obligation-transport seam consumed through proof-engine (#3310). It is a public
protocol edge, not cargo-intent application coupling: no cargo-intent
application crate (`intent-model`, `intent-engine`/`intent-compiler`,
`intent-edit`, `cargo-intent`) and no cargo-allow product crate is a
dependency. This records the current dependency graph only; it does not
stabilize sibling APIs or promote experimental provider support. Canonical
dependency direction lives in
[ADR-0002](../../docs/adr/CARGO-ALLOW-ADR-0002-three-product-ownership.md).

## PR1 (#2589-B)

Thin binary with `--help` / `--version`, product identity, human/JSON renderer framework, process exit mapping, and `plan` / `dry-run` commands wired to proof-engine surfaces.
