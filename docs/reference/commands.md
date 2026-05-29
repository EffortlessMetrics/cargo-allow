# Command Reference

This reference summarizes the user-facing cargo-allow command set. It is a quick
lookup companion to the tutorial and how-to documents; command-specific JSON
contracts live in the schema reference.

The primary command form is `cargo-allow ...`. `cargo allow ...` is accepted for
Cargo external subcommand compatibility, but cargo-allow still scans the source
tree directly.

## Global Conventions

- Source-tree paths are interpreted relative to the repository root unless a
  command documents otherwise.
- `--format json` emits machine-readable artifacts for commands that support
  structured output.
- `--output <path>` writes report-like output to a file instead of stdout.
- Summary artifact flags such as `--summary-format` and `--summary-output` are
  used by commands that edit or propose policy files.
- Reports must stay inside the claim boundary: they describe scanned
  source-tree inventory and ledger status, not semantic safety proof.

## Lifecycle Commands

| Command | Purpose | Typical use |
|---|---|---|
| `init` | Create `policy/allow.toml`. | Start an adoption with `cargo-allow init --strict`. |
| `audit` | Inventory findings and policy health. | Inspect current posture without gating a change. |
| `check` | Fail or pass according to a policy mode. | Mainline CI with `cargo-allow check --mode no-new`. |
| `diff` | Compare PR posture against a base revision. | Pull-request review with `cargo-allow diff --base origin/main`. |
| `worklist` | Emit actionable cleanup items. | Assign stale, broad, missing-evidence, or baseline-debt work. |
| `prune` | Preview or remove stale entries. | Clean policy entries no longer matched by inventory. |
| `doctor` | Validate local setup and policy readability. | Debug adoption and CI environment issues. |

## Policy Authoring Commands

| Command | Purpose | Typical use |
|---|---|---|
| `add` | Generate an entry from a current finding. | Add one reviewed exception with owner, reason, and evidence. |
| `propose` | Generate temporary baseline entries. | Bootstrap legacy repositories while marking generated debt. |
| `migrate` | Convert compatible legacy policy files. | Move bespoke xtask-era policy into `policy/allow.toml`. |

## Review Commands

| Command | Purpose | Typical use |
|---|---|---|
| `list` | Filter retained policy entries. | Find all entries by kind, family, owner, status, scope, or evidence posture. |
| `explain <id>` | Render one retained exception. | Review selector scope, evidence references, lifecycle, and match status. |

## Common Filters

`list` and `worklist` share several maintenance filters:

```bash
cargo-allow list --kind unsafe
cargo-allow list --family unwrap
cargo-allow list --classification baseline_debt
cargo-allow list --path crates/allow-core
cargo-allow list --status review_due
cargo-allow list --broad-scope
cargo-allow list --missing-evidence

cargo-allow worklist --baseline-debt --format human
cargo-allow worklist --broad-scope --format human
cargo-allow worklist --missing-evidence --format human
cargo-allow worklist --allow-id allow-0042 --format human
cargo-allow worklist --source-package allow-core --format human
```

Use filters to reduce review scope. Do not treat a filtered passing result as a
repository-wide cleanliness claim.

## Artifact Contracts

The JSON schema index documents stable v1 artifacts for:

- report and receipt output from audit, check, and diff;
- single-entry explanations;
- list, prune, propose, add, migrate, doctor, and worklist summaries.

When integrating cargo-allow into another tool, validate against those schemas
instead of scraping human output.
