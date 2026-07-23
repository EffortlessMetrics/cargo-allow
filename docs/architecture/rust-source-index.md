# rust-source-index

Human projection of the shared structural Rust subject index (#2587).

## Claim boundary

Source-syntax-visible package/target/module/test subject inventory and selector
resolution. PR1 (#2587-A) lands the crate skeleton and parity fixtures over current
`allow-rust::test_subjects` inventory APIs.

Parity fixtures live under `tests/fixtures/rust-source-index/`. Packet 2587-B moves
subject/selector/result DTOs into `rust-source-index::test_subjects` with a publish-safe
`allow-rust` snapshot copy. Packet 2587-C moves discovery logic and retires the duplicate
resolver from `allow-rust`.

## Module surfaces

- `rust-source-index::test_subjects` — structural test inventory (moves from `allow-rust`)

Scanning for cargo-allow source exceptions remains in `allow-rust`.
