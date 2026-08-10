# Extraction Parity

Human projection of `policy/extraction-parity.toml` (#2606 / `CARGO-ALLOW-PARITY-0001`).

## Claim boundary

Parity case and stage-receipt contracts plus a deterministic comparison kernel.
The kernel compares adapter-provided canonical observations, rejects stale
source identities, and emits a stable corpus digest. Surface-specific old/new
adapters and stage cutover receipts remain separate slices.
Linked shim registry: `CARGO-ALLOW-SHIM-REGISTRY-0001`.
