# Adopt cargo-allow with no-new mode

Use this guide when a repository already contains source-tree exceptions and you
want CI protection without stopping all work until historical debt is resolved.

## Goal

Adopt `cargo-allow` so that CI fails for newly introduced unreceipted findings,
while existing reviewed debt remains visible and scheduled for cleanup.

## 1. Start with a strict policy scaffold

```bash
cargo-allow init --strict
```

Commit the scaffold after reviewing it. Strict requirements make generated or
hand-written entries carry owner, reason, classification, lifecycle, and scoped
selector information.

## 2. Capture the current posture

```bash
cargo-allow audit --format markdown --output target/cargo-allow/audit.md
cargo-allow audit --format json --output target/cargo-allow/audit.json
```

Use the Markdown report for human review and the JSON report for automation or
later comparison. Save the artifacts during adoption discussions so reviewers
can see what is being grandfathered.

## 3. Generate a proposed baseline

```bash
cargo-allow propose \
  --write policy/allow.proposed.toml \
  --summary-format json \
  --summary-output target/cargo-allow/propose.json
```

Review every generated entry before it becomes policy. For historical findings
that cannot be fixed immediately:

- keep `classification = "baseline_debt"`;
- set a real `owner` instead of `unowned` whenever possible;
- add `review_after` or `expires` dates;
- replace generic generated reasons with repository-specific rationale;
- add evidence references when the exception is already justified elsewhere.

## 4. Move reviewed entries into policy

Copy only reviewed entries from `policy/allow.proposed.toml` into
`policy/allow.toml`. Leave rejected or immediately fixable findings out of the
policy so the check continues to point at them.

Prefer narrow selectors. A broad path or glob may make the initial check pass,
but it weakens future PR review because unrelated findings can match the same
receipt.

## 5. Gate new findings

```bash
cargo-allow check --mode no-new
```

If this fails, either fix the new finding, narrow or correct the policy entry,
or create a reviewed receipt. Do not add broad baseline entries merely to quiet
CI.

## 6. Add PR and mainline CI

Use the provided GitHub Actions examples as copyable starting points:

- [PR posture diff](../../examples/github-actions/cargo-allow-diff.yml)
- [Mainline no-new check](../../examples/github-actions/cargo-allow-check.yml)

The PR lane should publish a diff summary:

```bash
cargo-allow diff \
  --base origin/main \
  --format markdown \
  --output target/cargo-allow/pr-summary.md
```

The mainline lane should publish reports and the receipt:

```bash
cargo-allow check \
  --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

## 7. Pay down baseline debt

Use `list`, `explain`, `worklist`, and `prune` to keep the baseline from
becoming permanent:

```bash
cargo-allow list --status baseline_debt
cargo-allow worklist --baseline-debt --format human
cargo-allow explain allow-0042
cargo-allow prune --stale --dry-run
```

A healthy adoption path continuously converts baseline debt into one of three
outcomes: fixed source, narrower policy, or stronger evidence.
