# Fuzzing

This repository includes [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) targets under `fuzz/` for parser, scanner, matcher, and report-rendering paths.

## Targets

- `policy_roundtrip` feeds arbitrary TOML-like text into policy parsing, validation, rendering, and reparsing.
- `rust_scan` feeds arbitrary Rust-like text into the tree-sitter wrapper, source scanner, and report renderers.
- `match_report_pipeline` builds small synthesized findings, allow entries, match outcomes, and report contexts to exercise matching and JSON/SARIF/text/Markdown rendering.

## Running locally

Install the fuzzing subcommand once:

```sh
cargo install cargo-fuzz
```

Run a target from the repository root:

```sh
cargo fuzz run policy_roundtrip
cargo fuzz run rust_scan
cargo fuzz run match_report_pipeline
```

For quick smoke checks in CI or before opening a PR, run each target with a short timeout:

```sh
cargo fuzz run policy_roundtrip -- -max_total_time=30
cargo fuzz run rust_scan -- -max_total_time=30
cargo fuzz run match_report_pipeline -- -max_total_time=30
```
