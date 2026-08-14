# proof-engine

Human projection of the cargo-proof orchestration engine (#2589-A / #2943).

## Sole semantic authority

proof-engine is the sole selected currentness, cache, aggregate, contradiction, and phase-gate semantic authority of the proof family (#2943 step 8, #3320): currentness against captured receipts, cache decisions, blocking aggregation, contradiction interpretation, phase-gate evaluation, provider registry behavior, and obligation planning. `proof-protocol` is a data/serialization/structural-validation seam; cargo-proof and provider modules reach semantic decisions only through proof-engine operations (guarded by `semantic_routing_guard_tests` in cargo-proof).

## Claim boundary

Packet 2589-A lands engine scaffold: provider registry, captured receipts, obligation planning, currentness, dry-run projection, explicit execution gates, cache, contradiction detection, and phase-gate evaluation. Process execution and the thin `cargo-proof` CLI land in follow-on packets.

Packet 2713 lands `proof-engine::ripr_routing` — route/preflight composition consuming the external proof corpus (#2708) with fail-closed required aggregates. Live ripr-swarm dual-run and local planner deletion remain out of scope.

The absorbed `proof-provider-api` and `proof-adapter-command` crates live on as the `provider_api` and `command_adapter` modules (#2937). Obligation input is the canonical `intent-protocol` obligation plan envelope (#2936); the intent plan digest is load-bearing in plan identity, cache key, and currentness binding (#3316).

## Topology

Allowed upstream: `proof-protocol`, `intent-protocol` (sole obligation input authority), `repo-protocol`, `rust-source-index`.

Required edge: `proof-engine -> intent-protocol` (#2936/#3317, recorded in `policy/product-crates.toml` and `boundary.rs`).

Forbidden: `intent-engine`, `intent-model`; `cargo-allow` product must not depend on `proof-engine`. Reintroduction of a proof-owned obligation model is rejected by `obligation_authority_guard` (#3317).
