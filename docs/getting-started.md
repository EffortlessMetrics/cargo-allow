# Getting Started

This tutorial gets a repository from "no cargo-allow policy" to a first
source-tree exception check.

`cargo-allow` scans repository files without executing project code. The steps
below do not require Cargo metadata, compilation, rustc, Clippy, build scripts,
proc macro expansion, or proof-tool execution.

## 1. Install

Install the latest published release from crates.io:

```bash
cargo install cargo-allow --locked
```

For a pinned published release:

```bash
cargo install cargo-allow --version 0.1.10 --locked
```

Do not copy release-candidate versions into install commands until they are
published.

## 2. Check Setup

Run from the repository root:

```bash
cargo-allow doctor
```

`doctor` reports the source-tree root, inventory mode, policy path, scanner
limitations, and local evidence-health diagnostics. It does not build the
project.

If you are scanning a source snapshot or running from outside the repository
root, pass the root explicitly:

```bash
cargo-allow doctor --root path/to/source-tree
```

## 3. Audit Current Exceptions

Run:

```bash
cargo-allow audit
```

The audit shows syntax-visible exception surfaces such as unsafe syntax,
panic-family calls/macros, indexing/slicing, lint suppressions, non-Rust tracked
files, stale policy entries, expired entries, broad selectors, baseline debt,
and evidence-health issues.

The claim is scoped to scanned source-tree inventory. It is not a proof that no
exception exists outside the syntax-visible surface cargo-allow scanned.

## 4. Create a Policy

For a small or strict repository, start with a starter policy:

```bash
cargo-allow init --root .
```

For an existing repository with historical exceptions, adopt no-new-debt first:

```bash
cargo-allow propose --write policy/allow.toml
```

Generated baseline entries are intentionally uncomfortable. Treat
`classification = "baseline_debt"` as a queue for review, narrowing, evidence,
or removal. Do not convert generated debt into approval just to pass CI.

## 5. Run the No-New Check

Run:

```bash
cargo-allow check --mode no-new
```

A passing no-new check means:

```text
No new unreceipted findings were found in scanned source-tree inventory.
```

It does not mean the project is safe, buildable, type-checked, or free of all
possible exceptions.

## Minimal Policy Entry

Retained exceptions should be owned, scoped, evidenced, and reviewable:

```toml
[[allow]]
id = "allow-0042"
kind = "panic"
family = "indexing_slicing"
path = "crates/parser/src/span.rs"
owner = "parser"
classification = "validated_span_invariant"
reason = "Parser validates TextRange before slicing."
created = "2026-06-01"
review_after = "2026-09-01"
evidence = [
  "doc:docs/safety/parser-spans.md",
  "test:parser_rejects_invalid_text_range",
]

[allow.selector]
ast_kind = "index_expr"
container = "slice_checked_text_range"
```

Local-file evidence such as `doc:`, `spec:`, `adr:`, `ripr:`,
`unsafe-review:`, and `coverage:` can be checked for presence. Traceability
references such as `test:`, `issue:`, `pr:`, and `legacy-policy:` are reported
without running tools or contacting services.

## Next Workflows

- Explain an entry: `cargo-allow explain allow-0042`
- List policy entries: `cargo-allow list`
- Review a pull request: `cargo-allow diff --base origin/main`
- Generate agent work: `cargo-allow worklist --format json`
- Read claim boundaries: [claim-boundaries.md](claim-boundaries.md)
- Read the ledger model: [source-exception-ledger.md](source-exception-ledger.md)
