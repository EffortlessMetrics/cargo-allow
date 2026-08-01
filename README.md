<p align="center">
  <img src="docs/assets/cargo-allow-checkmark.svg" alt="cargo-allow green checkmark logo" width="96" height="96">
</p>

<h1 align="center">cargo-allow</h1>

<p align="center">
  <a href="https://github.com/EffortlessMetrics/cargo-allow/actions/workflows/ci.yml"><img src="https://github.com/EffortlessMetrics/cargo-allow/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI" /></a>
  <a href="https://github.com/EffortlessMetrics/cargo-allow/releases/tag/v0.1.11"><img src="https://img.shields.io/github/v/release/EffortlessMetrics/cargo-allow?display_name=tag&include_prereleases=false" alt="GitHub release" /></a>
  <a href="https://crates.io/crates/cargo-allow"><img src="https://img.shields.io/crates/v/cargo-allow.svg" alt="crates.io" /></a>
  <a href="https://crates.io/crates/cargo-allow"><img src="https://img.shields.io/crates/d/cargo-allow.svg?label=crates.io%20downloads" alt="crates.io downloads" /></a>
  <a href="https://docs.rs/cargo-allow"><img src="https://docs.rs/cargo-allow/badge.svg" alt="docs.rs" /></a>
</p>

<p align="center">
  <a href="https://doc.rust-lang.org/cargo/reference/manifest.html#the-rust-version-field"><img src="https://img.shields.io/badge/MSRV-1.95-blue.svg" alt="MSRV" /></a>
  <a href="#license"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg" alt="License: MIT OR Apache-2.0" /></a>
</p>

<p align="center">
  <em>Source-tree exception ledger and policy scanner for Rust repositories.</em>
</p>

<!-- cargo-allow badges report repository automation state and package metadata.
They do not prove that no unsafe, panic, lint suppression, generated code, or
non-Rust exception exists outside the scanned source-tree/source-syntax surface.
Current reports may claim only that no new unreceipted findings were found in
scanned source-tree inventory. The GitHub release badge tracks the latest tag;
it does not prove release readiness for unreleased commits. -->

No invisible source exceptions.

`cargo-allow` is a source-tree exception ledger and policy scanner for Rust repositories. It scans
repository files without executing project code, then checks syntax-visible
exceptions against `policy/allow.toml`.

It helps teams answer:

- What source exceptions exist?
- Why are they allowed?
- Who owns them?
- What evidence supports them?
- When do they expire or need review?
- Did this PR add, remove, broaden, weaken, or improve anything?
- What should a human or agent fix next?

## The First Useful Run

The first useful run should feel small:

```text
one repo
-> one visible exception surface
-> one no-new policy
-> one CI receipt
-> one worklist item to close
```

That is the adoption spine: make retained exceptions visible, keep new debt out,
record CI evidence, and give humans or agents a bounded repair queue.

Before the first command, pick a product channel: **Published** `0.1.11`
(`cargo install cargo-allow --version 0.1.11 --locked`) versus **source
candidate** (`cargo run -p cargo-allow -- …` on this checkout). The branched
first-hour journey, install prerequisites, clean-vs-brownfield forks, and
fixture-backed expected outputs live in
[Getting started](docs/getting-started.md). The offline published command
registry is
[`published-command-registry.toml`](docs/dogfood/fixtures/getting-started/published-command-registry.toml).

## The Problem

Most repositories accumulate exceptions:

- `unsafe`
- `unwrap` / `expect` / `panic!`
- indexing and slicing
- `#[allow]` / `#[expect]`
- generated code
- scripts, workflows, docs, config, and other non-Rust tracked files

The hard part is not finding one exception. The hard part is keeping retained
exceptions owned, scoped, evidenced, reviewable, and difficult to silently
broaden.

`cargo-allow` is the ledger layer.

## What cargo-allow Does

`cargo-allow` scans source-tree inventory and compares findings to policy
receipts. The durable policy file is `policy/allow.toml`.

Core workflows (Published `0.1.11` and source candidate):

```bash
cargo-allow doctor                    # validate local setup
cargo-allow audit                     # inventory exceptions and policy health
cargo-allow check --mode no-new       # CI gate for the exception ledger
cargo-allow diff --base origin/main   # PR-oriented report with git changed files
cargo-allow list                      # list allow entries
cargo-allow explain <allow-id>        # explain one allow entry
cargo-allow why --kind panic --path src/lib.rs --line 42  # diagnose an unreceipted finding
cargo-allow add --kind panic --path src/lib.rs --line 42 --update  # receipt a finding
cargo-allow worklist --format json    # actionable work items
```

Lifecycle commands for policy maintenance:

```bash
cargo-allow propose                   # generate temporary baseline_debt entries
cargo-allow refresh --allow-id <id> --write  # update drifted last_seen location
cargo-allow prune --stale             # preview or remove stale allow entries
cargo-allow migrate --from <file>     # convert legacy policy files
```

## What cargo-allow Does Not Claim

`cargo-allow` scans repository files directly. It may be installed as a Cargo
external subcommand, but the primary UX is the standalone `cargo-allow` binary.
`cargo allow ...` remains compatibility syntax for users who invoke it through
Cargo.

`cargo-allow` does not compile the project or execute repository code.
It does **not** require a successful build and does **not** invoke
Cargo metadata, Cargo commands, rustc, Clippy, build scripts, proc macros,
`cargo-deny`, `cargo-vet`, `ripr`, `unsafe-review`, or coverage tooling.
It does not require network access or GitHub APIs for its own scan.
`Cargo.toml` and `Cargo.lock` are files in the scanned source tree, not required
build metadata.

It does not require:

- Cargo metadata
- `cargo check` or `cargo test`
- rustc
- Clippy
- build scripts
- proc macro expansion
- dependency resolution
- type analysis
- MIR
- control-flow or data-flow analysis
- proof that unsafe code is correct
- proof that tests are adequate
- coverage proof
- network access
- GitHub API access

Other tools can provide evidence. `cargo-allow` owns the durable
source-exception ledger.

Current reports may claim:

```text
No new unreceipted findings were found in scanned source-tree inventory.
```

They must not claim that no unsafe, panic, lint suppression, or other exception
exists outside the syntax-visible surface that was scanned.

## Where cargo-allow Fits

`cargo-allow` is a source-syntax policy linter and durable exception ledger.
It is not a compiler wrapper, dependency-policy tool, type-aware analyzer, or
unsafe proof system.

```text
Clippy:
  flags code patterns.

cargo-deny / cargo-vet:
  govern dependency and supply-chain policy.

ripr / unsafe-review / coverage:
  can provide evidence for retained exceptions.

cargo-allow:
  owns the source-exception ledger: what is allowed, why, by whom, where,
  with what evidence, until when, and whether this PR weakened posture.
```

Other tools can provide evidence. `cargo-allow` owns the durable receipt.

## Quick Start

Most users start from the surface they already own.

| User type | First action | Main doc |
| --- | --- | --- |
| Maintainer | Run `cargo-allow doctor`, then `cargo-allow audit`. | [Getting started](docs/getting-started.md) |
| New adopter | Choose the closest path: source exceptions, no-new, spec-system, CI, or cross-repo rollout. | [Onboarding](docs/onboarding.md) |
| CI owner | Add `cargo-allow check --mode no-new` and upload the receipt. | [CI guide](docs/how-to/run-in-ci.md) |
| Reviewer | Run `cargo-allow diff --base origin/main`. | [PR posture](docs/pr-posture.md) |
| Auditor | Run `cargo-allow list`, `cargo-allow explain <id>`, and `cargo-allow why --kind <kind> --path <path> --line <line>` on Published `0.1.11`. | [Explain an allow](docs/how-to/explain-an-allow.md), [Explain why a finding](docs/how-to/explain-why-a-finding.md) |
| Migrator | Run `cargo-allow migrate --repo-policy <dir>`. | [Migration](docs/how-to/migrate-from-xtask.md) |
| Agent operator | Run `cargo-allow worklist --format json`. | [Agent worklists](docs/how-to/feed-agent-worklists.md) |
| Shell user | Generate completions from the installed binary. | [Install shell completions](docs/how-to/install-shell-completions.md) |

## First Run

Install:

```bash
cargo install cargo-allow --locked
```

For a specific published release:

```bash
cargo install cargo-allow --version 0.1.11 --locked
```

Use the latest published version shown on crates.io. Do not copy
release-candidate versions until they are published.

Check setup:

```bash
cargo-allow doctor
```

Inventory current exceptions:

```bash
cargo-allow audit
```

Start strict, for a small repo:

```bash
cargo-allow init --root .
```

Adopt no-new-debt, for an existing repo:

```bash
cargo-allow propose --write policy/allow.toml
cargo-allow check --mode no-new
```

Generated baseline entries are intentionally uncomfortable. Review them, narrow
them, add evidence, or remove them.

To receipt one new finding after that, the supported route is plan-then-apply.
Source candidate only (current `main`), not the Published `0.1.11` surface:

```bash
cargo run -p cargo-allow -- why \
  --kind panic --path src/lib.rs --line 42 \
  --plan target/cargo-allow/add-plan.json

cargo run -p cargo-allow -- add \
  --from-plan target/cargo-allow/add-plan.json \
  --update \
  --owner core --reason "<why this is acceptable>" \
  --evidence doc:docs/design.md
```

`why --plan` is read-only; `add --from-plan --update` re-verifies the plan
against the live tree before one atomic write and refuses a stale plan. Then
recheck that finding with `why`, and prove the repository with
`check --mode no-new`. See
[Manage an exception](docs/how-to/manage-an-exception.md).

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

## Example Review Signal

```text
REVIEW REQUIRED allow-0042

kind: panic
family: indexing_slicing
path: crates/parser/src/span.rs
owner: parser
classification: validated_span_invariant

Evidence:
  ✓ doc:docs/safety/parser-spans.md exists
  ? test:parser_rejects_invalid_text_range not validated offline

Current match:
  ast_kind: index_expr
  container: slice_checked_text_range
  selector precision: high

Claim boundary:
  source-tree/source-syntax only; no macro expansion, type analysis, MIR,
  control-flow, data-flow, or proof-tool execution.
```

A matching policy receipt is intentionally specific:

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
evidence = ["doc:docs/safety/parser-spans.md"]

[allow.selector]
ast_kind = "index_expr"
container = "slice_checked_text_range"
```

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

## Worklists for Humans and Agents

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

## Current Scope

| Surface | Current state |
| --- | --- |
| Source inventory | Git-tracked files first, filesystem fallback when needed. |
| Rust scanning | Source-syntax unsafe, panic-family, indexing/slicing, and lint suppressions. |
| Non-Rust scanning | Tracked scripts, workflows, docs/config, generated files, and other governed surfaces. |
| Policy | `policy/allow.toml` receipts with owner, reason, classification, lifecycle, selector, and evidence. |
| Evidence | Local evidence path checks plus traceability-only references. No proof-tool execution. |
| PR posture | `diff --base ...` reports new, removed, broadened, weakened, and improved exception posture. |
| Worklists | JSON queues for humans and agents to close retained-risk seams. |
| Migration | Legacy policy adapters for replacing bespoke xtask/TOML allowlists. |

## Supporting Docs

| Need | Doc |
| --- | --- |
| Choose an adoption path | [Onboarding](docs/onboarding.md) |
| First hour | [Getting started](docs/getting-started.md) |
| Claim boundaries | [Claim boundaries](docs/claim-boundaries.md) |
| Run in CI | [CI guide](docs/how-to/run-in-ci.md) |
| Explain retained exceptions | [Explain an allow](docs/how-to/explain-an-allow.md) |
| Explain unreceipted findings | [Explain why a finding](docs/how-to/explain-why-a-finding.md) |
| Repair evidence | [Fix broken evidence](docs/how-to/fix-broken-evidence.md) |
| Feed agents work | [Agent worklists](docs/how-to/feed-agent-worklists.md) |
| Migrate legacy policy | [Migration from xtask](docs/how-to/migrate-from-xtask.md) |
| Understand the model | [Source exception ledger](docs/source-exception-ledger.md) |
| Understand PR posture | [PR posture](docs/pr-posture.md) |
| Understand policy weakening | [Policy weakening](docs/policy-weakening.md) |
| JSON artifacts | [JSON schemas](docs/schemas/README.md) |
| Crate responsibilities | [Crates](docs/crates.md) |
| Changelog | [CHANGELOG.md](CHANGELOG.md) |

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

## Development

Use nearby project patterns, keep changes narrow, and validate source-tree
posture before publishing a README or policy change:

```bash
cargo test -p cargo-allow readme
cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md
```

## License

MIT OR Apache-2.0
