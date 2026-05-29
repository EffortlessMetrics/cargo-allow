# cargo-allow JSON Schemas

These schemas describe machine-readable cargo-allow artifacts. They are local
contracts for source-tree policy scans; they do not imply build, type,
macro-expansion, or proof-level coverage.

| Artifact | Schema ID | Producer |
|---|---|---|
| Setup diagnostics | `cargo-allow.doctor.v1` | `cargo-allow doctor --format json` |
| Audit/check/diff report | `cargo-allow.report.v1` | `cargo-allow audit --format json`, `cargo-allow check --format json`, `cargo-allow diff --format json` |
| Check receipt | `cargo-allow.receipt.v1` | `cargo-allow check --receipt <path>` |
| Single-entry explanation | `cargo-allow.explain.v1` | `cargo-allow explain <id> --format json` |
| Filtered ledger list | `cargo-allow.list.v1` | `cargo-allow list --format json` |
| Stale prune preview/result | `cargo-allow.prune.v1` | `cargo-allow prune --stale --format json` |
| Baseline proposal summary | `cargo-allow.propose.v1` | `cargo-allow propose --summary-format json` |
| Single-entry add summary | `cargo-allow.add.v1` | `cargo-allow add --summary-format json` |
| Legacy migration summary | `cargo-allow.migrate.v1` | `cargo-allow migrate --summary-format json` |
| Agent worklist | `cargo-allow.worklist.v1` | `cargo-allow worklist --format json` |

## Files

- [doctor.schema.json](doctor.schema.json)
- [report.schema.json](report.schema.json)
- [receipt.schema.json](receipt.schema.json)
- [explain.schema.json](explain.schema.json)
- [list.schema.json](list.schema.json)
- [prune.schema.json](prune.schema.json)
- [propose.schema.json](propose.schema.json)
- [add.schema.json](add.schema.json)
- [migrate.schema.json](migrate.schema.json)
- [worklist.schema.json](worklist.schema.json)

## Contract Status

The `*.v1` schema IDs are the current machine-readable contract names. They are
intended for CI, review automation, and agent routing that need stable artifact
identity and source-tree claim-boundary fields.

Consumers may rely on these common root fields across all current cargo-allow
JSON artifacts:

- `schema_version`
- `schema_id`
- `tool`
- `command`
- `claim_boundary`
- `scanner_limitations`
- `inventory.scope`
- `inventory.scanner`
- `inventory.source`

When available, artifacts also include `inventory.root` and
`inventory.files_scanned`. Those values describe the local source-tree scan that
produced the artifact; `root` may be an absolute local path and should not be
treated as portable identity.

Artifact-specific fields such as `diff`, `summary`, `allow_entries`,
`work_items`, `stale_entries`, `allow_entry`, and evidence diagnostics are
covered by their individual schema files. Consumers should branch on
`schema_id`, not on command-line spelling or filenames.

## Baseline Debt Counts

Baseline debt can appear in two related but distinct places:

- `baseline_debt` is an outcome-status count. It reflects current check results
  that were classified as baseline debt outcomes.
- `policy_baseline_debt` is a policy-ledger count. It reflects retained
  `classification = "baseline_debt"` entries that remain in `policy/allow.toml`
  even when the current findings match and `check --mode no-new` passes.

Report JSON may include `summary.policy_baseline_debt` when the policy-level
count is higher than the outcome-level `summary.baseline_debt`. Check receipts
use the same distinction under `counts.policy_baseline_debt`. Consumers should
use the policy-level field for debt-burn-down metrics and the outcome-level
field for current check-status accounting.

## Compatibility Coverage

The test suite parses the current report, receipt, diff, list, explain,
worklist, prune, propose, add, migrate, and doctor JSON renderers as JSON and
checks the shared v1 source-tree contract fields. That protects the artifact
root shape from accidental manual-rendering drift.

Schema compatibility tests also lock:

- the exact top-level property set for each schema;
- the exact top-level required-field set for each schema;
- `additionalProperties = false` at the artifact root and nested object schema
  nodes; and
- structural identity fields where artifacts expose finding identity.

This is not a promise that every field is permanently frozen. Breaking changes
should either preserve the existing `*.v1` contract or introduce a new schema ID
and update the schema file, tests, and this index together.

## Consumer Guidance

Use JSON artifacts for automation:

- `report` and `receipt` for CI gates and stored no-new evidence.
- `diff` for PR posture review.
- `worklist` for agent routing.
- `list` and `explain` for policy lookup and interactive tooling.
- `doctor` for setup diagnostics before wider scans.
- `propose` for generated-baseline review.
- `add` for one finding-to-policy-entry proposal summary.
- `migrate` for legacy-policy conversion receipts.
- `prune` for stale-entry cleanup previews or write receipts.

Do not parse human, Markdown, SARIF, or HTML output as the primary contract when
a JSON artifact exists for the same workflow. Markdown and HTML are review
surfaces; JSON is the machine surface.

## Boundary

Every schema carries explicit source-tree claim-boundary and scanner-limitation
fields. Current cargo-allow artifacts mean:

```text
No new unreceipted findings were found in scanned source-tree inventory.
```

They do not mean:

```text
No unsafe, panic, lint suppression, or policy exception exists outside the
syntax-visible inventory that cargo-allow scanned.
```

cargo-allow does not invoke Cargo metadata, Cargo commands, rustc, Clippy,
build scripts, proc macros, or repository code for these scans.
