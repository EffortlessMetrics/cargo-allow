# cargo-allow source exception ledger

This repository contains the repo-ready MVP for `cargo-allow`.

cargo-allow is a direct source-tree exception ledger. It scans repository files
and source syntax, matches findings to `policy/allow.toml`, and reports whether
exceptions are owned, scoped, receipted, stale, expired, ambiguous, or new.

The product boundary is source-tree policy. cargo-allow may be installed as a
Cargo external subcommand, but the primary UX is the standalone `cargo-allow`
binary. `cargo allow ...` remains compatibility syntax for users who invoke it
through Cargo.

The current MVP uses `serde`/`toml` for policy loading and `clap` for the CLI.
The active direction is to remove remaining Cargo-project assumptions from
inventory and treat `Cargo.toml` and `Cargo.lock` as files in the scanned source
tree, not as required build metadata.

## What currently works

```bash
cargo-allow init --strict
cargo-allow audit --format human
cargo-allow audit --format json
cargo-allow audit --kind non-rust --format human
cargo-allow audit --kind non-rust --format markdown --output target/cargo-allow/non-rust-audit.md
cargo-allow audit --kind non-rust --include-untracked
cargo-allow check --compat --kind non-rust --mode no-new
cargo-allow check --compat --kind generated --mode no-new
cargo-allow check --compat --kind executable --mode no-new
cargo-allow check --compat --kind workflow --mode no-new
cargo-allow check --compat --kind dependency-surface --mode no-new
cargo-allow check --compat --kind process --mode no-new
cargo-allow check --compat --kind network --mode no-new
cargo-allow check --mode no-new
cargo-allow propose --write policy/allow.proposed.toml
cargo-allow migrate --repo-policy policy/ --out policy/allow.toml
cargo-allow list
cargo-allow doctor
```

When developing this repository before installing the binary, run the same
subcommands through the local package, for example
`cargo run -p cargo-allow -- allow check --mode no-new`.

## Current claim boundary

The MVP scanner is source-tree and source-syntax based.

It reads files from the scanned inventory. It does **not** compile code, run
build scripts, expand macros, type-check expressions, analyze MIR, inspect build
output, run control-flow or data-flow analysis, or execute repository code.

Reports use this wording intentionally: "No new unreceipted findings were found
in scanned source-tree inventory," not "no unsafe exists" or "no panic exists."

## Implemented crates

| Crate | Status |
|---|---|
| `allow-core` | Core data model, simple glob matching, stable FNV hash, dates |
| `allow-policy` | Canonical `policy/allow.toml` parser, writer, validation |
| `allow-inventory` | Source-tree root and file inventory seams, currently with git-tracked inventory and recursive fallback |
| `allow-files` | Non-Rust/generated-file finding generation with configured generated globs |
| `allow-rust` | Source-syntax scanner for panic, unsafe, lint suppressions, indexing |
| `allow-match` | Structural matcher, lifecycle classification, stale/new/ambiguous statuses |
| `allow-report` | Human, Markdown, JSON report and receipt rendering, including non-Rust file inventory summaries |
| `allow-diff` | Git changed-file helper and lightweight diff wrapper |
| `allow-policy-legacy` | Legacy policy adapters, including shiplog-style non-Rust/file companion allowlists |
| `cargo-allow` | clap-based CLI wiring for init/audit/check/list/explain/propose/doctor/diff |
