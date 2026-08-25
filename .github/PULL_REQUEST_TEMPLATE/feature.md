## Feature summary

<!-- Describe the new user-facing or maintainer-facing behavior. -->

## Design and compatibility

<!-- Explain key design choices, CLI/output/schema compatibility, and migration concerns. -->

## Source-exception ledger impact

- [ ] No source-exception posture change
- [ ] Adds or changes source findings
- [ ] Removes or narrows source findings
- [ ] Changes policy entries or evidence

<!-- Include cargo-allow diff/audit output when applicable. -->

## Validation

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cargo run -p cargo-allow -- diff --base origin/main --format markdown --require-change-note`

## Follow-ups

<!-- List follow-up work, documentation updates, or release notes needed. -->
