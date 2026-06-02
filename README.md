# cargo-allow

No invisible source exceptions.

`cargo-allow` is a source-tree exception ledger for Rust repositories. It scans
repository files without executing project code, then checks syntax-visible
exceptions against `policy/allow.toml`.

It helps answer:

- What exceptions exist?
- Why are they allowed?
- Who owns them?
- What evidence supports them?
- When do they expire or need review?
- Did this PR add, remove, broaden, weaken, or improve anything?

## Why This Exists

Most repositories accumulate exceptions:

- `unsafe`
- `unwrap` / `expect` / `panic!`
- indexing and slicing
- `#[allow]` / `#[expect]`
- generated code
- scripts, workflows, and other non-Rust tracked files

The hard part is not finding one exception. The hard part is keeping retained
exceptions owned, scoped, evidenced, reviewable, and difficult to silently
broaden.

`cargo-allow` is the ledger layer.

## What It Does

`cargo-allow` scans source-tree inventory and compares findings to policy
receipts.

Core workflows:

```bash
cargo-allow audit
cargo-allow check --mode no-new
cargo-allow diff --base origin/main
cargo-allow explain allow-0042
cargo-allow list
cargo-allow worklist --format json
cargo-allow doctor
```

## What It Does Not Claim

`cargo-allow` scans repository files directly. It may be installed as a Cargo
external subcommand, but the primary UX is the standalone `cargo-allow` binary.
`cargo allow ...` remains compatibility syntax for users who invoke it through
Cargo.

`cargo-allow` does not compile the project or execute repository code.
It does **not** require a successful build and does **not** invoke
Cargo metadata, Cargo commands, rustc, Clippy, build scripts, proc macros,
`cargo-deny`, `cargo-vet`, `ripr`, `unsafe-review`, or coverage tooling.
`Cargo.toml` and `Cargo.lock` are files in the scanned source tree, not required
build metadata.

It does not require:

- Cargo metadata
- `cargo check`
- rustc
- Clippy
- build scripts
- proc macro expansion
- type analysis
- MIR
- control-flow or data-flow analysis
- proof that unsafe code is correct
- proof that tests are adequate

Other tools can provide evidence. `cargo-allow` owns the durable
source-exception ledger.

Current reports may claim:

```text
No new unreceipted findings were found in scanned source-tree inventory.
```

They must not claim that no unsafe, panic, lint suppression, or other exception
exists outside the syntax-visible surface that was scanned.

## Install

```bash
cargo install cargo-allow --locked
```

For a specific published release:

```bash
cargo install cargo-allow --version 0.1.3 --locked
```

Use the latest published version shown on crates.io. Do not copy
release-candidate versions until they are published.

## First Run

From a repository root:

```bash
cargo-allow doctor
cargo-allow audit
```

If no policy exists:

```bash
cargo-allow init --root .
```

To adopt no-new-debt without fixing all history first:

```bash
cargo-allow propose --write policy/allow.toml
cargo-allow check --mode no-new
```

Generated baseline entries are intentionally uncomfortable. They should be
reviewed, narrowed, evidenced, or removed.

## Policy Example

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

## CI

For pull requests:

```bash
cargo-allow diff \
  --base origin/main \
  --format markdown \
  --output target/cargo-allow/pr-summary.md
```

For mainline:

```bash
cargo-allow check \
  --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

Upload `target/cargo-allow/` as a CI artifact, especially on failure.

## Evidence Diagnostics

Evidence references can point to local files or traceability handles.

Locally checked examples:

```text
doc:docs/safety.md
spec:docs/specs/parser.md
adr:docs/adr/0001.md
ripr:target/ripr/span-gap.json
unsafe-review:target/unsafe-review/ffi.json
coverage:target/coverage/receipt.json
```

Traceability-only examples:

```text
test:parser_rejects_invalid_text_range
issue:#123
pr:#456
legacy-policy:no-panic-baseline
```

`cargo-allow` does not run those tools. It classifies what it can see and
reports what is missing.

## Agent Worklists

```bash
cargo-allow worklist --format json
```

Worklist items are intended for bounded human or agent work:

```text
broken_evidence_link
weak_evidence_reference
baseline_debt
stale_allow
broad_scope
unsafe_missing_evidence
new_unreceipted_finding
```

Use the suggested actions and proof commands. Do not suppress findings just to
pass CI.

## Documentation

- Source exception ledger: [docs/source-exception-ledger.md](docs/source-exception-ledger.md)
- Getting started: [docs/getting-started.md](docs/getting-started.md)
- Claim boundaries: [docs/claim-boundaries.md](docs/claim-boundaries.md)
- CI examples: [docs/ci.md](docs/ci.md)
- How-to guides: [docs/how-to/README.md](docs/how-to/README.md)
- JSON schemas: [docs/schemas/README.md](docs/schemas/README.md)
- Agent worklists: [docs/agents/cargo-allow-worklist.md](docs/agents/cargo-allow-worklist.md)
- Migration from xtask: [docs/migration-from-xtask.md](docs/migration-from-xtask.md)
- Crates: [docs/crates.md](docs/crates.md)
- Design notes: [docs/design.md](docs/design.md)
- Roadmap: [docs/roadmap.md](docs/roadmap.md)

## Crates

Most users only need `cargo-allow`.

The workspace uses `allow-*` crates for implementation layers:

- `allow-core`
- `allow-policy`
- `allow-policy-legacy`
- `allow-inventory`
- `allow-files`
- `allow-rust`
- `allow-match`
- `allow-diff`
- `allow-report`

These crates are public because the workspace is split cleanly, but their
primary purpose is supporting `cargo-allow`. See the
[crate responsibility guide](docs/crates.md) and
[crate namespace policy](docs/crate-namespace.md) before adding new public
crates.

## License

MIT OR Apache-2.0
