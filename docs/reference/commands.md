# Command Reference

This reference summarizes the public cargo-allow commands, their primary use,
and the artifacts they normally produce. Use the installed binary form
`cargo-allow ...`; `cargo allow ...` is accepted only as Cargo external
subcommand compatibility.

## Commands

| Command | Primary use | Common artifacts |
|---|---|---|
| `init` | Create a starter `policy/allow.toml`. | Policy file. |
| `audit` | Inventory source-tree findings and policy posture without enforcing a gate. | Human, Markdown, JSON, or HTML report. |
| `check` | Enforce policy in `no-new` or `strict` mode. | Report plus optional JSON receipt. |
| `diff` | Compare exception and policy posture against a base revision. | Markdown, human, or JSON PR summary. |
| `explain` | Show one retained exception with scope, lifecycle, and evidence context. | Human or JSON explanation. |
| `list` | Filter retained policy entries for maintenance. | Human or JSON list. |
| `worklist` | Generate prioritized cleanup items for humans or agents. | Human or JSON worklist. |
| `prune` | Find or remove stale policy entries. | Human or JSON prune summary. |
| `add` | Add a policy entry from command-line fields. | Updated policy plus optional JSON summary. |
| `propose` | Generate proposed entries for unmatched findings. | Proposed TOML plus optional JSON summary. |
| `migrate` | Convert legacy repository policy inputs into `policy/allow.toml`. | Migrated TOML plus optional JSON summary. |
| `doctor` | Validate policy shape, schema contracts, and local evidence references. | Human or JSON diagnostics. |

## Output Format Pattern

Commands that emit reports generally support `--format` and `--output`:

```bash
cargo-allow audit --format json --output target/cargo-allow/audit.json
cargo-allow audit --format markdown --output target/cargo-allow/audit.md
cargo-allow audit --format html --output target/cargo-allow/audit.html
```

Commands that change or propose policy keep machine summaries separate from the
policy TOML so generated policy stays parseable:

```bash
cargo-allow propose \
  --write policy/allow.proposed.toml \
  --summary-format json \
  --summary-output target/cargo-allow/propose.json
```

## Gate Modes

Use `check --mode no-new` during adoption. It prevents new unreceipted findings
while allowing explicit existing baseline debt to remain visible.

Use `check --mode strict` when the repository is ready to require the configured
metadata, evidence, lifecycle, and stale-entry rules across the ledger.

## Durable Artifact Directory

Use `target/cargo-allow/` for generated command artifacts:

```bash
mkdir -p target/cargo-allow
cargo-allow check \
  --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

Upload this directory in CI even when the command fails. The artifacts carry the
claim boundary and explain which source-tree finding or policy entry needs
review.

## JSON Schemas

Machine-readable artifacts are documented under the schema index. Keep schema
consumers pinned to the schema file that matches the command artifact they read.
