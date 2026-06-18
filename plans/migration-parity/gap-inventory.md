# Migration Parity Gap Inventory

Living inventory for [CARGO-ALLOW-SPEC-0002](../../docs/specs/CARGO-ALLOW-SPEC-0002-migration-parity.md)
and the [PR queue](pr-queue.md). Last reconciled after B1 inventory (B1r) and B2
no-panic-baseline slice (#1691, merge `1cd408e`).

Parity status values:

- `complete` — lane meets all acceptance criteria in [pr-queue.md](pr-queue.md)
- `partial` — migration/compat characterization exists; parity proof or fixture
  matrix still open
- `gap` — known missing behavior or open tracked issue blocks adoption
- `unknown—needs fixture` — behavior not yet characterized with a fixture

| Lane | Gap | Parity status | Owner | Evidence | PR |
| --- | --- | --- | --- | --- | --- |
| non-rust | Migration and compat expansion covered in `non_rust_tests.rs` and evidence matrix; no unified `tests/fixtures/migration/` matrix; no repo dogfood side-by-side receipt | partial | repo-infra | `allow-policy-legacy` non-rust tests; `docs/migration-from-xtask.md` | B3, B5 |
| generated | Parser/converter and evidence/metadata matrix characterized; `.gitattributes` compat drift not in unified migration fixture tree | partial | repo-infra | `generated_executable_tests.rs`; evidence/metadata matrix | B3, B5 |
| executable | Git tree-mode compat and evidence preservation characterized; unified migration fixtures absent | partial | repo-infra | `generated_executable_tests.rs`; evidence matrix | B3, B5 |
| workflow | Workflow file and action entries migrate; evidence/metadata matrix covers both shapes; workflow-file vs action edge cases not in shared fixture tree | partial | repo-infra | `workflow_dependency_tests.rs`; evidence/metadata matrix | B3, B5 |
| dependency-surface | Dependency entries migrate with evidence/metadata matrix coverage; side-by-side xtask delta not dogfooded | partial | repo-infra | `workflow_dependency_tests.rs`; evidence matrix | B3, B5 |
| process | Process policy entries migrate; compat matched/new/stale characterized in `process_network_tests.rs` | partial | repo-infra | `process_network_tests.rs`; evidence/metadata matrix | B3, B5 |
| network | Network policy entries migrate; compat drift characterized in `process_network_tests.rs` | partial | repo-infra | `process_network_tests.rs`; evidence/metadata matrix | B3, B5 |
| no-panic allowlist | Structural panic migration, compat drift, and evidence/covered_by preservation in `no_panic_tests.rs`; no unified migration fixture matrix | partial | repo-infra | `no_panic_tests.rs`; evidence matrix | B3, B5 |
| panic baseline | B2 closed: owner/reason/evidence/covered_by preservation, visible `baseline_debt` when evidence absent, `occurrence_limit` from legacy `count`, lifecycle `review_after`/`expires` fix (#1691); unified migration fixtures and side-by-side dogfood still open | partial | repo-infra | `no_panic_tests.rs`; evidence/metadata matrix; #1691 | B3, B5 |
| lint-exception | Clippy migration, minimal-entry `baseline_debt` path, and evidence preservation in `lint_unsafe_tests.rs`; attribute-target edge cases outside current matrix | partial | repo-infra | `lint_unsafe_tests.rs`; evidence matrix | B3, B5 |
| unsafe | Unsafe migration preserves legacy evidence; entries without evidence keep `TODO: add unsafe-review or boundary-test evidence` and `baseline_debt` (`converter_unsafe_entries.rs`); not parity-ready for unsafe-review retirement | partial | repo-infra | `lint_unsafe_tests.rs`; evidence/metadata matrix | B3, B5 |
| doc/spec-system | Spec-system profile is separate from legacy xtask compat lanes; governed by CARGO-ALLOW-SPEC-0001 closeout | partial | repo-infra | CARGO-ALLOW-CLOSEOUT-0001; `policy/doc-artifacts.toml` | out of B3 scope |
| import/parity [#1466](https://github.com/EffortlessMetrics/cargo-allow/issues/1466) | No import mode for bespoke semantic-selector ledgers (container/receiver fingerprints, advisory drift re-bless, multi-family model); blocks ripr-style adoption | gap | repo-infra | open issue #1466; CARGO-ALLOW-PROP-0004 draft scope | B6 |
| policy dialect [#1470](https://github.com/EffortlessMetrics/cargo-allow/issues/1470) | Discovery hard-fails on foreign-dialect `policy/allow.toml` without `policy = "cargo-allow"` marker; no `policy/cargo-allow.toml` preference yet | gap | repo-infra | open issue #1470; ub-review dogfood receipts in issue body | B6, PLAN-0004 |
| policy-dir batch import | `loader_policy_dir.rs` documents accepted legacy filenames; unknown—needs fixture for mixed-policy-dir failure modes and ordering | unknown—needs fixture | repo-infra | `policy_dir_tests.rs`; loader doc comment | B3 |
| canonical rerun stability | unknown—needs fixture proving byte-stable `policy/allow.toml` reruns across all compat kinds in one migration batch | unknown—needs fixture | repo-infra | scattered unit tests only | B3 |

## Claim Boundary

This inventory tracks observed migration characterization and known adoption
blockers. `partial` rows are not parity claims. `gap` rows reference open issues
or missing product behavior. `unknown—needs fixture` rows are honest inventory
placeholders until B3 characterization lands.
