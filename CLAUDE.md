# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Companion instructions

[`AGENTS.md`](AGENTS.md) is the operating contract for agents in this repo
(product boundary, git/PR/release workflow, validation reporting rules, Windows
shell notes). Read it — this file covers build/test mechanics and architecture
and does not repeat it.

## Commands

Rust 2024, MSRV/toolchain pinned to **1.95** (`rust-toolchain.toml`,
`workspace.package.rust-version`, and the `dtolnay/rust-toolchain@1.95.0` pin in
`ci.yml` must all agree — `scripts/check-msrv-consistency.sh` enforces this).
Building requires a C toolchain because of tree-sitter.

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings

cargo test -p cargo-allow --bins --locked     # fast proof: unit tests inside the binary
cargo test -p cargo-allow --tests --locked    # contract proof: crates/cargo-allow/tests/*
cargo test --workspace --locked               # full union
cargo test --doc --workspace
```

`just ci` (optional) runs the same sequence as `.github/workflows/ci.yml`;
`cargo` remains the source of truth. `just --list` shows the recipes.

Single test / narrow runs:

```bash
cargo test -p cargo-allow --bins --locked worklist_priority   # substring filter on unit tests
cargo test -p cargo-allow --test e2e_lifecycle                # one integration binary
cargo test -p cargo-allow --test e2e_lifecycle -- --exact some::test_name
cargo test -p allow-match --locked                            # one library crate
```

Run the CLI from the checkout (do not require an installed binary):

```bash
cargo run -p cargo-allow -- <subcommand>
```

The default final source-tree gate, and the last thing to run before claiming a
change is validated:

```bash
cargo run -p cargo-allow -- check --mode no-new --format markdown \
  --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md
```

User-facing changes need a Changie fragment in `.changes/` (Changie pinned to
`1.25.2`; `changie new`, validate with `changie batch <next> --dry-run`).
Mutating `changie batch`/`merge` are not the release authority — see
`docs/how-to/manage-changelog.md`.

## The repo dogfoods itself

`policy/allow.toml` is this repository's own exception ledger. Adding `unsafe`,
`unwrap`/`expect`/`panic!`, indexing/slicing, or `#[allow]`/`#[expect]` to
scanned source makes `check --mode no-new` fail until a receipt exists. Receipt
it through the plan-then-apply route (`why --plan` → `add --from-plan --update`)
with a real owner, reason, classification, and evidence — never widen policy,
extend expiry, or launder `baseline_debt` to get CI green.

`.allow/config.toml` federates additional ledgers (e.g. the spec-system
doc-artifacts ledger) beyond `policy/allow.toml`.

## Architecture

### Three products in one workspace

The workspace holds three product families plus product-neutral shared
substrate. `docs/architecture/product-crate-law.md` is the human entry point;
the machine manifests under `policy/` are authoritative and are contract-tested.

- **cargo-allow** (`crates/cargo-allow` + `allow-*`) — the shipping product: a
  source-tree exception ledger and policy scanner. It reads repository files
  and never compiles or executes the scanned project.
- **cargo-intent** (`crates/cargo-intent`, `intent-*`) and **cargo-proof**
  (`crates/cargo-proof`, `proof-*`) — independently experimental siblings.
  Their maturity does not gate cargo-allow.
- **shared substrate** (`effortless-repo-*`, `effortless-rust-source-index`) —
  must stay product-neutral: no production dependency on allow/intent/proof
  ontology. This is the architecture law, not a claim that every shared
  package has already reached the target: `effortless-repo-snapshot` remains
  transitional because its source-view layer still depends on cargo-allow
  inventory and error types.

The cargo-allow, cargo-intent, cargo-proof, and shared packages use separate
version lines. Candidate and package proofs are mixed-version aware, so a
package's published version is not inferred from the workspace root version.
The proof adapters have been absorbed into `cargo-proof`; its provider modules
are now the authority for those integrations.

Dependency direction is law, not style. `cargo-allow` takes no intent/proof
library dependency (compatibility goes through an *installed* `cargo-intent`
binary). Temporary reverse edges must be recorded in
`policy/extraction-shims.toml` with move/parity/expiry records — they are
visible non-final state, never suppressed. Governing manifests:

| Question | File |
| --- | --- |
| Package/version/publication posture | `policy/product-package-topology*.toml` |
| Crate ownership + move disposition | `policy/product-crates*.toml`, `policy/product-move-ledger.toml` |
| Temporary compatibility edges | `policy/extraction-shims.toml` |
| Parity / cutover evidence | `policy/extraction-parity.toml` |

These are enforced by `*_parity_tests.rs`, `product_crate_architecture_tests.rs`,
`product_move_ledger_tests.rs`, and `product_package_topology_tests.rs` inside
`crates/cargo-allow/src/`. Moving or renaming a crate means updating the
manifest, not just the path.

### cargo-allow pipeline

```text
allow-inventory   root discovery + git-tracked file inventory (fs fallback)
  -> allow-rust   tree-sitter source-syntax scan (unsafe, panic family,
                  indexing/slicing, lint suppressions)
  -> allow-files  non-Rust / generated / governed-surface scan
  -> allow-match  finding <-> policy receipt matching, lifecycle outcomes
  -> allow-diff   PR posture: new/removed/broadened/weakened/improved
  -> allow-report reports, receipts, artifact rendering
```

`allow-core` is the shared domain model (including `CargoAllowError` /
`CargoAllowErrorKind`); `allow-policy` owns policy loading, validation,
rendering, and evidence diagnostics; `allow-policy-legacy` holds migration
adapters for legacy xtask/TOML allowlists.

Fail-closed is the default for ambiguous policy matches. Line/column values are
review hints, not identity anchors — identity comes from the selector
(`ast_kind`, `container`, `callee`, `normalized_snippet_hash`).

### CLI shape

`crates/cargo-allow/src/main.rs` is a binary whose command surface is split
into `<cmd>.rs`, `<cmd>_args.rs`, `<cmd>_render.rs`,
`<cmd>_types.rs`, and `<cmd>_tests.rs`. Unit tests live in sibling `*_tests.rs`
files wired from the crate root under `#[cfg(test)]` (a project convention —
keep it), so command modules stay reviewable. Integration/contract tests live in
`crates/cargo-allow/tests/` and drive the real binary against temporary
repositories.

Exit codes are mapped **by error kind only, never by message text**
(`exit_code.rs`): `Usage` → 2, everything else → 1; policy-gate failures in
`check`/`diff` exit 1 from their own handlers. Prefer
`CargoAllowError::with_kind(...)` over untyped construction — there is an
in-flight migration away from `CargoAllowError::new()`.

## Conventions that bite

- **Claim discipline.** Reports may say "no new unreceipted findings were found
  in scanned source-tree inventory" — never that no exception exists. Do not add
  wording, docs, or code implying macro expansion, type/MIR analysis,
  control-/data-flow, build awareness, unsafe proof, or coverage proof.
- **Schemas move with artifacts.** JSON emitted by any command has a schema in
  `docs/schemas/` and conformance tests (`artifact_schema_*_tests.rs`,
  `tests/schema_conformance.rs`). Add/rename a field, enum value, or artifact
  version → update schema, tests, and docs in the same change.
- **README and docs are tested.** `readme_tests.rs`, `reference.rs`, and
  `docs/support-matrix.toml` are cross-checked against
  `[package.metadata.cargo-allow.reference]` in `crates/cargo-allow/Cargo.toml`.
  Version and install-command edits must stay synchronized. `cargo test -p
  cargo-allow readme` is the quick check.
- **Crate namespace.** First-party libraries use `allow-*`; never create a
  parallel `cargo-allow-*` library namespace. See `docs/crate-namespace.md`.
- **Scripts are contract-tested.** Every `scripts/foo.sh` used by CI has a
  `scripts/test-foo.sh` characterization guard. Change the script, run its test.
- Keep `target/`, cargo-allow review artifacts, and proposed policy drafts out
  of commits.

## Docs map

`docs/design.md` and `docs/claim-boundaries.md` before changing scanner
behavior, report wording, or policy semantics. `docs/specs/` holds numbered
normative specs, `docs/adr/` the architecture decisions, `docs/roadmap.md` the
preferred PR-sized sequence, and `docs/architecture/` one page per crate.
Reviews follow `.agents/skills/review-current-head/SKILL.md`.
