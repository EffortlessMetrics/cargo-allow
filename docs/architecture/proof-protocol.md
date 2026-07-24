# proof-protocol

Human projection of the cargo-proof protocol crate (#2588).

## Claim boundary

Packet 2588-A lands crate scaffold, boundary documentation, parity/ledger registration, and enforced dependency topology. `proof-protocol` must not depend on `intent-model` or `intent-engine` (ADR-0002 forbidden edges). `cargo-allow` must not take a production dependency on proof libraries.

Plan DTO transport and proof-provider-api surfaces land in later #2588/#2603 packets.

Parity fixtures live under `tests/fixtures/proof-protocol/`.

## Module surfaces

- `proof-protocol::boundary` — claim boundary and upstream topology markers (#2588-A)

## Allowed upstream dependencies

```text
proof-protocol → repo-protocol
```

## Forbidden dependency edges

```text
proof-protocol → intent-model / intent-engine
cargo-allow product → proof-protocol
```
