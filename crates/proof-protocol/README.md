# proof-protocol

Proof plan transport and provider-neutral contracts for three-product extraction (#2588).

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) or downstream cargo-proof products; `proof-protocol` is an internal cargo-proof crate for proof plan DTOs and provider API contracts.

## Claim boundary

Packet 2588-A lands crate scaffold, boundary documentation, parity/ledger registration, and enforced dependency topology. Plan DTO transport and provider API surfaces land in later #2588/#2603 packets.

`proof-protocol` does not execute proof commands, spawn processes, access the network, or depend on intent-model or intent-engine.

## Packet 2588-A

- `proof-protocol::boundary` — claim boundary and upstream topology markers

## Packet 2588-B+

- `proof-protocol::plan_dtos` — portable proof plan command transport
- `proof-protocol::capability_dtos` — provider capability catalog transport
- `proof-protocol::receipt_dtos` — receipt binding transport bound to repo-protocol
- `proof-protocol::contradiction_dtos` — contradiction report transport
- `proof-protocol::phase_gate_dtos` — phase-gate transport
