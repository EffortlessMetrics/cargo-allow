# Quickstart tutorial

This tutorial takes a repository from no `cargo-allow` policy to a repeatable
local check. It is for first-time users who want to see the policy loop before
wiring CI.

## What you will learn

By the end, you will have:

- created a starter `policy/allow.toml`;
- inspected source-tree findings;
- generated a proposed baseline for existing findings;
- run a no-new check against that baseline;
- produced a receipt artifact that can later be uploaded from CI.

The tutorial uses `cargo-allow` as a standalone binary. If you are developing
this repository before installing the binary, prefix commands with
`cargo run -p cargo-allow -- allow` instead.

## 1. Install the binary

```bash
cargo install cargo-allow --locked
```

Confirm the command is available:

```bash
cargo-allow --help
```

## 2. Create a starter policy

Run the initializer from the repository root:

```bash
cargo-allow init --strict
```

This creates `policy/allow.toml` with strict maintenance requirements. The file
is intentionally human-owned: future entries should explain why an exception is
retained, who owns it, when it needs review, and what evidence supports it.

## 3. Inventory current findings

Ask `cargo-allow` to scan the source tree and render a human report:

```bash
cargo-allow audit --format human
```

At this stage, existing source-tree exceptions may appear as new or
unreceipted. That is expected for a repository adopting the tool for the first
time.

## 4. Generate a proposed baseline

Create a proposed policy for existing findings:

```bash
cargo-allow propose --write policy/allow.proposed.toml
```

Review the generated entries before moving them into `policy/allow.toml`.
Generated entries are starting points, not approvals. Keep temporary entries
visibly classified as `baseline_debt` until a human has narrowed the scope,
assigned ownership, and added a durable reason.

## 5. Run the no-new gate

After reviewing the proposed entries and updating `policy/allow.toml`, run the
adoption-friendly gate:

```bash
cargo-allow check --mode no-new
```

`no-new` mode is useful during adoption because it blocks newly introduced
unreceipted findings while allowing historical, reviewed baseline debt to be
paid down over time.

## 6. Produce review artifacts

Write a Markdown report and JSON receipt to `target/cargo-allow/`:

```bash
cargo-allow check \
  --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

The Markdown report is for humans. The receipt is the durable machine-readable
claim for this scan: it describes the source-tree inventory, policy match
results, and command claim boundary.

## 7. Choose the next document

Continue with one of these paths:

- To put the gate in GitHub Actions, see [CI](ci.md).
- To understand what the tool can and cannot claim, see
  [Source-tree boundary](explanation/source-tree-boundary.md).
- To look up commands and artifacts, see [CLI reference](reference/cli.md).
