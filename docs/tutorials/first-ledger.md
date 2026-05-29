# Tutorial: Create Your First Source Exception Ledger

This tutorial walks through a small, end-to-end cargo-allow adoption. It is for
readers who want to learn the workflow by doing it once on a repository with an
existing source-tree exception.

By the end, you will have:

- created `policy/allow.toml`;
- inventoried current findings;
- proposed temporary baseline entries;
- reviewed one entry into a stronger receipt; and
- run a no-new gate.

## Before You Start

Use a Git working tree with at least one Rust source file. The commands below
use the installed binary form:

```bash
cargo-allow --help
```

When developing cargo-allow itself, replace `cargo-allow` with:

```bash
cargo run -p cargo-allow -- allow
```

## 1. Create The Policy File

Start with the strict starter policy so the ledger has explicit expectations:

```bash
cargo-allow init --strict
```

This creates `policy/allow.toml`. Commit the starter policy before adding
baseline debt so reviewers can see which settings were chosen.

## 2. Inventory The Current Source Tree

Run an audit to see the current posture without changing policy:

```bash
cargo-allow audit --format human
```

For reviewable artifacts, also write JSON and Markdown output:

```bash
mkdir -p target/cargo-allow
cargo-allow audit --format json --output target/cargo-allow/audit.json
cargo-allow audit --format markdown --output target/cargo-allow/audit.md
```

Read the human output first. Treat each finding as source-tree governance data,
not as proof that the code is wrong.

## 3. Generate A Temporary Proposal

Generate a proposed policy for unmatched findings:

```bash
cargo-allow propose --write policy/allow.proposed.toml
```

Open the proposed file and look for entries with:

```toml
owner = "unowned"
classification = "baseline_debt"
```

Those entries are adoption scaffolding. They should be reviewed, narrowed, or
removed instead of copied into permanent policy unchanged.

## 4. Promote One Entry Into A Receipt

Pick one proposed entry and move it into `policy/allow.toml`. Replace generated
metadata with a real owner, classification, reason, lifecycle date, and evidence
where available:

```toml
[[allow]]
id = "allow-panic-0001"
kind = "panic"
family = "indexing_slicing"
path = "crates/parser/src/span.rs"
owner = "parser"
classification = "validated_span_invariant"
reason = "The parser validates the text range before slicing."
created = "2026-05-29"
review_after = "2026-08-29"
evidence = ["test:parser_rejects_invalid_text_range"]

[allow.selector]
ast_kind = "index_expr"
container = "slice_checked_text_range"
symbol = "source[range]"
normalized_snippet_hash = "fnv1a64:..."
```

Prefer structural selector fields over line-only matching. Line numbers can help
reviewers find code, but they are not durable identity.

## 5. Check No-New Mode

Run the adoption gate:

```bash
cargo-allow check --mode no-new
```

If the command fails, inspect whether the failure is a new unreceipted finding,
a stale policy entry, invalid evidence, or missing required metadata. Fix the
ledger rather than broadening policy just to pass the command.

## 6. Review One Entry

Explain the entry you promoted:

```bash
cargo-allow explain allow-panic-0001
```

The explanation should answer who owns the exception, why it exists, which
source surface it covers, what evidence supports it, and when it must be
reviewed again.

## What To Do Next

- Use the no-new adoption guide when adding CI to a repository.
- Use the policy reference when tightening fields and selectors.
- Use the command reference when choosing output formats and artifacts.
