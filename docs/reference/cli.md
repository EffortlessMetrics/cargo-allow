# CLI Reference

This is a compact reference for the supported `cargo-allow` command surface.
Use `cargo-allow <command> --help` for the exact help text shipped by the
installed binary.

## Command map

| Command | Purpose | Common output formats |
|---|---|---|
| `init` | Create `policy/allow.toml`. | TOML policy file |
| `audit` | Inventory source-tree findings and policy health. | human, markdown, html, json, sarif |
| `check` | Run a CI gate against the ledger. | human, markdown, html, json, sarif plus optional receipt |
| `diff` | Render PR-oriented source and policy posture. | human, markdown, html, json, sarif |
| `list` | List policy entries with filters. | human, json |
| `explain` | Explain one allow entry. | human, json |
| `add` | Generate one reviewed allow entry from a current finding. | TOML plus human/json summary |
| `propose` | Generate temporary `baseline_debt` entries. | TOML plus human/json summary |
| `worklist` | Emit actionable maintenance items. | human, json |
| `migrate` | Convert compatible legacy policy files. | TOML plus human/json summary |
| `prune` | Preview or remove stale allow entries. | human, json |
| `doctor` | Validate local setup. | human, json |

## Shared scan options

Most commands that inspect source or policy accept:

- `--root <ROOT>`: source-tree root. Defaults to the nearest git root, then the
  current directory.
- `--config <CONFIG>`: policy config path.
- `--kind <KIND>`: governed surface filter when the command supports finding
  filters.
- `--include-untracked`: include untracked files in addition to git-tracked
  files.
- `--format <FORMAT>`: output format for report-like commands.
- `--output <OUTPUT>`: write output to a file instead of stdout.

## `init`

```bash
cargo-allow init --strict
cargo-allow init --strict --config policy/allow.toml
cargo-allow init --strict --force
```

Use `--force` only when you intend to overwrite an existing policy file.

## `audit`

```bash
cargo-allow audit
cargo-allow audit --format markdown --output target/cargo-allow/audit.md
cargo-allow audit --format json --output target/cargo-allow/audit.json
```

Use `audit` for visibility. Use `check` for a gate.

## `check`

```bash
cargo-allow check --mode no-new
cargo-allow check --mode strict
cargo-allow check --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

Supported modes are `audit`, `no-new`, `strict`, and `release`.

## `diff`

```bash
cargo-allow diff --base origin/main
cargo-allow diff --base origin/main --head HEAD \
  --format markdown \
  --output target/cargo-allow/pr-summary.md
```

Use `diff` in pull requests to compare source findings and policy ledger
changes against a base git revision.

## `list`

```bash
cargo-allow list --kind unsafe
cargo-allow list --family unwrap
cargo-allow list --classification baseline_debt
cargo-allow list --path crates/allow-core
cargo-allow list --source-package allow-core
cargo-allow list --status stale
cargo-allow list --broad-scope
cargo-allow list --missing-evidence
cargo-allow list --format json --output target/cargo-allow/list.json
```

`list` is policy-entry oriented. Use `worklist` when you want prioritized
maintenance tasks.

## `explain`

```bash
cargo-allow explain allow-0042
cargo-allow explain allow-0042 --format json --output target/cargo-allow/explain.json
```

Use `explain` before editing a policy entry because it combines policy fields,
current match status, lifecycle status, selector details, and evidence reference
diagnostics.

## `add`

```bash
cargo-allow add \
  --kind panic \
  --path crates/foo/src/lib.rs \
  --line 42 \
  --owner parser \
  --reason "Parser validates range before slicing" \
  --evidence test:parser_rejects_invalid_text_range \
  --write policy/allow.toml
```

`add` targets a current finding by kind, path, and nearby line. Provide a real
owner and reason; the default classification is `reviewed_exception`.

## `propose`

```bash
cargo-allow propose --write policy/allow.proposed.toml
cargo-allow propose --kind panic --write policy/allow.proposed.toml
cargo-allow propose --summary-format json --summary-output target/cargo-allow/propose.json
```

`propose` is for adoption scaffolding. Review generated entries before moving
them into the canonical policy.

## `worklist`

```bash
cargo-allow worklist --format human
cargo-allow worklist --difficulty small --format human
cargo-allow worklist --baseline-debt --format human
cargo-allow worklist --broad-scope --format human
cargo-allow worklist --missing-evidence --format human
cargo-allow worklist --format json --output target/cargo-allow/worklist.json
```

Use filters such as `--owner`, `--classification`, `--allow-id`, `--path`,
`--source-package`, `--risk`, and `--difficulty` to produce bounded handoffs.

## `migrate`

```bash
cargo-allow migrate --repo-policy policy/ --out policy/allow.toml
cargo-allow migrate --from policy/legacy.toml --out policy/allow.toml
cargo-allow migrate --summary-format json --summary-output target/cargo-allow/migrate.json
```

Use `migrate` when replacing compatible legacy policy files with the canonical
source exception ledger.

## `prune`

```bash
cargo-allow prune --stale --dry-run
cargo-allow prune --stale --write
cargo-allow prune --stale --format json --output target/cargo-allow/prune.json
```

Always review the dry-run result and the policy diff before writing stale entry
removals.

## `doctor`

```bash
cargo-allow doctor
cargo-allow doctor --format json --output target/cargo-allow/doctor.json
```

Use `doctor` to validate local setup and policy accessibility before debugging
report output.
