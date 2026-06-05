## Summary

<!-- Briefly describe what changed and why. -->

## Scope

- [ ] One behavior / one seam / one policy slice
- [ ] No unrelated cleanup
- [ ] Generated receipts updated if needed

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

## Policy

- [ ] No new panic-family calls without a receipt
- [ ] No bare `#[allow(clippy::...)]`
- [ ] Any `#[expect(...)]` has a policy-backed reason
- [ ] Non-Rust/source exceptions are receipted through cargo-allow or policy TOML
- [ ] Unsafe changes have unsafe-review evidence or follow-up

## CI economics

- Estimated default PR LEM:
- New default PR lanes:
- New label/main/nightly lanes:
- Expensive runners:
- Cache behavior:
- Does this affect branch protection?

## Validation

<!-- List commands run locally or in CI, and note any intentionally skipped checks. -->

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cargo run -p cargo-allow -- diff --base origin/main --format markdown`
- [ ] `cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md`

## Claim boundary

What this PR proves:
What it does not prove:
Follow-ups:

## Review notes

<!-- Call out reviewer focus areas, risks, follow-ups, or migration notes. -->
