# Adopting cargo-allow in a new repository

This guide is the **repeatable rollout recipe** for adopting cargo-allow from
an empty or existing repository. It records the exact bootstrap, CI, and
no-new-debt enforcement path — the same path cargo-allow dogfoods on itself.

## Prerequisites

- Rust toolchain (MSRV 1.95+)
- `cargo-allow` installed: `cargo install --git https://github.com/EffortlessMetrics/cargo-allow`
- A git repository (cargo-allow scans git-tracked files)

## Optional pre-commit integration

cargo-allow also ships a pre-commit framework hook for the same blocking
no-new check used in CI. The hook deliberately uses `language: system`: install
the `cargo-allow` binary in the environment that runs pre-commit, then pin the
repository revision in the consumer's `.pre-commit-config.yaml`.

For the current unreleased candidate, use the source revision temporarily:

```yaml
repos:
  - repo: https://github.com/EffortlessMetrics/cargo-allow
    rev: main
    hooks:
      - id: cargo-allow
```

Replace `main` with the first release tag that contains this hook before
adopting it as a stable consumer contract. The hook ignores filenames passed
by pre-commit and scans the repository's tracked source tree, so it has the
same scope as `cargo-allow check --mode no-new` in CI. Run it manually with
`pre-commit run cargo-allow --all-files`.

## Step 1: Bootstrap a policy (first hour)

From your repository root:

```bash
# See what cargo-allow finds (advisory, no policy yet):
cargo-allow audit --kind panic

# Generate a baseline policy from current findings:
cargo-allow propose --kind panic --write policy/allow.toml

# Verify the generated policy passes its own check:
cargo-allow check --mode no-new --config policy/allow.toml --kind panic
```

If `check` passes, commit `policy/allow.toml`. The baseline is now a ratchet
floor — adding a new `.unwrap()` inside the baselined scope will fail the gate.

## Step 2: Broad-scope baseline (optional)

For a broad scope (e.g. all panics in `src/`), use `add --glob` to pin the
current occurrence count:

```bash
cargo-allow add --kind panic --family unwrap --callee unwrap \
  --glob "src/**/*.rs" --owner core --reason "baseline" \
  --classification reviewed_exception --review-after 2027-01-01 \
  --config policy/allow.toml --write policy/allow.toml --force
```

The N+1th in-scope occurrence will fail `check --mode no-new` with
"occurrence_limit exceeded".

## Step 3: CI integration

Add this to `.github/workflows/cargo-allow.yml`:

```yaml
name: cargo-allow
on: [pull_request]
jobs:
  no-new-debt:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # required for diff
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install --git https://github.com/EffortlessMetrics/cargo-allow
      - run: cargo-allow check --mode no-new --config policy/allow.toml --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: cargo-allow-reports
          path: target/cargo-allow/
```

This fails the PR if new exception debt is added beyond the baseline.

## Step 4: Diff posture (optional, for policy-review enforcement)

To require a revision note for weakening policy edits:

```bash
cargo-allow diff --base origin/main --require-change-note
```

This fails if a policy change (e.g. scope broadening) lacks a matching
revision note in `.allow/revisions/`.

## Step 5: Doctor (health check)

```bash
cargo-allow doctor
```

Reports: config validity, inventory source, deleted-tracked files, git
inventory errors, skipped paths, submodule detection, broken evidence links.

## Self-adoption evidence

cargo-allow dogfoods itself on `main` — the repo's own CI runs:

```bash
cargo-allow check --mode no-new --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

This guard passes with 0 new findings on every merge to `main`. The
`policy/allow.toml` ledger receipts every scanned file.

## Known limitations

- Submodule contents are not scanned; run cargo-allow inside each submodule.
- Non-UTF-8 filenames are not yet handled (planned).
- The spec-system governance profile is advisory until portability is proven.
- `--recurse-submodules` for inventory is not supported.
