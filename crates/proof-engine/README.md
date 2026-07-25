# proof-engine

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) or downstream cargo-proof products; `proof-engine` orchestrates provider registry, captured receipts, obligation planning, currentness, dry-run projection, explicit execution gates, cache, contradiction detection, and phase gates. It does not scan source files, does not invoke Cargo, compile code, execute repository code, spawn processes, or depend on intent crates.

## Claim boundary

Packet 2589-A lands engine scaffold and orchestration contracts only. Process execution and the thin `cargo-proof` CLI land in follow-on packets.

## Packet 2713

- `proof-engine::ripr_routing` — route/preflight composition consuming the external RIPR proof corpus (#2708) with stable claim IDs and fail-closed required aggregates; does not execute live ripr-swarm providers
