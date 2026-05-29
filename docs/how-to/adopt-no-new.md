# Adopt No-New in CI

Use this guide when you want cargo-allow to block newly introduced source-tree
exceptions while allowing the current, reviewed ledger to remain visible.

## Before you start

You need:

- a committed `policy/allow.toml`,
- a local `cargo-allow audit` run that reviewers have inspected, and
- a decision about whether existing findings are reviewed entries or temporary
  `baseline_debt`.

If you do not have a policy yet, start with the [getting started tutorial](../tutorials/getting-started.md).

## 1. Run the gate locally

```bash
cargo-allow check --mode no-new
```

Use `--include-untracked` only for local discovery when uncommitted files should
be part of the scan:

```bash
cargo-allow check --mode no-new --include-untracked
```

Do not use `--include-untracked` for normal CI runs unless your CI job creates
source files that must be governed.

## 2. Save review artifacts

Markdown is useful for pull request summaries, and the receipt is useful for
machines that need the exact check result:

```bash
cargo-allow check --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

Upload both files from CI. The Markdown file is for reviewers; the JSON receipt
is for downstream automation.

## 3. Add the GitHub Actions step

A minimal job installs the command, runs the no-new gate, and uploads artifacts:

```yaml
name: cargo-allow

on:
  pull_request:

jobs:
  no-new:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-allow --locked
      - run: |
          mkdir -p target/cargo-allow
          cargo-allow check --mode no-new \
            --format markdown \
            --receipt target/cargo-allow/check.receipt.json \
            --output target/cargo-allow/check.md
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: cargo-allow-check
          path: target/cargo-allow
```

The repository also keeps reusable workflow examples in
`examples/github-actions/`.

## 4. Add PR posture diffs

For pull requests, render the source and policy posture relative to the base
branch:

```bash
cargo-allow diff --base origin/main \
  --format markdown \
  --output target/cargo-allow/pr-summary.md
```

Use this alongside the no-new gate. The diff explains whether the PR added,
removed, receipted, broadened, weakened, or improved exception posture.

## 5. Choose the failure policy

Use these modes deliberately:

| Mode | Use it for |
|---|---|
| `audit` | Producing reports without failing on new findings. |
| `no-new` | Normal adoption and PR gating. |
| `strict` | Requiring a fully healthy policy with no stale or missing required metadata. |
| `release` | Release lanes that need stricter lifecycle posture. |

Start with `no-new`. Move to `strict` only after baseline debt, stale entries,
missing evidence, and lifecycle gaps are actively maintained.

## 6. State the claim accurately

A passing no-new CI check supports this claim:

```text
No new unreceipted findings were found in scanned source-tree inventory.
```

Do not claim that cargo-allow proves the repository has no unsafe, panic, lint
suppression, generated code, operational script, dependency risk, or runtime
behavior outside the scanned source-tree surface.
