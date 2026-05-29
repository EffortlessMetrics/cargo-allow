# CLI reference

This reference summarizes the documented `cargo-allow` command surface. Use it
when you know what task you want to perform and need the command shape, common
formats, and artifact paths.

For the authoritative option list for an installed binary, run
`cargo-allow <command> --help`.

## Invocation forms

The primary invocation is the standalone binary:

```bash
cargo-allow check --mode no-new
```

Cargo external-subcommand compatibility is optional:

```bash
cargo allow check --mode no-new
```

When developing this repository locally, run commands through the package:

```bash
cargo run -p cargo-allow -- allow check --mode no-new
```

## Commands

| Command | Use it to | Common artifact |
|---|---|---|
| `init` | Create a starter policy file. | `policy/allow.toml` |
| `audit` | Inventory source-tree findings and policy match posture without enforcing a gate. | `target/cargo-allow/audit.{json,md,html}` |
| `check` | Enforce a policy gate such as `no-new` or `strict`. | `target/cargo-allow/check.md`, `check.receipt.json`, `check.sarif` |
| `diff` | Compare PR posture against a base revision. | `target/cargo-allow/pr-summary.md` |
| `explain` | Explain one retained exception by allow ID. | `target/cargo-allow/explain.json` |
| `list` | Query retained policy entries and statuses. | `target/cargo-allow/list.json` |
| `worklist` | Generate cleanup items for humans or authorized agents. | `target/cargo-allow/worklist.json` |
| `prune` | Identify or remove stale policy entries. | `target/cargo-allow/prune.json` |
| `add` | Add a reviewed entry for a known finding. | `target/cargo-allow/add.json` |
| `propose` | Generate proposed entries for current findings. | `policy/allow.proposed.toml`, `target/cargo-allow/propose.json` |
| `migrate` | Convert legacy repository policy into `policy/allow.toml`. | `target/cargo-allow/migrate.json` |
| `doctor` | Validate policy and artifact health. | `target/cargo-allow/doctor.json` |

## Formats

Commands that render artifacts commonly support human-oriented and
machine-oriented formats. Documented examples use:

- `human` for terminal review;
- `markdown` for PR summaries and uploaded reports;
- `json` for automation and schema validation;
- `html` for static audit artifacts;
- `sarif` for code-scanning surfaces on `check`.

## Common command patterns

### Initialize policy

```bash
cargo-allow init --strict
```

### Audit current posture

```bash
cargo-allow audit --format human
cargo-allow audit --format json --output target/cargo-allow/audit.json
cargo-allow audit --format markdown --output target/cargo-allow/audit.md
cargo-allow audit --format html --output target/cargo-allow/audit.html
```

### Enforce no-new mode

```bash
cargo-allow check --mode no-new
cargo-allow check \
  --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
cargo-allow check \
  --mode no-new \
  --format sarif \
  --output target/cargo-allow/check.sarif
```

### Review PR posture

```bash
cargo-allow diff \
  --base origin/main \
  --format markdown \
  --output target/cargo-allow/pr-summary.md
```

### Explain, list, and route cleanup

```bash
cargo-allow explain allow-0042
cargo-allow explain allow-0042 --format json --output target/cargo-allow/explain.json
cargo-allow list --status baseline_debt
cargo-allow list --missing-evidence
cargo-allow worklist --difficulty small --format human
cargo-allow worklist --allow-id allow-0042 --format human
```

### Maintain policy

```bash
cargo-allow prune --stale --dry-run
cargo-allow propose --write policy/allow.proposed.toml
cargo-allow doctor
```

## JSON schemas

Machine-readable artifacts have JSON schemas in [schemas](../schemas/README.md).
Use the schemas in tests or CI when another tool consumes a `cargo-allow` JSON
artifact.
