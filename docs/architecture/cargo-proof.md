# cargo-proof

Human projection of the cargo-proof product shell (#2589-B).

## Claim boundary

Packet 2589-B lands thin CLI with identity, help, version, render, exit mapping, and proof-engine plan/dry-run wiring. Process execution and provider adapters land in follow-on packets.

## Commands

- `cargo-proof identity` — product identity frame
- `cargo-proof plan --obligation-plan <intent-envelope.json>` — `intent.obligation-plan.v1` envelope → proof plan via proof-engine (#2936); the plan frame binds the intent plan digest (#3316)
- `cargo-proof dry-run --proof-plan <toml>` — structured argv projection only (never pasteable shell from prose)

## Semantic routing

cargo-proof is a shell: it exchanges protocol DTOs with providers and reaches every semantic decision (planning, dry-run projection, currentness, provider behavior) through proof-engine operations. proof-protocol supplies data only (guarded by `semantic_routing_guard_tests`).
