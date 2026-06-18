# Migration Parity Gap Inventory

Living inventory for [CARGO-ALLOW-SPEC-0002](../../docs/specs/CARGO-ALLOW-SPEC-0002-migration-parity.md)
and the [PR queue](pr-queue.md). Last reconciled after #1466 governance split
(adoption-substrate-pr-005, 2026-06-18).

Parity status values:

- `complete` — lane meets all acceptance criteria in [pr-queue.md](pr-queue.md)
- `partial` — migration/compat characterization exists; parity proof or fixture
  matrix still open
- `closed` — tracked issue or discovery gap resolved; adoption follow-ups may
  remain under other rows
- `gap` — known missing behavior or open tracked issue blocks adoption
- `unknown—needs fixture` — behavior not yet characterized with a fixture

## Compat Lane Inventory

| Lane | Gap | Parity status | Owner | Evidence | PR |
| --- | --- | --- | --- | --- | --- |
| non-rust | Unified `tests/fixtures/migration/` matrix characterizes parse, metadata, evidence, and compat loaders; side-by-side dogfood receipt still open | partial | repo-infra | `migration_fixture_matrix_tests.rs`; `tests/fixtures/migration/` | adoption |
| generated | Fixture matrix covers covered_by preservation, derived generator evidence, and generated compat loader; `.gitattributes` compat drift not in matrix tree | partial | repo-infra | `tests/fixtures/migration/generated.toml`; evidence/metadata matrix | adoption |
| executable | Fixture matrix covers covered_by preservation and executable compat loader; git tree-mode drift not in matrix tree | partial | repo-infra | `tests/fixtures/migration/executable.toml`; evidence matrix | adoption |
| workflow | Fixture matrix covers workflow action evidence and workflow compat loader; workflow-file edge cases not in matrix tree | partial | repo-infra | `tests/fixtures/migration/workflow.toml`; evidence/metadata matrix | adoption |
| dependency-surface | Fixture matrix covers evidence and `dep_count_at_baseline` preservation plus compat loader | partial | repo-infra | `tests/fixtures/migration/dependency-surface.toml` | adoption |
| process | Fixture matrix covers covered_by preservation and process compat loader | partial | repo-infra | `tests/fixtures/migration/process.toml` | adoption |
| network | Fixture matrix covers evidence preservation and network compat loader | partial | repo-infra | `tests/fixtures/migration/network.toml` | adoption |
| no-panic allowlist | Fixture matrix covers structural panic migration and no-panic allowlist compat loader | partial | repo-infra | `tests/fixtures/migration/no-panic-allowlist.toml` | adoption |
| panic baseline | B5 in-repo dogfood receipt records compat/migrate/canonical/worklist/closeout for one scoped baseline; full lane acceptance still open | partial | repo-infra | `docs/dogfood/cargo-allow-panic-baseline.md`; `tests/fixtures/migration/panic-baseline*.toml`; #1691 | adoption |
| lint-exception | Fixture matrix covers reviewed and minimal `baseline_debt` clippy paths plus compat loader | partial | repo-infra | `tests/fixtures/migration/lint-exception*.toml` | adoption |
| unsafe | Fixture matrix covers reviewed evidence and missing-evidence TODO debt plus unsafe compat loader | partial | repo-infra | `tests/fixtures/migration/unsafe*.toml` | adoption |
| doc/spec-system | Spec-system profile is separate from legacy xtask compat lanes; governed by CARGO-ALLOW-SPEC-0001 closeout | partial | repo-infra | CARGO-ALLOW-CLOSEOUT-0001; `policy/doc-artifacts.toml` | out of B3 scope |
| import/parity [#1466](https://github.com/EffortlessMetrics/cargo-allow/issues/1466) | **Governance split.** Umbrella open; child issues [#1713](https://github.com/EffortlessMetrics/cargo-allow/issues/1713) (semantic selectors), [#1714](https://github.com/EffortlessMetrics/cargo-allow/issues/1714) (advisory drift), [#1715](https://github.com/EffortlessMetrics/cargo-allow/issues/1715) (re-bless receipts), [#1716](https://github.com/EffortlessMetrics/cargo-allow/issues/1716) (multi-family model), [#1717](https://github.com/EffortlessMetrics/cargo-allow/issues/1717) (owner/reason/evidence fixture), [#1718](https://github.com/EffortlessMetrics/cargo-allow/issues/1718) (ripr adoption receipt); no import mode yet | gap | repo-infra | #1466 comment split index; CARGO-ALLOW-PROP-0004; CARGO-ALLOW-SPEC-0004 | adoption-substrate-pr-005 done |
| policy dialect [#1470](https://github.com/EffortlessMetrics/cargo-allow/issues/1470) | **Closed.** Discovery prefers `policy/cargo-allow.toml`, recognizes the `policy = "cargo-allow"` dialect marker, and skips foreign-dialect `policy/allow.toml` with named diagnostics. Import-mode parity (#1466) and federation follow-ups remain open for full adoption | closed | repo-infra | #1699 merge `53ea19aa`; #1700; `policy_discovery` integration tests; `allow-policy` discovery unit tests | B6 |
| policy-dir batch import | Primary-lane batch import characterized in `migration_fixture_matrix_policy_dir_batch_imports_primary_lanes`; mixed-policy-dir failure modes and ordering still open | partial | repo-infra | `migration_fixture_matrix_tests.rs`; `policy_dir_tests.rs` | adoption |
| canonical rerun stability | Primary-lane deterministic rerun characterized in `migration_fixture_matrix_rerun_is_deterministic_for_primary_lanes`; B5 committed migrate summary is deterministic for the dogfood slice; full multi-lane batch byte-stability still open | partial | repo-infra | `migration_fixture_matrix_tests.rs`; `docs/dogfood/receipts/cargo-allow-panic-baseline.migrate-summary.json` | adoption |

## Adoption Substrate Lane (active)

Internal coherence and modularization work on the path to adoption-ready migration
substrate. Release cut (`0.1.10`) is deferred; this lane is not a publish
authorization.

| Item | Gap | Status | Owner | Evidence | PR |
| --- | --- | --- | --- | --- | --- |
| migration lane descriptors | Compat kinds lack a single modular descriptor surface for agents and docs | ready | repo-infra | active goal `adoption-substrate-pr-002` | PR 2 |
| evidence/lifecycle helpers | Shared import metadata paths are duplicated across compat loaders | blocked | repo-infra | B2 characterization in `allow-policy-legacy` | PR 3 |
| closeout queue normalization | `next_queues` routing varies by compat kind; needs consistent phased naming | done | repo-infra | #1712 `migrate_closeout_queues`; `CloseoutQueueHints` | PR 4 |
| #1466 governance split | Umbrella issue mixes import design, parity proof, and adoption blockers | done | repo-infra | #1713–#1718 child issues; #1466 split index comment | PR 5 |
| advisory occurrence counts | Baseline debt visibility lacks advisory ratcheting metadata for migration summaries | blocked | repo-infra | `baseline_debt` markers in fixture matrix | PR 6 |

## Claim Boundary

This inventory tracks observed migration characterization and known adoption
blockers. `partial` rows are not parity claims. `closed` rows record resolved
tracked issues only. `gap` rows reference open issues or missing product
behavior.
