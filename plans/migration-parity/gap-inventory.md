# Migration Parity Gap Inventory

Living inventory for [CARGO-ALLOW-SPEC-0002](../../docs/specs/CARGO-ALLOW-SPEC-0002-migration-parity.md)
and the [PR queue](pr-queue.md). Last reconciled after B3 migration fixture matrix characterization.

Parity status values:

- `complete` — lane meets all acceptance criteria in [pr-queue.md](pr-queue.md)
- `partial` — migration/compat characterization exists; parity proof or fixture
  matrix still open
- `gap` — known missing behavior or open tracked issue blocks adoption
- `unknown—needs fixture` — behavior not yet characterized with a fixture

| Lane | Gap | Parity status | Owner | Evidence | PR |
| --- | --- | --- | --- | --- | --- |
| non-rust | Unified `tests/fixtures/migration/` matrix characterizes parse, metadata, evidence, and compat loaders; side-by-side dogfood receipt still open | partial | repo-infra | `migration_fixture_matrix_tests.rs`; `tests/fixtures/migration/` | B5 |
| generated | Fixture matrix covers covered_by preservation, derived generator evidence, and generated compat loader; `.gitattributes` compat drift not in matrix tree | partial | repo-infra | `tests/fixtures/migration/generated.toml`; evidence/metadata matrix | B5 |
| executable | Fixture matrix covers covered_by preservation and executable compat loader; git tree-mode drift not in matrix tree | partial | repo-infra | `tests/fixtures/migration/executable.toml`; evidence matrix | B5 |
| workflow | Fixture matrix covers workflow action evidence and workflow compat loader; workflow-file edge cases not in matrix tree | partial | repo-infra | `tests/fixtures/migration/workflow.toml`; evidence/metadata matrix | B5 |
| dependency-surface | Fixture matrix covers evidence and `dep_count_at_baseline` preservation plus compat loader | partial | repo-infra | `tests/fixtures/migration/dependency-surface.toml` | B5 |
| process | Fixture matrix covers covered_by preservation and process compat loader | partial | repo-infra | `tests/fixtures/migration/process.toml` | B5 |
| network | Fixture matrix covers evidence preservation and network compat loader | partial | repo-infra | `tests/fixtures/migration/network.toml` | B5 |
| no-panic allowlist | Fixture matrix covers structural panic migration and no-panic allowlist compat loader | partial | repo-infra | `tests/fixtures/migration/no-panic-allowlist.toml` | B5 |
| panic baseline | Fixture matrix covers B2 behaviors; migrate closeout routes baseline debt and weak generated markers through `closeout.next_queues` | partial | repo-infra | `tests/fixtures/migration/panic-baseline*.toml`; `migrate_closeout_summary_tests.rs`; #1691 | B5 |
| lint-exception | Fixture matrix covers reviewed and minimal `baseline_debt` clippy paths plus compat loader | partial | repo-infra | `tests/fixtures/migration/lint-exception*.toml` | B5 |
| unsafe | Fixture matrix covers reviewed evidence and missing-evidence TODO debt plus unsafe compat loader | partial | repo-infra | `tests/fixtures/migration/unsafe*.toml` | B5 |
| doc/spec-system | Spec-system profile is separate from legacy xtask compat lanes; governed by CARGO-ALLOW-SPEC-0001 closeout | partial | repo-infra | CARGO-ALLOW-CLOSEOUT-0001; `policy/doc-artifacts.toml` | out of B3 scope |
| import/parity [#1466](https://github.com/EffortlessMetrics/cargo-allow/issues/1466) | No import mode for bespoke semantic-selector ledgers (container/receiver fingerprints, advisory drift re-bless, multi-family model); blocks ripr-style adoption | gap | repo-infra | open issue #1466; CARGO-ALLOW-PROP-0004 draft scope | B6 |
| policy dialect [#1470](https://github.com/EffortlessMetrics/cargo-allow/issues/1470) | Discovery hard-fails on foreign-dialect `policy/allow.toml` without `policy = "cargo-allow"` marker; no `policy/cargo-allow.toml` preference yet | gap | repo-infra | open issue #1470; ub-review dogfood receipts in issue body | B6, PLAN-0004 |
| policy-dir batch import | Primary-lane batch import characterized in `migration_fixture_matrix_policy_dir_batch_imports_primary_lanes`; mixed-policy-dir failure modes and ordering still open | partial | repo-infra | `migration_fixture_matrix_tests.rs`; `policy_dir_tests.rs` | B6 |
| canonical rerun stability | Primary-lane deterministic rerun characterized in `migration_fixture_matrix_rerun_is_deterministic_for_primary_lanes`; full multi-lane batch byte-stability still open | partial | repo-infra | `migration_fixture_matrix_tests.rs` | B5 |

## Claim Boundary

This inventory tracks observed migration characterization and known adoption
blockers. `partial` rows are not parity claims. `gap` rows reference open issues
or missing product behavior. `unknown—needs fixture` rows are honest inventory
placeholders until B3 characterization lands.
