# cargo-proof

Human projection of the cargo-proof product shell (#2589-B).

## Claim boundary

Packet 2589-B lands thin CLI with identity, help, version, render, exit mapping, and proof-engine plan/dry-run wiring. Process execution and provider adapters land in follow-on packets.

## Commands

- `cargo-proof identity` — product identity frame
- `cargo-proof plan --obligation-plan <intent-envelope.json>` — legacy `intent.obligation-plan.v1` envelope → proof plan via proof-engine (#2936)
- `cargo-proof plan --obligation-plan <intent-envelope.json> --receipt-inventory <receipts.json> --output <plan.json>` — selected provider registry + captured receipt inventory → atomic `proof.plan.v2` artifact
- `cargo-proof dry-run --proof-plan <toml>` — structured argv projection only (never pasteable shell from prose)

## Semantic routing

cargo-proof is a shell: it exchanges protocol DTOs with providers and reaches every semantic decision (planning, dry-run projection, currentness, provider behavior) through proof-engine operations. proof-protocol supplies data only (guarded by `semantic_routing_guard_tests`).
The provider-neutral execution kernel in `proof-engine` accepts only reviewed
structured invocation specifications. It clears the inherited environment,
closes stdin, bounds stdout/stderr and wall-clock execution, rejects shell
programs, and records observation status separately from provider semantics.
Provider adapters remain responsible for preparing requests and interpreting
the receipt; the runner never treats process exit as proof satisfaction.
