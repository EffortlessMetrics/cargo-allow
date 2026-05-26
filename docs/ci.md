# CI

cargo-allow has two different CI jobs:

- PR CI should run `cargo allow diff --base <base>` so reviewers can see how a
  pull request changes source exception posture.
- Mainline CI should run `cargo allow check --mode no-new` so the committed
  policy remains a passing ledger.

The example workflows are intentionally small and copyable:

- [cargo-allow-diff.yml](../examples/github-actions/cargo-allow-diff.yml)
- [cargo-allow-check.yml](../examples/github-actions/cargo-allow-check.yml)

The examples use `cargo run -p cargo-allow -- allow ...` because this repository
is a workspace that contains the CLI package. In a consuming repository with an
installed `cargo-allow`, use `cargo allow ...` instead.

## Pull Requests

Use the diff workflow for pull requests:

```bash
cargo run -p cargo-allow -- allow diff \
  --base origin/main \
  --format markdown \
  --output target/cargo-allow/pr-summary.md
```

The Markdown output starts with a PR Summary section. That section reports:

- net posture: `unchanged`, `improved`, `review-required`, or `worse`;
- current no-new failures;
- new and removed source findings;
- policy failures and policy review items;
- the reviewer action implied by those signals.

This is reviewer guidance for source-syntax and policy-ledger posture. It does
not claim macro expansion, type information, proof adequacy, or coverage.

## Mainline

Use the check workflow on `main`:

```bash
cargo run -p cargo-allow -- allow audit \
  --format json \
  --output target/cargo-allow/audit.json

cargo run -p cargo-allow -- allow check \
  --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

The JSON audit is useful for machines and future trend reporting. The receipt
is the durable CI claim for the current source exception ledger.

## Artifacts

Upload `target/cargo-allow/` even on failure. The report and receipt explain
which exception changed, whether the change was unmatched or stale, and the
claim boundary for the command.
