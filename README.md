# cargo-allow MVP workspace

This directory is a repo-ready implementation scaffold for `cargo-allow`.

It is intentionally dependency-light: the MVP compiles with the Rust standard library only. That keeps the first generated repository easy to test in offline/agent sandboxes. Later PRs can swap the handwritten policy parser and source scanner for `toml`/`serde`, `clap`, and a richer Rust syntax parser once the product seams are stable.

## What currently works

```bash
cargo run -p cargo-allow -- allow init --strict
cargo run -p cargo-allow -- allow audit --format human
cargo run -p cargo-allow -- allow audit --format json
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
| `allow-inventory` | Workspace discovery and git-tracked file inventory with recursive fallback |
| `allow-files` | Non-Rust and generated-file finding generation |
| `allow-rust` | Source-syntax scanner for panic, unsafe, lint suppressions, indexing |
| `allow-match` | Structural matcher, lifecycle classification, stale/new/ambiguous statuses |
| `allow-report` | Human, Markdown, JSON report and receipt rendering |
| `allow-diff` | Git changed-file helper and lightweight diff wrapper |
| `allow-policy-legacy` | Legacy adapter stubs with concrete migration notes |
| `cargo-allow` | Manual CLI wiring for init/audit/check/list/explain/propose/doctor/diff |
