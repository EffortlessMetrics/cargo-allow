# proof-protocol

Human projection of the cargo-proof protocol crate (#2588).

## Claim boundary

Packet 2588-A lands crate scaffold, boundary documentation, parity/ledger registration, and enforced dependency topology. `proof-protocol` must not depend on `intent-model` or `intent-engine` (ADR-0002 forbidden edges). `cargo-allow` must not take a production dependency on proof libraries.

Plan DTO transport lands in #2588-B. Proof-provider-api surfaces land in #2603.

Parity fixtures live under `tests/fixtures/proof-protocol/`.

## Module surfaces

- `proof-protocol::boundary` — claim boundary and upstream topology markers (#2588-A)
- `proof-protocol::plan_dtos` — portable proof plan command transport (#2588-B)
- `proof-protocol::capability_dtos` — provider capability catalog transport (#2588-B)
- `proof-protocol::receipt_dtos` — receipt binding transport (#2588-B)
- `proof-protocol::contradiction_dtos` — contradiction report transport (#2588-B+)
- `proof-protocol::phase_gate_dtos` — phase-gate transport (#2588-B+)

## Allowed upstream dependencies

```text
proof-protocol → repo-protocol
```

## Forbidden dependency edges

```text
proof-protocol → intent-model / intent-engine
cargo-allow product → proof-protocol
```
