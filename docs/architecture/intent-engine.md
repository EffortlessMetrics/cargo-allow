# intent-engine

Human projection of the cargo-intent evaluator crate (#2586).

## Claim boundary

Evaluator packet envelopes for spec-system orchestration. Packet 2586-A lands the evaluator packet transport bound to `intent-protocol` query envelopes.

Graph compilation, precommit evaluation, and workspace assembly remain in `allow-policy` / `cargo-allow` until later #2586 packets.

Parity fixtures live under `tests/fixtures/intent-engine/`.

## Module surfaces

- `intent-engine::evaluator_packet` — evaluator packet envelope (#2586-A)
