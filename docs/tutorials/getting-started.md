# Getting Started Tutorial

This tutorial takes a repository from no cargo-allow policy to a CI-ready
source exception ledger. Follow it when you want the first successful local
run and a small, reviewable next step.

## Goal

By the end, you will have:

- a `policy/allow.toml` file,
- a local audit report,
- a no-new check command that can be copied into CI, and
- one saved artifact for review.

## 1. Install the command

Install the published binary when you are using cargo-allow in another
repository:

```bash
cargo install cargo-allow --locked
```

When developing this repository, use the local package instead:

```bash
cargo run -p cargo-allow -- --help
```

The standalone command is the primary interface. `cargo allow ...` exists as
Cargo external subcommand compatibility, but cargo-allow scans source-tree files
directly rather than asking Cargo to describe a build.

## 2. Create the policy file

Start with strict defaults:

```bash
cargo-allow init --strict
```

This creates `policy/allow.toml`. Commit the file so future diffs can compare
policy changes against source changes.

If you are trying the command inside this repository before installing it, run:

```bash
cargo run -p cargo-allow -- init --strict
```

## 3. Run the first audit

Inventory the current source-tree posture:

```bash
cargo-allow audit --format human
```

Save a machine-readable report for reviewers or automation:

```bash
cargo-allow audit --format json --output target/cargo-allow/audit.json
```

The audit reports syntax-visible findings, matching policy entries, lifecycle
status, and policy health. It does not claim that a successful build exists or
that external proof tools have run.

## 4. Decide whether to review now or baseline first

For a small repository, add reviewed entries one at a time:

```bash
cargo-allow add \
  --kind panic \
  --path crates/example/src/lib.rs \
  --line 42 \
  --owner parser \
  --reason "Parser validates the range before slicing" \
  --evidence test:parser_rejects_invalid_text_range \
  --write policy/allow.toml
```

For a larger repository, generate temporary adoption entries into a separate
file and review the diff before replacing the policy:

```bash
cargo-allow propose --write policy/allow.proposed.toml
```

Generated `baseline_debt` entries are adoption scaffolding. They need owners,
reasons, evidence, narrower selectors, and lifecycle review before they become a
healthy ledger.

## 5. Gate future changes

Once the policy represents the current accepted posture, run the no-new gate:

```bash
cargo-allow check --mode no-new
```

Save a Markdown check artifact when you want a CI summary:

```bash
cargo-allow check --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

A passing no-new check means no unreceipted findings were found in the scanned
source-tree inventory. It is not a proof that no exception exists outside the
scanned surface.

## 6. Review PR posture

On a branch with a git base revision, render a PR summary:

```bash
cargo-allow diff --base origin/main \
  --format markdown \
  --output target/cargo-allow/pr-summary.md
```

Use this in pull requests to show source finding changes and policy weakening
or improvement signals together.

## 7. Pick the next cleanup item

Generate a human-readable worklist:

```bash
cargo-allow worklist --format human
```

Start with small, policy-backed items:

```bash
cargo-allow worklist --difficulty small --format human
```

For agent handoff, prefer the JSON artifact:

```bash
cargo-allow worklist --format json --output target/cargo-allow/worklist.json
```

## Next reading

- Use the adoption guide for CI rollout details: [Adopt No-New in CI](../how-to/adopt-no-new.md).
- Use the worklist guide for cleanup routing: [Triage Worklist Items](../how-to/triage-worklist.md).
- Use the command reference for options: [CLI Reference](../reference/cli.md).
- Use the claim-boundary explanation before writing release claims: [Claim Boundaries](../claim-boundaries.md).
