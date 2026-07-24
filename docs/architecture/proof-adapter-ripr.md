# proof-adapter-ripr

Human projection of the RIPR grip receipt adapter (#2556).

## Claim boundary

Packet 2556 lands grip receipt validation (#2217), requirement-grip comparison (#2218), and receipt currentness contracts. Authored evidence purpose remains cargo-intent owned. Process execution remains proof-engine owned.

`proof-adapter-ripr` must not depend on `intent-model`, `intent-engine`, or `cargo-allow` private crates (ADR-0002 forbidden edges). `cargo-allow` must not take a production dependency on proof libraries.

Parity fixtures live under `tests/fixtures/proof-adapter-ripr/`.

## Module surfaces

- `proof-adapter-ripr::boundary` — claim boundary and upstream topology markers
- `proof-adapter-ripr::grip_receipt` — `RiprGripReceiptV1` transport and validation (#2217)
- `proof-adapter-ripr::receipt_currentness` — snapshot/subject currentness evaluation
- `proof-adapter-ripr::grip_comparison` — `RequirementGripComparisonV1` (#2218)
- `proof-adapter-ripr::ripr_adapter` — `ProofProviderV1` wiring for captured-receipt mode

## Allowed upstream dependencies

```text
proof-adapter-ripr → proof-provider-api, proof-protocol, repo-protocol
```

## Forbidden dependency edges

```text
proof-adapter-ripr → intent-model / intent-engine / cargo-allow / allow-core
cargo-allow product → proof-adapter-ripr
```
