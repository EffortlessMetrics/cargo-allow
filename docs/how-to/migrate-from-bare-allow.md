# Migrate from bare `#[allow]` and clippy attributes

This guide shows how to adopt cargo-allow when your codebase already has
scattered `#[allow(clippy::xxx)]` and `#[allow(warnings)]` attributes.

Every cargo-allow command reads findings against a policy ledger, so the
starter policy comes first. The `--compat` lint bridge is for repos that
already have a shiplog-style `policy/clippy-exceptions.toml`; a repo with only
bare attributes does not need it.

## Step 1: Generate a starter policy

```bash
cargo-allow init --strict
```

This creates `policy/allow.toml` with strict defaults (owner required, reason
required, evidence required for unsafe) and seeds a self-receipt for the
ledger file itself.

## Step 2: See what you have

```bash
cargo-allow check --kind lint-exception --format human
```

The check output is the attribute inventory: every `#[allow]`, `#[expect]`,
`#[deny]`, `#[forbid]`, and `#[warn]` finding with its file location and
attribute family. Use `--format json` or `why` (Step 5) for per-occurrence
detail. `list` shows ledger entries, not attributes — after `init` it is
empty; the attributes appear here as new findings.

Running this with `--compat --kind lint-exception` instead requires a legacy
`policy/clippy-exceptions.toml` file and fails with
`E0009: failed to read legacy policy` when the repo has none.

## Step 3: Generate baseline entries

```bash
cargo-allow propose --kind lint-exception --write policy/allow.toml --force
```

`propose` creates `baseline_debt` entries for the unreceipted findings it is
allowed to receipt. Bare `#[allow(...)]` attributes are not: with the strict
default `requirements.allow_bare_allow_attributes = false`, propose skips them
and still exits 0, printing the skip notice:

```text
not receiptable: 2 new findings not proposed; requirements.allow_bare_allow_attributes = false forbids receipting bare #[allow(...)] attributes
  next: set that requirement to true to baseline them, or repair the source so the findings disappear
```

The generated entries are temporary — they pass `check --mode no-new` but
show up as worklist items that need human review.

## Step 4: Review the worklist

```bash
cargo-allow worklist --baseline-debt --format human
```

This lists every generated baseline_debt entry. For each one, decide:

- **Keep**: add owner, reason, evidence, and lifecycle dates, then change
  classification from `baseline_debt` to `reviewed_exception`
- **Remove**: delete the `#[allow]` from source and remove the entry from policy

## Step 5: Diagnose and receipt specific findings

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

`add` on a bare `#[allow(...)]` finding fails with `E0003` and the safe
recovery while `requirements.allow_bare_allow_attributes` is still `false`:

```text
error[E0003_INVALID_POLICY]: configuration conflict: 1 lint_exception entry/entries receipt bare #[allow(...)] attributes while requirements.allow_bare_allow_attributes = false:
  - allow-0003
Next safe action: set requirements.allow_bare_allow_attributes = true if this repository intentionally receipts bare #[allow(...)] occurrences, or remove/re-scope the listed entries to non-bare selectors.
```

## Step 6: Decide how to handle bare `#[allow]` attributes

Each remaining bare `#[allow(...)]` finding needs one of two decisions:

- **The repository intentionally keeps bare allows**: set
  `requirements.allow_bare_allow_attributes = true` in `policy/allow.toml`,
  then re-run the Step 3 `propose` command to baseline them.
- **The attributes should go**: remove them from source (or convert to
  `#[expect]`, which propose can receipt without the requirement flip).

## Step 7: Verify the gate

```bash
cargo-allow check --mode no-new
```

This passes once every suppression attribute is removed from source or
receipted — bare `#[allow]` occurrences only after the Step 6 decision — and
the remaining finding kinds visible to a full check (panic-family,
non-Rust, and other lanes) are receipted by their own commands. The ledger
itself must be committed: the scanner reads git-tracked files, and an
uncommitted `policy/allow.toml` turns the seeded self-receipt stale.

## Vocabulary

Run `cargo-allow vocabulary` to list all accepted kind values, evidence prefixes,
and match statuses.
