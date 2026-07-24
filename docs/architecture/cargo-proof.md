# cargo-proof

Human projection of the cargo-proof product shell (#2589-B).

## Claim boundary

Packet 2589-B lands thin CLI with identity, help, version, render, exit mapping, and proof-engine plan/dry-run wiring. Process execution and provider adapters land in follow-on packets.

## Commands

- `cargo-proof identity` — product identity frame
- `cargo-proof plan --obligation-plan <toml>` — obligation plan → proof plan via proof-engine
- `cargo-proof dry-run --proof-plan <toml>` — structured argv projection only (never pasteable shell from prose)
