# allow-policy-legacy

Part of `cargo-allow`, a direct source-tree exception ledger for Rust
repositories.

## What this crate owns

`allow-policy-legacy` contains adapters for migration from supported older
policy formats into canonical cargo-allow policy entries. It is for replacing
bespoke xtask/TOML allowlists with `policy/allow.toml` while preserving review
intent where possible.

Generated migration output should remain reviewable. Baselines should stay
`baseline_debt` until a human reviews, narrows, evidences, or removes them.

## Who should use it

Most users should use the `cargo-allow migrate` command. Depend on this crate
directly only when building a migration integration around cargo-allow policy
internals.

## Claim boundary

This crate converts source-policy data. It does not scan repositories, compile
code, run Cargo metadata, execute legacy xtasks, run proof tools, or decide that
an exception is safe.

## Stability

This crate is versioned with the cargo-allow workspace. Public APIs may evolve
as migration adapters and canonical policy rendering mature during the 0.x
series.

## Links

- Binary crate: `cargo-allow`
- Product docs: repository README
- Claim boundaries: `docs/claim-boundaries.md`
