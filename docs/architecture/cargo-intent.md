# cargo-intent

Human projection of the cargo-intent product shell (#2599).

## Claim boundary

Product identity, config entrypoint, renderer framework, and exit mapping. `change status --staged --phase precommit` provides read-only staged posture and obligation skeleton only; authoritative spec-system evaluation remains in cargo-allow until #2601 delegation.

## Module surfaces

- `cargo-intent::identity` — product identity envelope (#2599-A)
- `cargo-intent::config` — config entrypoint (#2599-A)
- `cargo-intent::render` — human/JSON renderer framework (#2599-A)
- `cargo-intent::exit` — process exit mapping (#2599-A)

## Install smoke (#2599-C)

`scripts/intent-candidate-smoke.sh` packages the seven-crate intent stack from
`docs/dogfood/fixtures/release/intent-candidate-crate-set.toml`, including the
neutral Rust source index and authored intent model consumed by
`intent-compiler`. It installs `cargo-intent` from extracted packages outside
the workspace and emits `cargo-allow.intent-candidate-smoke.v1`. The harness
denies workspace `crates/` reads during decisive install and rejects accidental
`target/debug/cargo-intent` usage. It does not invoke proof runners or
`cargo test`.
