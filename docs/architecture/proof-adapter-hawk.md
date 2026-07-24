# proof-adapter-hawk

Human projection of the Hawk analysis receipt adapter (#2555).

## Claim boundary

Packet 2555 Stage A lands captured Hawk JSON validation, finding-to-adapter result mapping with absence-as-NotProven, source-anchor resolution, and receipt currentness. Process execution and Hawk liveness remain provider-owned.

`proof-adapter-hawk` must not depend on `intent-model`, `intent-engine`, or `cargo-allow` private crates (ADR-0002 forbidden edges). `cargo-allow` must not take a production dependency on proof libraries.

Parity fixtures live under `tests/fixtures/proof-adapter-hawk/`.

## Module surfaces

- `proof-adapter-hawk::boundary` — claim boundary and upstream topology markers
- `proof-adapter-hawk::analysis_receipt` — `HawkAnalysisReceiptV1` transport (#2555 Stage A)
- `proof-adapter-hawk::finding_mapping` — Hawk finding result class preservation
- `proof-adapter-hawk::source_anchor_resolution` — intent anchor to Hawk declaration identity
- `proof-adapter-hawk::receipt_currentness` — toolchain/config/snapshot currentness
- `proof-adapter-hawk::hawk_adapter` — `ProofProviderV1` wiring for captured-report mode

## Allowed upstream dependencies

```text
proof-adapter-hawk → proof-provider-api, proof-protocol, repo-protocol
```

## Forbidden dependency edges

```text
proof-adapter-hawk → intent-model / intent-engine / cargo-allow / allow-core
cargo-allow product → proof-adapter-hawk
```
