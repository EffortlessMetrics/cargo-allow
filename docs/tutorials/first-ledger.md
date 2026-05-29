# Tutorial: Your First Source-Exception Ledger

This tutorial walks through the first cargo-allow loop on a small Rust
repository. It is intentionally learning-oriented: run each command, inspect the
artifact it creates, and then decide what your policy should retain.

By the end, you will have:

- created `policy/allow.toml`;
- inventoried the current source-tree exception posture;
- generated temporary baseline entries for existing findings;
- reviewed the baseline as debt instead of treating it as proof;
- run the CI gate that prevents new unreceipted findings.

## Before You Start

You need a Git-tracked Rust repository and a `cargo-allow` binary. When working
inside this repository before installing the binary, replace `cargo-allow` with:

```bash
cargo run -p cargo-allow --
```

cargo-allow scans the source tree directly. It does not need a successful build,
Cargo metadata, macro expansion, or test execution to produce an inventory.

## 1. Create The Policy File

Start with a strict policy so that future entries need owners, reasons,
classifications, and lifecycle dates:

```bash
cargo-allow init --strict
```

This creates `policy/allow.toml`. Commit that file even if the ledger has no
entries yet, because it documents the governance rules reviewers should expect.

## 2. Inventory The Current Posture

Run an audit and save both human-readable and JSON artifacts:

```bash
cargo-allow audit --format human
cargo-allow audit --format json --output target/cargo-allow/audit.json
```

Read the summary first. New findings mean cargo-allow saw source-tree exception
surfaces that are not yet represented by the ledger. Common examples include
panic-family calls, unsafe syntax, lint suppressions, generated files, and
non-Rust tracked files.

## 3. Generate A Temporary Baseline

For an existing repository, use `propose` to create starting entries:

```bash
cargo-allow propose --write policy/allow.proposed.toml \
  --summary-format json \
  --summary-output target/cargo-allow/propose.json
```

Review the proposed file before replacing or merging it into `policy/allow.toml`.
Generated entries should normally use `classification = "baseline_debt"` until
a human supplies a durable reason, owner, scope, and evidence.

## 4. Explain One Entry

Pick one proposed allow ID and inspect the single-entry view:

```bash
cargo-allow explain allow-0042
cargo-allow explain allow-0042 --format json --output target/cargo-allow/explain.json
```

Use this view to check whether the selector is narrow enough, whether evidence
references are present and valid, and whether the lifecycle date creates a real
review point.

## 5. Gate New Findings

Once the policy reflects the current baseline, run the same gate CI should use:

```bash
cargo-allow check --mode no-new
```

A passing check means no new unreceipted findings were found in the scanned
source-tree inventory. It does not mean every exception is safe, proven, or
reachable at runtime.

## 6. Create The First Cleanup Work Item

Generate a worklist and choose one small item:

```bash
cargo-allow worklist --format json --output target/cargo-allow/worklist.json
cargo-allow worklist --baseline-debt --difficulty small --format human
```

Prefer work that narrows scope, restores missing evidence, removes stale entries,
or converts generated baseline debt into reviewed policy. Treat the worklist as a
queue for cleanup, not permission to add suppressions.

## What To Commit

A first adoption PR should usually include:

- `policy/allow.toml` with strict requirements;
- reviewed baseline entries for existing findings;
- saved CI configuration or documentation showing the intended gate;
- no broadening of exception scope beyond the current source-tree inventory.

After that PR lands, use `cargo-allow diff --base origin/main` in pull requests
and `cargo-allow check --mode no-new` on mainline.
