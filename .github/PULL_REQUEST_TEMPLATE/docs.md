## Documentation summary

<!-- Describe the documentation change and target audience. -->

## Accuracy checks

<!-- Note commands, examples, links, schemas, or screenshots verified. -->

## Source-exception ledger impact

- [ ] No source-exception posture change
- [ ] Adds or changes source findings
- [ ] Removes or narrows source findings
- [ ] Changes policy entries or evidence

<!-- Include cargo-allow diff/audit output when applicable. -->

## Validation

- [ ] Documentation reviewed for current command names and claim boundaries
- [ ] Links or referenced paths checked
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` when code is affected
- [ ] `cargo run -p cargo-allow -- diff --base origin/main --format markdown --require-change-note` when applicable
