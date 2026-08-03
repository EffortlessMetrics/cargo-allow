# proof-adapter-cargo-allow

Snapshot-bound read-only `cargo-allow` proof provider for three-product extraction (#2567 / #2554).

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) or downstream cargo-proof products; `proof-adapter-cargo-allow` discovers an installed `cargo-allow` binary via public process protocol, advertises reviewed command capabilities from `proof-adapter-command`, and compiles dry-run invocation specs without importing `cargo-allow` private crates.

## Claim boundary

Packet 2567 lands the snapshot-bound read-only provider contract. Packet 2554 lands discovery, process-protocol argv compilation, and `ProofProviderV1` wiring. Process execution remains proof-engine owned.

The provider advertises the read-only `cargo-allow capabilities --format json`
report as `cargo-allow.capabilities.json`. Its reviewed argv is exact, emits no
file writes or network access, and carries the `cargo-allow.sensor-capabilities.v1`
claim boundary without duplicating sensor rows.

`proof-adapter-cargo-allow` does not scan source files, does not invoke Cargo, compile code, or depend on intent or `cargo-allow` application crates.

## Packet 2567 / 2554

- `proof-adapter-cargo-allow::boundary` — claim boundary and upstream topology markers
- `proof-adapter-cargo-allow::provider_contract` — snapshot-bound read-only provider contract (#2567)
- `proof-adapter-cargo-allow::provider_discovery` — public process discovery without workspace target/crates leaks
- `proof-adapter-cargo-allow::process_protocol` — dry-run argv compilation via reviewed command registry
- `proof-adapter-cargo-allow::cargo_allow_provider` — `ProofProviderV1` implementation (#2554)
