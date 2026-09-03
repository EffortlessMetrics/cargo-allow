# proof-adapter-cargo-allow

Human projection of the snapshot-bound read-only cargo-allow proof provider (#2567 / #2554).

## Claim boundary

The cargo-allow binary advertises the transport contract beside its sensor
capability catalog through `cargo-allow capabilities --format json`.
The provider adapter mirrors and validates that contract before selecting a
provider. Packet 2554 lands public process discovery, dry-run argv compilation
via `proof-adapter-command`, and `ProofProviderV1` wiring. Process execution
remains proof-engine owned.

`proof-adapter-cargo-allow` must not depend on `intent-model`, `intent-engine`, or `cargo-allow` private crates (ADR-0002 forbidden edges). `cargo-allow` must not take a production dependency on proof libraries.

Parity fixtures live under `tests/fixtures/proof-adapter-cargo-allow/`.

## Module surfaces

- `proof-adapter-cargo-allow::boundary` — claim boundary and upstream topology markers
- `cargo-allow capabilities --provider-contract --format json` — snapshot-bound read-only provider contract descriptor (#2567). The installed request/receipt endpoint is a subsequent slice; this descriptor does not advertise an executable protocol yet.
- `proof-adapter-cargo-allow::provider_discovery` — public process discovery without workspace leaks
- `proof-adapter-cargo-allow::process_protocol` — dry-run argv compilation via reviewed registry
- `proof-adapter-cargo-allow::cargo_allow_provider` — `ProofProviderV1` implementation (#2554)

## Allowed upstream dependencies

```text
proof-adapter-cargo-allow → proof-provider-api, proof-protocol, proof-adapter-command, repo-protocol
```

## Forbidden dependency edges

```text
proof-adapter-cargo-allow → intent-model / intent-engine / cargo-allow / allow-core
cargo-allow product → proof-adapter-cargo-allow
```
