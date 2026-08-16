# Contributing to cargo-allow

Thank you for helping improve `cargo-allow`. This project is a source-tree
exception ledger for Rust repositories, so contributions should preserve its
core promise: exceptions are visible, durable, reviewable, and removable.

## Before You Start

- Read the [README](README.md) for product scope and current commands.
- Review the [design](docs/design.md) and [claim boundaries](docs/claim-boundaries.md)
  before changing scanner behavior, report wording, or policy semantics.
- Check the [roadmap](docs/roadmap.md) for the preferred PR-sized sequence of
  work.
- Keep changes focused. One pull request should have a clear purpose,
  non-goals, validation plan, claim boundary, and rollback path.

## Community and Triage Surfaces

Project participation is covered by the [Code of Conduct](CODE_OF_CONDUCT.md).
Use the GitHub issue templates for bug reports, feature requests, and policy or
scanner gaps so triage starts from a reproducible source-tree surface. Use the
pull request templates to record source-exception ledger impact, validation,
claim boundaries, and follow-up risks before review.

## Development Setup

This workspace uses Rust 2024 and the workspace `rust-version` declared in
`Cargo.toml`. Install a recent stable Rust toolchain, then run commands from the
repository root. `rust-toolchain.toml` pins the `stable` channel plus the
`rustfmt` and `clippy` components, so `rustup` installs a matching toolchain
automatically the first time you run a `cargo` command here.

That file pins the channel, not the MSRV, so your stable must already be at or
above the declared `rust-version`; an older stable fails the build with a
`rustc … is not supported` error rather than downloading the MSRV for you. CI
proves the MSRV separately in its own job — see [SUPPORT.md](SUPPORT.md).

Useful local commands:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p cargo-allow -- audit --format human
cargo run -p cargo-allow -- check --mode no-new
```

## Changelog

User-facing changes require a Changie fragment. The repository compatibility
contract is pinned to Changie `1.25.2`; install that exact source version and
run `changie new` before merging a PR that changes user-visible behavior:

```bash
go install github.com/miniscruff/changie@v1.25.2
changie new
```

Before review or merge, validate all selected fragments and preview the next
version note without modifying the repository:

```bash
changie batch <next-version> --dry-run
```

Mutating `changie batch` and `changie merge` are not the current release
authority. The existing changelog history has not yet been backfilled into a
complete version-file archive or proven to round-trip exactly. See
[Manage the Changelog](docs/how-to/manage-changelog.md) for the supported
workflow and its claim boundary.

If you have [`just`](https://github.com/casey/just) installed, `just ci` runs
the same checks as the CI workflow (`just --list` shows the individual
recipes). This is an optional convenience; `cargo` remains the source of truth
for every command.

Keep generated build output, cargo-allow review artifacts, backup files, and
proposed policy drafts out of commits unless a PR explicitly promotes them to
reviewed source-tree artifacts.

When developing the repository before installing the binary, invoke the CLI
through the local package:

```bash
cargo run -p cargo-allow -- <subcommand>
```

## Code Organization

- `cargo-allow` is the user-facing CLI package.
- First-party libraries use the `allow-*` crate namespace.
- Do not create a parallel `cargo-allow-*` library namespace for integrations or
  plugins.
- Read the [crate namespace policy](docs/crate-namespace.md) before adding a new
  public crate.
- Keep command tests in sibling `*_tests.rs` modules referenced from the crate
  root so command modules remain reviewable.

## Product Boundaries

`cargo-allow` scans repository files directly. It must not silently expand its
claims to depend on a successful build, Cargo metadata, rustc, Clippy, build
scripts, proc macros, dependency policy tools, unsafe-proof tools, or coverage
systems.

When changing scanners, reports, or schemas:

- State exactly what source-tree surface is scanned.
- Preserve fail-closed behavior for ambiguous policy matches.
- Keep line and column values as review hints rather than identity anchors.
- Avoid wording that claims absence of exceptions outside the scanned surface.
- Update documentation and schemas together when machine-readable contracts
  change.

## Policy and Report Changes

Policy entries are receipts, not suppressions. Changes that affect policy
loading, matching, rendering, or lifecycle classification should include tests
for both the accepted path and the fail-closed path.

For report and artifact changes:

- Keep JSON schemas in `docs/schemas/` synchronized with emitted artifacts.
- Update schema contract tests when adding or renaming fields, enum values, or
  artifact versions.
- Prefer stable vocabulary for automation-facing fields.
- Keep Markdown and HTML reports human-review oriented; use JSON for automation.

## Pull Request Checklist

Before opening a pull request, make sure the PR description includes:

- Purpose: what problem the change solves.
- Non-goals: what the change intentionally does not solve.
- Validation: tests and commands run locally.
- Claim boundary: any scanner, report, schema, or policy claim affected.
- Rollback path: how the change can be reverted or disabled if needed.

Run the standard checks:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

For cheaper iteration on the `cargo-allow` package, use the explicit proof
classes:

```bash
cargo test -p cargo-allow --bins --locked
cargo test -p cargo-allow --tests --locked
```

The first command is the fast package proof for binary-unit, parser, rendering,
schema, and command-unit tests. The second is the contract integration proof
for real-binary, temporary-repository, lifecycle, saved-artifact, and
first-hour targets. The hosted workflow retains both classes and the full
workspace union; neither class is a replacement for the release gate.

If the change affects CLI output or source-tree posture, also run the relevant
local `cargo run -p cargo-allow -- ...` command and include any generated
review artifacts in the PR discussion when useful.

## Current-Head Review and Merge Readiness

Reviewers and agents should follow the canonical
[`review-current-head` skill](.agents/skills/review-current-head/SKILL.md).
The source-exception posture guide in
[`docs/how-to/review-pr-posture.md`](docs/how-to/review-pr-posture.md) is one
review input; it is not a substitute for correctness, architecture,
integration, test-oracle, security, simplification, or release review.

A substantive review must:

- record the exact current head, base SHA/ref, and effective merge base;
- reconstruct the controlling issue/specification and compare it to the actual
  diff;
- inspect every changed file plus relevant owners, callers, consumers, schemas,
  fixtures, docs, packages, and release surfaces;
- inspect existing review threads before posting and avoid duplicate comments;
- verify bot claims against current code and primary contracts;
- post one bounded actionable review rather than streaming generic comments;
- leave repair to one writer.

Any author or repair commit makes the affected review evidence stale. A material
base or merge-base change can also change the effective patch and invalidates
the affected review dimensions. Re-review the new exact pair, verify prior
dispositions against current code, inspect repair- or base-created edge cases,
and inspect exact-pair CI and receipts again. A reviewer who pushes a fix is an
author of the new head and cannot count the old review as independent
verification.

A PR is merge-ready only when the final reviewed head, base SHA/ref, and merge
base still equal the live effective PR pair, the PR is non-draft and mergeable,
substantive conversations are resolved with current-pair evidence, and all
required checks are terminal and green or explicitly not applicable under
repository policy. Pending, cancelled, stale, malformed, action-required, and
silently skipped evidence are not green. Green CI alone is not merge readiness.
Prefer a head-pinned merge and complete the post-merge main, issue, branch, and
worktree reconciliation.

## Documentation

Documentation changes are first-class contributions. Update the README for
user-facing workflows, `docs/README.md` for new documentation entries, and the
specific design or schema document that owns the changed behavior.
