## Bug summary

<!-- Describe the bug and the user-visible failure mode. -->

## Fix

<!-- Describe how the fix addresses the root cause. -->

## Regression coverage

<!-- Point to tests, fixtures, or manual reproduction steps that prevent recurrence. -->

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
