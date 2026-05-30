# Design

cargo-allow is the source-exception ledger for source trees. Its job is to
make retained source exceptions visible, owned, scoped, evidenced, expirable,
diffable, and actionable.

It is not a linter. It does not replace rustc lints, Clippy, cargo-deny,
cargo-vet, cargo-geiger, ripr, unsafe-review, coverage tooling, or local test
suites. Those tools detect adjacent facts. cargo-allow records which source
exceptions a repository permits to remain and why.

cargo-allow scans repository files directly. It may be invoked as `cargo allow`
through Cargo external subcommand compatibility, but the primary command is the
standalone `cargo-allow` binary. Its own policy scan must not require Cargo
project facts, successful compilation, build scripts, proc macro expansion,
rustc, Clippy, dependency policy tools, unsafe-review, ripr, or coverage tools.
Those tools may later provide evidence artifacts; cargo-allow owns the durable
source-exception ledger.

## Product Lane

cargo-allow answers these questions:

- What source-level exceptions exist?
- Why is each exception allowed?
- Who owns the exception?
- What file and code surface does it cover?
- What evidence supports the exception?
- When does it expire or require review?
- Did this PR add, remove, broaden, weaken, or clean up an exception?
- What work should a human or agent do next?

The governed surfaces are:

- unsafe syntax and unsafe declarations.
- panic-family calls and macros, including unwrap, expect, panic, todo,
  unimplemented, unreachable, indexing, and slicing.
- lint suppressions such as `#[allow]`, `#![allow]`, `#[expect]`, and
  `#![expect]`.
- non-Rust tracked files in source trees.
- generated-code and ignored-surface carveouts.
- legacy policy exceptions from bespoke xtasks.

## Current MVP

The current MVP keeps the scanner and policy surface narrow. It provides:

- `cargo-allow audit`
- `cargo-allow check`
- `cargo-allow diff`
- `cargo-allow list`
- `cargo-allow explain`
- `cargo-allow propose`
- `cargo-allow migrate`
- `cargo-allow prune`
- `cargo-allow doctor`

The MVP scanner is source-syntax based and line-oriented. The matcher already
uses structural selectors where the current scanner can provide them:

- finding kind and family.
- path or exact glob.
- AST-like kind.
- container.
- callee, macro name, or lint name.
- symbol and fingerprints.
- normalized snippet hash.
- line and column hints.

Line numbers are hints. They are not durable identity.

`allow-rust` also has a tree-sitter Rust parser foundation. It can parse source
files and report syntax errors without compiling the repository, running
repository build scripts, expanding proc macros, or loading Cargo metadata.
Current Rust scanner lanes use source syntax facts for syntax-visible exception
surfaces, but they do not parse macro token-tree contents as Rust expressions.

Inventory follows the source-tree boundary: explicit `--root`, nearest git root,
then current directory. Cargo manifests and lockfiles are scanned as files, not
used as required build metadata.

## Crate Namespace

`cargo-allow` is the product binary and Cargo external
subcommand-compatible package. First-party library crates use the canonical
`allow-*` namespace.

New scanners, matchers, policy adapters, exporters, report formats, evidence
integrations, fixtures, schema helpers, and other libraries should use
`allow-*` names. Do not split the crate family into `allow-*` for core crates
and `cargo-allow-*` for integrations or plugins; that creates a durable
semantic distinction users cannot reliably interpret.

Use `cargo-allow-*` only for a separately installed user-facing binary or
service, and only after documenting why it is not a normal library crate in the
`allow-*` family. Do not rename published `allow-*` crates for branding
cleanup, and do not create duplicate `cargo-allow-*` wrapper crates around
`allow-*` libraries. Before adding any public crate, justify why it cannot
remain an internal module of an existing `allow-*` crate.

## CLI Organization

The `cargo-allow` binary should stay thin. `crates/cargo-allow/src/main.rs`
owns process entrypoint wiring only; command parsing and dispatch live in
`cli.rs` and the command modules.

Each command module owns one command family's behavior and command-local
helpers. Command tests should live in sibling `*_tests.rs` modules referenced
with `#[path = "..."]` from the command module, so command implementation files
remain reviewable as the product surface grows.

Shared helpers should move out of command modules only when at least two command
families use them. Do not create a new public crate for command organization
unless there is a durable API boundary that cannot remain an internal module.

## Matching Direction

Matching must move toward durable source identity:

- kind and path or glob are required.
- AST kind, container, callee, macro name, lint name, and snippet hash are strong
  selectors.
- line and column are tie-breakers only.
- ambiguity fails closed.
- stale entries are reported instead of silently retained.
- generated baseline debt remains uncomfortable and temporary.

Future matching should detect policy weakening and review-required policy
changes, including:

- exact path changed to broad glob.
- selector precision decreased.
- expiry extended for explicit review.
- owner, reason, classification, or evidence removed.
- baseline debt added or normalized as permanent approval.

## Evidence Model

An allow entry separates the claim from the proof:

- `reason` explains why the exception is acceptable.
- `evidence` points to what supports that reason.

Evidence references may later include:

- `test:`
- `cargo:`
- `ripr:`
- `unsafe-review:`
- `coverage:`
- `doc:`
- `spec:`
- `adr:`
- `issue:`
- `pr:`
- `legacy-policy:`

V1 may validate only local shape and file existence where practical. It must not
claim semantic proof from the presence of a reference, and it must not execute
external evidence tools as part of its own scan.

## Reports

Reports should serve three audiences:

- maintainers need concise current-state inventory and CI failures.
- reviewers need a PR posture diff.
- agents need machine-readable work items with suggested external proof
  commands.

The desired report artifacts are:

- `target/cargo-allow/report.json`
- `target/cargo-allow/receipt.json`
- `target/cargo-allow/pr-summary.md`
- `target/cargo-allow/worklist.json`

All machine-readable formats need explicit schema versions before users or
agents depend on them.

## Non-Goals

cargo-allow must not claim:

- macro-expanded coverage.
- type-aware analysis.
- MIR-level analysis.
- build-aware analysis.
- control-flow or data-flow analysis.
- execution of repository code.
- required Cargo project facts.
- proof that unsafe is sound.
- proof that tests are adequate.
- proof that coverage means semantic correctness.
- dependency policy or third-party audit decisions.

Those claims belong to other tools or to human review. cargo-allow may link to
their receipts as evidence.
