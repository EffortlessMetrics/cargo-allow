# cargo-allow MVP workspace

This directory is a repo-ready implementation scaffold for `cargo-allow`.

The MVP started dependency-light to keep the first generated repository easy to
test in offline/agent sandboxes. The policy parser now uses `serde`/`toml`, the
CLI uses `clap`, and workspace discovery uses Cargo metadata; later PRs can
replace the source scanner with a richer Rust syntax parser once the product
seams are stable.

## What currently works

```bash
cargo run -p cargo-allow -- allow init --strict
cargo run -p cargo-allow -- allow audit --format human
cargo run -p cargo-allow -- allow audit --format json
cargo run -p cargo-allow -- allow audit --kind non-rust --format human
cargo run -p cargo-allow -- allow audit --kind non-rust --format markdown --output target/cargo-allow/non-rust-audit.md
cargo run -p cargo-allow -- allow audit --kind non-rust --include-untracked
cargo run -p cargo-allow -- allow check --compat --kind non-rust --mode no-new
cargo run -p cargo-allow -- allow check --compat --kind generated --mode no-new
cargo run -p cargo-allow -- allow check --compat --kind executable --mode no-new
cargo run -p cargo-allow -- allow check --mode no-new
cargo run -p cargo-allow -- allow propose --write policy/allow.proposed.toml
cargo run -p cargo-allow -- allow list
cargo run -p cargo-allow -- allow doctor
cargo test --workspace
```

## Current claim boundary

The MVP scanner is source-syntax based.

It does **not** analyze macro expansion, MIR, type information, or rustc semantics. Reports use this wording intentionally: “No unreceipted unsafe syntax was found in scanned Rust source files,” not “no unsafe exists.”

## Implemented crates

| Crate | Status |
|---|---|
| `allow-core` | Core data model, simple glob matching, stable FNV hash, dates |
| `allow-policy` | Canonical `policy/allow.toml` parser, writer, validation |
| `allow-inventory` | Cargo metadata workspace facts and git-tracked file inventory with recursive fallback |
| `allow-files` | Non-Rust/generated-file finding generation with configured generated globs |
| `allow-rust` | Source-syntax scanner for panic, unsafe, lint suppressions, indexing |
| `allow-match` | Structural matcher, lifecycle classification, stale/new/ambiguous statuses |
| `allow-report` | Human, Markdown, JSON report and receipt rendering, including non-Rust file inventory summaries |
| `allow-diff` | Git changed-file helper and lightweight diff wrapper |
| `allow-policy-legacy` | Legacy policy adapters, including shiplog-style non-Rust allowlists |
| `cargo-allow` | clap-based CLI wiring for init/audit/check/list/explain/propose/doctor/diff |
