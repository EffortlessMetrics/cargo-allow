# cargo-intent getting started

`cargo-intent` is the opt-in experimental intent and obligation compiler.
It compiles authored intent and governance declarations into validation
receipts. It is a separate product: installing or running `cargo-allow`
does not install, enable, or imply `cargo-intent`.

## First hour

`cargo-intent` is an experimental `0.1.0` workspace product; it is not
on the published install channel yet. Evaluate it from the source tree:

```bash
cargo run -p cargo-intent -- identity
cargo run -p cargo-intent -- governance --receipt target/cargo-intent/governance-receipt.json
```

The `governance` command compiles the governance authority (crate
identities, package postures, dependency law, move ledger, extraction
shims, parity references) into a `cargo-intent.governance-receipt.v1`
validation receipt with commit and tree identity. CI for this
repository consume that receipt; a partial or failed compile emits a
bounded failure rather than a green receipt.

Optional integration — delegation — is explicitly configured by the user
in `.allow/compatibility/intent-delegation.toml`. Without that file,
`cargo-allow` never invokes `cargo-intent`.

Claim boundary: cargo-intent performs compiled-graph-aware intent and
governance evaluation over authored declarations. It does not scan
source trees for exceptions, render release notes, execute proof
commands, or prove target-repository behavior.
