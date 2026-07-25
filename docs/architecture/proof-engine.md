# proof-engine

Human projection of the cargo-proof orchestration engine (#2589-A).

## Claim boundary

Packet 2589-A lands engine scaffold: provider registry, captured receipts, obligation planning, currentness, dry-run projection, explicit execution gates, cache, contradiction detection, and phase-gate evaluation. Process execution and the thin `cargo-proof` CLI land in follow-on packets.

Packet 2713 lands `proof-engine::ripr_routing` — route/preflight composition consuming the external proof corpus (#2708) with fail-closed required aggregates. Live ripr-swarm dual-run and local planner deletion remain out of scope.

## Topology

Allowed upstream: `proof-protocol`, `proof-provider-api`, `proof-adapter-command`, `repo-protocol`.

Forbidden: intent crates; `cargo-allow` product must not depend on `proof-engine`.
