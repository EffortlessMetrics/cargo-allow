# cargo-allow

No invisible exceptions.

`cargo-allow` is a source-tree exception ledger for Rust repositories. It scans
repository files and source syntax, matches exception findings to
`policy/allow.toml`, and reports whether each retained exception is owned,
scoped, evidenced, current, stale, expired, ambiguous, or new.

It is not a linter, compiler wrapper, dependency policy tool, unsafe proof
system, or coverage tool. Its job is governance: make allowed source-level
exceptions visible, durable, reviewable, and removable.

## What It Answers

- What source-level exceptions exist in this repository?
- Why is each retained exception allowed?
- Who owns it?
- What exact source/file surface does it cover?
- What evidence supports it?
- When does it expire or require review?
- Did this PR add, remove, broaden, weaken, or clean up exceptions?
- What work should humans or agents do next?

## Source-Tree Boundary

`cargo-allow` scans repository files directly. It may be installed as a Cargo
external subcommand, but the primary UX is the standalone `cargo-allow` binary.
`cargo allow ...` remains compatibility syntax for users who invoke it through
Cargo.

The scanner does **not** require a successful build and does **not** invoke
Cargo metadata, Cargo commands, rustc, Clippy, build scripts, proc macros,
`cargo-deny`, `cargo-vet`, `ripr`, `unsafe-review`, or coverage tooling.
`Cargo.toml` and `Cargo.lock` are files in the scanned source tree, not required
build metadata.

Current reports may claim:

```text
No new unreceipted findings were found in scanned source-tree inventory.
```

They must not claim that no unsafe, panic, lint suppression, or other exception
exists outside the syntax-visible surface that was scanned.

## Quickstart

Install from crates.io:

```bash
cargo install cargo-allow --locked
```

Create a policy file:

```bash
cargo-allow init --strict
```

Inventory the current source-tree exception posture:

```bash
cargo-allow audit --format human
cargo-allow audit --format json --output target/cargo-allow/audit.json
cargo-allow audit --format markdown --output target/cargo-allow/audit.md
cargo-allow audit --format html --output target/cargo-allow/audit.html
```

Gate CI against the current policy:

```bash
cargo-allow check --mode no-new
cargo-allow check --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

Review PR posture:

```bash
cargo-allow diff --base origin/main \
  --format markdown \
  --output target/cargo-allow/pr-summary.md
```

Explain one retained exception:

```bash
cargo-allow explain allow-0042
cargo-allow explain allow-0042 --format json --output target/cargo-allow/explain.json
```

Generate an agent-safe worklist:

```bash
cargo-allow worklist --format json --output target/cargo-allow/worklist.json
cargo-allow worklist --difficulty small --format human
cargo-allow worklist --family unwrap --format human
cargo-allow worklist --item-kind stale_allow --format human
cargo-allow worklist --status baseline_debt --format human
cargo-allow worklist --allow-id allow-0042 --format human
cargo-allow worklist --path crates/allow-core --format human
cargo-allow worklist --source-package allow-core --format human
cargo-allow worklist --owner unowned --classification baseline_debt --format human
cargo-allow worklist --baseline-debt --format human
cargo-allow worklist --broad-scope --format human
cargo-allow worklist --missing-evidence --format human
```

When developing this repository before installing the binary, run the same
subcommands through the local package:

```bash
cargo run -p cargo-allow -- allow check --mode no-new
```

## Governed Surfaces

The current implementation inventories these source-tree surfaces:

- unsafe syntax
- panic-family calls and macros
- indexing and slicing syntax
- lint suppressions
- non-Rust tracked files
- generated-code policy
- legacy policy exceptions

Every retained exception should carry owner, reason, classification, lifecycle,
scope, and evidence. Generated baselines are temporary `baseline_debt`, not a
claim of cleanliness.

## Policy Model

A retained exception is a receipt, not a suppression. A mature entry answers:

```toml
[[allow]]
id = "allow-0042"
kind = "panic"
family = "indexing_slicing"
path = "crates/parser/src/span.rs"
owner = "parser"
classification = "validated_span_invariant"
reason = "Parser validates the text range before slicing."
created = "2026-05-26"
review_after = "2026-08-01"
expires = "2026-11-01"
evidence = [
  "test:parser_rejects_invalid_text_range",
  "doc:docs/specs/parser-span-invariants.md",
]

[allow.selector]
ast_kind = "index_expr"
container = "slice_checked_text_range"
symbol = "source[range]"
normalized_snippet_hash = "fnv1a64:..."
```

Matching is structural first: kind and path plus selector fields such as AST
kind, container, callee, macro, lint, and snippet hash. Line and column values
are hints only. Ambiguous matches fail closed.

## Common Commands

```bash
cargo-allow audit
cargo-allow check --mode no-new
cargo-allow diff --base origin/main
cargo-allow explain allow-0042
cargo-allow explain allow-0042 --format json --output target/cargo-allow/explain.json
cargo-allow list --kind unsafe
cargo-allow list --family unwrap
cargo-allow list --classification baseline_debt
cargo-allow list --path crates/allow-core
cargo-allow list --status baseline_debt
cargo-allow list --broad-scope
cargo-allow list --missing-evidence
cargo-allow list --source-package allow-core
cargo-allow list --format json --output target/cargo-allow/list.json
cargo-allow worklist --baseline-debt --format human
cargo-allow worklist --broad-scope --format human
cargo-allow worklist --missing-evidence --format human
cargo-allow prune --stale --dry-run
cargo-allow prune --stale --format json --output target/cargo-allow/prune.json
cargo-allow add \
  --kind panic \
  --path crates/foo/src/lib.rs \
  --line 42 \
  --owner parser \
  --reason "Parser validates range before slicing" \
  --summary-format json \
  --summary-output target/cargo-allow/add.json
cargo-allow propose --write policy/allow.proposed.toml
cargo-allow propose --write policy/allow.proposed.toml \
  --summary-format json \
  --summary-output target/cargo-allow/propose.json
cargo-allow migrate --repo-policy policy/ --out policy/allow.toml \
  --summary-format json \
  --summary-output target/cargo-allow/migrate.json
cargo-allow doctor
cargo-allow doctor --format json --output target/cargo-allow/doctor.json
```

## Repository Layout

`cargo-allow` is the product binary. First-party implementation libraries use
the `allow-*` crate namespace. Future scanners, matchers, policy adapters,
exporters, report formats, evidence integrations, fixtures, and schema helpers
should stay in that namespace unless they are separately installed
user-facing binaries or services. Do not create a parallel `cargo-allow-*`
library namespace for integrations or plugins. See the
[crate namespace policy](docs/crate-namespace.md) before adding a new public
crate.

| Crate | Role |
|---|---|
| `allow-core` | Core data model, simple glob matching, stable FNV hash, dates |
| `allow-policy` | Canonical `policy/allow.toml` parser, writer, validation |
| `allow-inventory` | Source-tree root discovery and file inventory |
| `allow-files` | Non-Rust and generated-file finding generation |
| `allow-rust` | Source-syntax scanner for panic, unsafe, lint suppressions, indexing |
| `allow-match` | Structural matcher, lifecycle classification, stale/new/ambiguous statuses |
| `allow-report` | Human, Markdown, JSON, SARIF, HTML report and receipt rendering |
| `allow-diff` | Git changed-file helper and lightweight diff wrapper |
| `allow-policy-legacy` | Legacy policy adapters |
| `cargo-allow` | clap-based CLI wiring |

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Documentation

- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Contributing](CONTRIBUTING.md)
- [Design](docs/design.md)
- [Claim boundaries](docs/claim-boundaries.md)
- [Crate namespace policy](docs/crate-namespace.md)
- [Roadmap](docs/roadmap.md)
- [Source exception ledger](docs/source-exception-ledger.md)
- [Migration from xtask](docs/migration-from-xtask.md)
- [CI examples](docs/ci.md)
- [Examples](examples/README.md)
- [Agent worklist prompt](docs/agents/cargo-allow-worklist.md)
- [JSON schema index](docs/schemas/README.md)
- [Add JSON schema](docs/schemas/add.schema.json)
- [Migrate JSON schema](docs/schemas/migrate.schema.json)
- [Explain JSON schema](docs/schemas/explain.schema.json)
- [List JSON schema](docs/schemas/list.schema.json)
