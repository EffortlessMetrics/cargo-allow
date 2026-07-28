# Migrate from bare `#[allow]` and clippy attributes

This guide shows how to adopt cargo-allow when your codebase already has
scattered `#[allow(clippy::xxx)]` and `#[allow(warnings)]` attributes.

## Step 1: See what you have

```bash
cargo-allow check --compat --kind lint-exception --format human
```

The `--compat` flag runs the scanner in compatibility mode, which detects
`#[allow]`, `#[expect]`, `#[deny]`, `#[forbid]`, and `#[warn]` attributes
in your source. The `--kind lint-exception` filter narrows results to just
lint-suppression findings.

## Step 2: List current suppressions

```bash
cargo-allow list --kind lint-exception --format human
```

This shows every `#[allow]` / `#[expect]` attribute with its file location,
the lint name, and whether it matches a policy entry yet.

## Step 3: Generate a starter policy

```bash
cargo-allow init --strict
```

This creates `policy/allow.toml` with strict defaults (owner required, reason
required, evidence required for unsafe).

## Step 4: Generate baseline entries

```bash
cargo-allow propose --kind lint-exception --write policy/allow.toml --force
```

`propose` creates `baseline_debt` entries for all unreceipted findings. These
are temporary — they pass `check --mode no-new` but show up as worklist items
that need human review.

## Step 5: Review the worklist

```bash
cargo-allow worklist --baseline-debt --format human
```

This lists every generated baseline_debt entry. For each one, decide:

- **Keep**: add owner, reason, evidence, and lifecycle dates, then change
  classification from `baseline_debt` to `reviewed_exception`
- **Remove**: delete the `#[allow]` from source and remove the entry from policy

## Step 6: Diagnose and receipt specific findings

For a specific finding, use the `why` → `add` workflow:

```bash
# Find the finding coordinates
cargo-allow check --kind lint-exception --format json | jq '.outcomes[] | select(.status == "new")'

# Diagnose why it's unreceipted
cargo-allow why --kind lint-exception --path src/lib.rs --line 42

# Receipt it with reviewed evidence
cargo-allow add --kind lint-exception --path src/lib.rs --line 42 \
  --owner "core" --reason "Reviewed: this unwrap is safe because..." \
  --evidence "test:coverage_path" --update
```

## Step 7: Verify the gate

```bash
cargo-allow check --mode no-new
```

This should pass once all findings are receipted or converted from baseline_debt
to reviewed_exception with proper evidence and lifecycle.

## Vocabulary

Run `cargo-allow vocabulary` to list all accepted kind values, evidence prefixes,
and match statuses.
