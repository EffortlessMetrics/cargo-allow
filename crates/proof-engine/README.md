# proof-orchestrator

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) or the `cargo-proof` CLI. `proof-orchestrator` is the experimental provider-neutral package for obligation planning, currentness, captured-receipt validation, cache identity, contradiction detection, and gate composition.

The first-publication package rename preserves the Rust library import `proof_engine` and the current workspace path `crates/proof-engine`; those source identities may move only through a separate reviewed migration.

## Claim boundary

The package does not scan source files, invoke Cargo, or compile repository code. Planning remains non-executing; its bounded provider-neutral observation kernel is an explicit low-level process boundary, while provider-specific application wiring and semantic receipts remain in `cargo-proof`/`proof-protocol`.

## Packet 2713

- `proof_engine::ripr_routing` — route/preflight composition consuming the external RIPR proof corpus (#2708) with stable claim IDs and fail-closed required aggregates; does not execute live ripr-swarm providers
