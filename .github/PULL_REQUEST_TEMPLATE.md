## Summary

<!-- Briefly describe what changed and why. -->

## Source-exception ledger impact

<!--
Describe any cargo-allow posture changes. If this PR touches unsafe syntax,
panic-family calls, indexing/slicing, lint suppressions, non-Rust tracked files,
generated-code policy, or policy/allow.toml, include the relevant
cargo-allow diff/audit output or explain why it does not apply.
-->

- [ ] No source-exception posture change
- [ ] Adds or changes source findings
- [ ] Removes or narrows source findings
- [ ] Changes policy entries or evidence

## Validation

<!-- List commands run locally or in CI, and note any intentionally skipped checks. -->

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test --workspace`
- [ ] `cargo run -p cargo-allow -- allow diff --base origin/main --format markdown`

## Review notes

<!-- Call out reviewer focus areas, risks, follow-ups, or migration notes. -->
