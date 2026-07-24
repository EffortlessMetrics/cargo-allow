# intent-edit

Human projection of the cargo-intent edit crate (#2613).

## Claim boundary

Packet 2613-A lands crate scaffold, boundary documentation, parity/ledger registration, and enforced dependency topology. `intent-engine` must not depend on `intent-edit` (ADR-0002 forbidden edge). `cargo-allow` must not take a production dependency on intent or proof libraries.

Plan/find-before-create, stable action IDs, dialect adapters, approval/currentness, translate to repo-edit, recompile, and settlement land in later #2613 packets.

Parity fixtures live under `tests/fixtures/intent-edit/`.

## Module surfaces

- `intent-edit::boundary` — claim boundary and upstream topology markers (#2613-A)

## Allowed upstream dependencies

```text
intent-edit → intent-engine, intent-model, intent-protocol,
              repo-protocol, repo-snapshot, repo-edit
```

## Forbidden dependency edges

```text
intent-engine → intent-edit
cargo-allow product → intent-edit
```
