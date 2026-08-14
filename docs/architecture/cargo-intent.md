# cargo-intent

Human projection of the cargo-intent product shell (#2599).

## Claim boundary

Product identity, config entrypoint, renderer framework, and exit mapping. `change status --staged --phase precommit` provides read-only staged posture and obligation skeleton only; authoritative spec-system evaluation remains in cargo-allow until #2601 delegation.

## Module surfaces

- `cargo-intent::identity` — product identity envelope (#2599-A)
- `cargo-intent::config` — config entrypoint (#2599-A)
- `cargo-intent::render` — human/JSON renderer framework (#2599-A)
- `cargo-intent::exit` — process exit mapping (#2599-A)
- `cargo-intent::governance` — repository governance receipt operation (#2942 step 4, #3540)

## Governance receipt operation (#2942)

`cargo-intent governance [--receipt <path>]` compiles the live governance
authority (crate identities, package topology, move ledger, shims, parity,
dependency law) through the intent-engine reconciliation and closure
validation operations into a deterministic `cargo-intent.governance-receipt.v1`
with candidate package-row projections. It reads repository files as text
only; no Cargo invocation.

**Consumers** (#2942 step 5): the CI `test` job runs the operation with a
pinned receipt path and uploads `governance-receipt` as an artifact; the
step fails on any blocking finding. cargo-allow's check pipeline and
extraction-parity command keep calling allow-policy validators directly
during the cutover window (#3542): the runtime guards are bounded
adapters recorded in the MOVE-GOV ledger rows, and their deletion is
blocked on #3548 (source-coupling guard migration) and #3469 (cutover
receipts) per the #3543 consumer audit.

## Install smoke (#2599-C)

`scripts/intent-candidate-smoke.sh` packages the seven-crate intent stack from
`docs/dogfood/fixtures/release/intent-candidate-crate-set.toml`, including the
neutral Rust source index and authored intent model consumed by
`intent-compiler`. It installs `cargo-intent` from extracted packages outside
the workspace and emits `cargo-allow.intent-candidate-smoke.v1`. The harness
denies workspace `crates/` reads during decisive install and rejects accidental
`target/debug/cargo-intent` usage. It does not invoke proof runners or
`cargo test`.
