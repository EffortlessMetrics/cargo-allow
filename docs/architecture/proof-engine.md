# proof-engine

Human projection of the cargo-proof orchestration engine (#2589-A).

## Claim boundary

Packet 2589-A lands engine scaffold: provider registry, captured receipts, obligation planning, currentness, dry-run projection, explicit execution gates, cache, contradiction detection, and phase-gate evaluation. Process execution and the thin `cargo-proof` CLI land in follow-on packets.

## Topology

Allowed upstream: `proof-protocol`, `proof-provider-api`, `proof-adapter-command`, `repo-protocol`.

Forbidden: intent crates; `cargo-allow` product must not depend on `proof-engine`.
