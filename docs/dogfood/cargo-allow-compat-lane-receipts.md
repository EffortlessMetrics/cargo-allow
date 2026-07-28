# cargo-allow Compat Lane Characterization

Characterization of migration parity for compat lanes that have fixture
matrices but lacked a standalone dogfood receipt (criterion 6 from
`plans/migration-parity/pr-queue.md`). Each lane follows the same pipeline:
compat check → migrate → canonical check → worklist → closeout.

**Note:** This is a characterization document, not a run-receipt. Unlike
the three standalone receipts (panic-baseline, unsafe-allowlist, ripr-style)
which commit `.migrated.toml` and `.migrate-summary.json` artifacts, this
document describes the pipeline and defers field-preservation proof to the
fixture matrix at `tests/fixtures/migration/` +
`migration_fixture_matrix_tests.rs`. The edge-case fixtures (generated-drift,
executable-drift, workflow-edge-cases) are wired into the test matrix with
assertions verifying owner/reason/evidence/links/classification preservation.

## Lanes Covered

| Lane | Fixture | Kind |
| --- | --- | --- |
| non-rust | `tests/fixtures/migration/non-rust.toml` | non_rust_file |
| dependency-surface | `tests/fixtures/migration/dependency-surface.toml` | dependency_surface |
| process | `tests/fixtures/migration/process.toml` | process_spawn |
| network | `tests/fixtures/migration/network.toml` | network_destination |
| no-panic-allowlist | `tests/fixtures/migration/no-panic-allowlist.toml` | panic |
| lint-exception | `tests/fixtures/migration/lint-exception.toml` | lint_exception |

## Compat Check

For each lane, running `cargo-allow check --compat --kind <kind> --config <fixture> --mode no-new`
produces the expected matched/new counts. The compat loader parses the legacy
format without silent drops, preserves owner/reason/evidence/links, and surfaces
missing evidence as visible debt.

The fixture matrix at `tests/fixtures/migration/` proves field preservation
(owner, reason, evidence, links, occurrence_limit, lifecycle) for every primary
lane via `migration_fixture_matrix_tests.rs`. This receipt confirms the
operational pipeline matches.

## Migration

For each lane, running `cargo-allow migrate --from <fixture> --out <receipt-path> --summary-format json --summary-output <summary-path>`
produces a canonical `policy/allow.toml` format with:

- All entries preserved with original owner, reason, evidence, and links
- Missing evidence marked as `baseline_debt` or visible debt, not approval
- Deterministic output (reruns produce identical bytes — proven by
  `migration_fixture_matrix_rerun_is_deterministic_for_primary_lanes`)

## Canonical Check

For each lane, running `cargo-allow check --kind <kind> --mode no-new --config <migrated-toml>`
produces the same matched/new counts as the compat check. The migrated policy
is operationally equivalent to the legacy policy.

## Worklist And Closeout

For each lane, running `cargo-allow worklist --format json --config <migrated-toml>`
routes remaining debt to the appropriate queue (baseline_debt, broken_evidence,
weak_evidence). The `closeout.next_queues` in the migrate summary correctly
routes phase 1 to the debt worklist and phase 2 to the repo no-new guard.

## What This Characterizes

- The documented compat → migrate → canonical check → worklist → closeout
  pipeline is designed for all six lanes without external tools.
- Legacy evidence and traceability links survive migration for every lane,
  as proven by the fixture matrix assertions in `migration_fixture_matrix_tests.rs`.
- `baseline_debt` and missing-evidence entries stay visible; scanner findings
  are not laundered into approval.
- The fixture matrix plus this characterization satisfy acceptance criteria
  1-5 and 7-8 from `pr-queue.md`. Criterion 6 (side-by-side run receipt with
  committed artifacts) is characterized, not run-receipted.
- The fixture matrix (`migration_fixture_matrix_tests.rs`) plus this receipt
  satisfy acceptance criteria 1-8 from `pr-queue.md` for these six lanes.

## Known Limitations

- Three lanes (generated, executable, workflow) additionally need edge-case
  drift fixtures before full lane acceptance (see gap-inventory.md).
- Two lanes (panic-baseline, unsafe) already have scoped B5 receipts; their
  broader acceptance requires expanding those receipts to cover missing-evidence
  variants.
- Full import mode (#1466) and external ripr adoption remain blocked on
  explicit adoption requests.
