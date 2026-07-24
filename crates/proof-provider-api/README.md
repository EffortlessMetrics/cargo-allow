# proof-provider-api

Proof provider API contracts and conformance harness for three-product extraction (#2603).

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) or downstream cargo-proof products; `proof-provider-api` defines provider-neutral execution contracts over `proof-protocol` transport.

## Claim boundary

Packet 2603-A lands crate scaffold, provider API surface, fake provider, and conformance harness. Command adapters land in `proof-adapter-command`.

`proof-provider-api` does not spawn processes, access the network, or depend on intent crates.

## Packet 2603-A

- `proof-provider-api::boundary` — claim boundary and upstream topology markers
- `proof-provider-api::provider_api` — provider trait and validation helpers
- `proof-provider-api::fake_provider` — conformance fake provider
- `proof-provider-api::conformance` — provider conformance harness
