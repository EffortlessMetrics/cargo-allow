# proof-adapter-command

Human projection of the cargo-proof command adapter crate (#2603-B).

## Claim boundary

Packet 2603-B lands reviewed command registry, structured invocation specs, cwd/env/io declarations, dry-run projection, and receipt interpretation. Issue/spec prose must never become executable shell. Process execution remains proof-engine owned.

`proof-adapter-command` must not depend on `intent-model` or `intent-engine` (ADR-0002 forbidden edges). `cargo-allow` must not take a production dependency on proof libraries.

Parity fixtures live under `tests/fixtures/proof-adapter-command/`.

## Module surfaces

- `proof-adapter-command::boundary` — claim boundary and upstream topology markers (#2603-B)
- `proof-adapter-command::command_registry` — reviewed command registry transport (#2603-B)
- `proof-adapter-command::command_spec` — structured program/argv invocation specs (#2603-B)
- `proof-adapter-command::dry_run` — dry-run projections without prose-to-shell (#2603-B)
- `proof-adapter-command::receipt_interpretation` — command receipt interpretation (#2603-B)

## Allowed upstream dependencies

```text
proof-adapter-command → proof-provider-api, proof-protocol, repo-protocol
```

## Forbidden dependency edges

```text
proof-adapter-command → intent-model / intent-engine
cargo-allow product → proof-adapter-command
```
