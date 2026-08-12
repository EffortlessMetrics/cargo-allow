# Three-Product Generation-2 Reconstruction Fixture

This fixture proves that a fresh builder can reconstruct the current and target
cargo-allow, cargo-intent, cargo-proof, and shared-substrate contract from
retained repository authority rather than issue archaeology.

`disposition-map.toml` is a parsed, contract-checked projection. It is not a
second architecture, package, move, shim, or parity authority.

## Authority entry points

| Question | Current answer |
| --- | --- |
| Why three products and this convergence direction? | `CARGO-ALLOW-PROP-0010` |
| Product ownership | `CARGO-ALLOW-ADR-0002` |
| Package/path/version identity | `CARGO-ALLOW-ADR-0003` |
| Current convergence requirements | `CARGO-ALLOW-SPEC-0011` |
| Historical generation-1 requirements | `CARGO-ALLOW-SPEC-0010` |
| Merge-before-dependent-branch train | `CARGO-ALLOW-PLAN-0010` |
| Logical topology provenance | Issue `#2612` |
| Package topology | Issue `#2604` |
| Move/deletion denominator | Issue `#2598` |
| Shim and parity meaning | Issues `#2607` and `#2606` |
| Exact release authorization | Issues `#2371`, `#2501`, and `#2502` |

## Contract reconstructed

```text
historical maximum scaffold packages = 27
current source packages             = 22
retained topology packages          = 22

cargo-allow   10 current / 10 retained
shared         4 current /  4 retained
cargo-intent   5 current /  5 retained
cargo-proof    3 current /  3 retained
```

The five historical package identities were absorbed into modules under
`proof-engine` or `cargo-proof`. The four shared logical IDs retain concise Rust
imports at `crates/effortless-*` paths and `effortless-*` Cargo package names:

```text
effortless-repo-protocol
effortless-repo-snapshot
effortless-repo-edit
effortless-rust-source-index
```

Generation-2 governance is authored in `intent-model`, reconciled in
`intent-engine`, and exposed through cargo-intent/repository-CI receipts.
`proof-protocol` owns data contracts; `proof-engine` owns semantic currentness,
cache, contradiction, and gate evaluation.

Repository extraction and cargo-allow `0.2.x` release remain unauthorized. A
complete cargo-proof candidate is independent of cargo-allow release
qualification. The exact candidate refreeze remains Issue `#2501`.

## Validation

```bash
cargo test -p allow-policy --test three_product_design --locked -- --nocapture
cargo test -p cargo-allow spec_design_artifact_links --locked -- --nocapture
cargo test -p allow-policy support_tier --locked -- --nocapture
cargo run -p cargo-allow --locked -- check --mode no-new
```

The fixture proves reconstructability only. It does not prove implementation
convergence, package publication, release readiness, or repository extraction.
