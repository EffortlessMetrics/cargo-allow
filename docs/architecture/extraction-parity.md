# Extraction Parity

Human projection of `policy/extraction-parity.toml` (#2606 / `CARGO-ALLOW-PARITY-0001`).

## Claim boundary

Parity case and stage-receipt contracts plus a deterministic comparison kernel.
The kernel compares adapter-provided canonical observations, rejects stale
source identities, and emits a stable corpus digest. The policy-layer cutover
receipt producer derives stage coverage from proven parity cases and the move
ledger; runtime adapters still supply exact source, reachability, ownership,
and build evidence. CLI generation and CI artifact upload remain separate
slices. The reachability checker distinguishes semantic evaluators from
bounded compatibility, historical, fixture, and generated views.
Linked shim registry: `CARGO-ALLOW-SHIM-REGISTRY-0001`.
