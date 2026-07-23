# intent-model

Human projection of the cargo-intent domain model crate (#2584).

## Claim boundary

Spec-system configuration and domain DTOs parsed from source-tree artifacts.
PR1 (#2584-A) lands the crate skeleton and parity fixtures over current
`allow-policy::spec_system` domain APIs.

Parity fixtures live under `tests/fixtures/intent-model/`. Packet 2584-B moves
domain DTOs into `intent-model::spec_system` with a publish-safe `allow-policy`
snapshot copy. Packet 2584-C moves parsing helpers and retires duplicate logic
from `allow-policy`.

## Module surfaces

- `intent-model::spec_system` — spec-system domain types (moves from `allow-policy`)

Graph compilation and evaluator behavior remain in `intent-engine` / `cargo-allow`
until later extraction stages.
