# intent-engine

Human projection of the cargo-intent evaluator crate (#2586).

## Claim boundary

Evaluator packet envelopes and workspace authority composition for spec-system orchestration. Packet 2586-A lands evaluator packet transport. Packet 2586-B lands generic workspace composition and authority compile plans.

Graph compilation, precommit evaluation, and paired graph comparison remain in `allow-policy` / `cargo-allow` until later #2586 packets.

Parity fixtures live under `tests/fixtures/intent-engine/`.

## Module surfaces

- `intent-engine::evaluator_packet` — evaluator packet envelope (#2586-A)
- `intent-engine::workspace_compiler` — workspace composition and authority compile plan (#2586-B)
