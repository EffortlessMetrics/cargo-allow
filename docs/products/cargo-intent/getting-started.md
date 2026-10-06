# cargo-intent getting started

`cargo-intent` is the opt-in experimental intent and obligation compiler.
It compiles authored intent and governance declarations into validation
receipts. It is a separate product: installing or running `cargo-allow`
does not install, enable, or imply `cargo-intent`.

## First hour

This guide describes the current source tree. Exact public `0.1.0`
registry observations and checksums are retained in
[#3705](https://github.com/EffortlessMetrics/cargo-allow/issues/3705#issuecomment-5390989059).
Registry visibility does not establish installed-product qualification,
publication provenance, or stable/direct support; those remain incomplete.
These source commands do not qualify the immutable installed `0.1.0` binary.
Run them from the repository root:

```bash
cargo run -p cargo-intent -- identity
cargo run -p cargo-intent -- governance --receipt target/cargo-intent/governance-receipt.json
```

The `governance` command compiles the governance authority (crate
identities, package postures, dependency law, move ledger, extraction
shims, parity references) into a `cargo-intent.governance-receipt.v1`
validation receipt. CI for this repository consumes that receipt; a partial
or failed compile emits a
bounded failure rather than a green receipt.

That governance receipt is not an `intent.obligation-plan.v1` envelope.
The [cargo-proof source guide](../cargo-proof/getting-started.md) requires an
existing intent obligation plan and a separate captured-receipt inventory;
this governance command does not produce either artifact.

Optional integration — delegation — is explicitly configured by the user
in `.allow/compatibility/intent-delegation.toml`. Without that file,
`cargo-allow` never invokes `cargo-intent`.

Claim boundary: cargo-intent performs compiled-graph-aware intent and
governance evaluation over authored declarations. It does not scan
source trees for exceptions, render release notes, execute proof
commands, or prove target-repository behavior.
