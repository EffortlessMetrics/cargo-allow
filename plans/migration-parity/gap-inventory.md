# Migration Parity Gap Inventory

Living inventory for [CARGO-ALLOW-SPEC-0002](../../docs/specs/CARGO-ALLOW-SPEC-0002-migration-parity.md)
and the [PR queue](pr-queue.md). Last reconciled after portable governance
transition closeout CARGO-ALLOW-CLOSEOUT-0005 (2026-06-18).

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
| unsafe | Fixture matrix plus B5 second dogfood receipt record compat/migrate/canonical/worklist/closeout for one scoped reviewed entry; full lane acceptance still open | partial | repo-infra | `docs/dogfood/cargo-allow-unsafe-allowlist.md`; `tests/fixtures/migration/unsafe*.toml` | adoption |
| doc/spec-system | Spec-system profile is separate from legacy xtask compat lanes; governed by CARGO-ALLOW-SPEC-0001 closeout | partial | repo-infra | CARGO-ALLOW-CLOSEOUT-0001; `policy/doc-artifacts.toml` | out of B3 scope |
| import/parity [#1466](https://github.com/EffortlessMetrics/cargo-allow/issues/1466) | **Execution lane closed** (CARGO-ALLOW-CLOSEOUT-0004). Umbrella remains open for full import mode and external adoption; [#1713](https://github.com/EffortlessMetrics/cargo-allow/issues/1713)–[#1718](https://github.com/EffortlessMetrics/cargo-allow/issues/1718) characterization slices landed; ripr-style in-repo dogfood receipt records multi-family compat→migrate→check→worklist→closeout without external `ripr` migration | partial | repo-infra | `plans/migration-parity/closeouts/import-parity-lane.md`; `docs/dogfood/cargo-allow-ripr-style-adoption.md`; `import_parity_metadata_acceptance_tests.rs`; `legacy_import_batch.rs`; CARGO-ALLOW-PROP-0004; CARGO-ALLOW-SPEC-0004 | CLOSEOUT-0004 |
| import-parity governance [#1717](https://github.com/EffortlessMetrics/cargo-allow/issues/1717) | Acceptance fixture matrix proves owner/reason/evidence/`covered_by` round-trip for semantic-selector entries across no-panic, lint, and unsafe lanes; weak or missing evidence stays visible as debt. Converters already passed — characterization only; full lane `complete` parity still open | partial | repo-infra | `import_parity_metadata_acceptance_tests.rs`; `tests/fixtures/migration/*-semantic-selectors-covered-by.toml` | #1740 |
| policy dialect [#1470](https://github.com/EffortlessMetrics/cargo-allow/issues/1470) | **Closed.** Discovery prefers `policy/cargo-allow.toml`, recognizes the `policy = "cargo-allow"` dialect marker, and skips foreign-dialect `policy/allow.toml` with named diagnostics. Import-mode parity (#1466) and federation follow-ups remain open for full adoption | closed | repo-infra | #1699 merge `53ea19aa`; #1700; `policy_discovery` integration tests; `allow-policy` discovery unit tests | B6 |
| policy-dir batch import | Multi-family batch import model (`LegacyImportBatch`) preserves per-lane metadata and deterministic descriptor-table ordering; panic + lint characterization in `legacy_import_batch` and `migration_fixture_matrix_multi_family_batch_preserves_lane_metadata`; ripr-style dogfood receipt records `--repo-policy` batch on panic+unsafe+lint fixtures; mixed-policy-dir failure modes still open | partial | repo-infra | `legacy_import_batch.rs`; `docs/dogfood/cargo-allow-ripr-style-adoption.md`; `migration_fixture_matrix_tests.rs`; `policy_dir_tests.rs` | adoption |
| canonical rerun stability | Primary-lane deterministic rerun characterized in `migration_fixture_matrix_rerun_is_deterministic_for_primary_lanes`; B5 panic and unsafe dogfood migrate summaries are deterministic for their slices; full multi-lane batch byte-stability still open | partial | repo-infra | `migration_fixture_matrix_tests.rs`; `docs/dogfood/receipts/cargo-allow-panic-baseline.migrate-summary.json`; `docs/dogfood/receipts/cargo-allow-unsafe-allowlist.migrate-summary.json` | adoption |

## Adoption Substrate Lane (closed)

10-PR cleanup queue complete on main; structural identity D3–D7 slices recorded in
closeout extension. Structural identity execution lane (D1–D8) closed; D8 docs
landed. Advisory ratcheting complete: receipt `advisory` counters, `check --deny
<status>` (#1474 closed), per-lane posture (#1473), and `occurrence_headroom`
(#1472 closed). Closeouts:
[CARGO-ALLOW-CLOSEOUT-0003](closeouts/adoption-substrate-lane.md) and portable
governance transition [CARGO-ALLOW-CLOSEOUT-0005](../spec-system/closeouts/portable-governance-transition.md).
Release cut (`0.1.10`) remains deferred; these closeouts are not publish
authorization.

| Item | Gap | Status | Owner | Evidence | PR |
| --- | --- | --- | --- | --- | --- |
| migration lane descriptors | Compat kinds lack a single modular descriptor surface for agents and docs | done | repo-infra | #1709 merge `35e1f70a` | pr-002 |
| evidence/lifecycle helpers | Shared import metadata paths duplicated across compat loaders | done | repo-infra | #1711 merge `04facd42` | pr-003 |
| closeout queue normalization | `next_queues` routing varies by compat kind | done | repo-infra | #1712 `migrate_closeout_queues`; `CloseoutQueueHints` | pr-004 |
| #1466 governance split | Umbrella issue mixes import design, parity proof, and adoption blockers | done | repo-infra | #1713–#1718 child issues; #1466 split index comment | pr-005 |
| advisory occurrence counts | Baseline debt visibility lacks advisory ratcheting metadata | done | repo-infra | receipt `advisory` counters | pr-006 |
| `--deny <status>` escalation | Receipt advisory counts not promotable to blocking exit | done | repo-infra | `check --deny <status>` (#1474) | pr-007 |
| per-lane posture | No per-lane advisory/shadow/blocking model | done | repo-infra | `[lanes.<kind>]` posture (#1473) | pr-008 |
| occurrence headroom | Counted `occurrence_limit` debt could rot silently with no ratchet-down signal | done | repo-infra | receipt `advisory.occurrence_headroom`, worklist routing, `check --deny occurrence_headroom` (#1472) | post-import-1472 |
| dogfood receipts | Three in-repo side-by-side receipts (panic-baseline, unsafe-allowlist, ripr-style multi-family batch); additional lanes still open | partial | repo-infra | `docs/dogfood/cargo-allow-panic-baseline.md`; `docs/dogfood/cargo-allow-unsafe-allowlist.md`; `docs/dogfood/cargo-allow-ripr-style-adoption.md` | import-parity-1718 |
| structural identity D3 | Container module-qualification landed | done | repo-infra | #1724 merge `ffc4a47`; `plans/structural-identity/gap-inventory.md` | pr-010 |
| structural identity D4 | Receiver/target fingerprint hardening landed | done | repo-infra | #1726 merge `4f19e298`; `plans/structural-identity/gap-inventory.md` | pr-011 |
| structural identity D5 | Lint attribute target identity landed | done | repo-infra | #1728 merge `7b2f2785`; `plans/structural-identity/gap-inventory.md` | pr-012 |
| structural identity D6 | Matcher selector precision characterization landed | done | repo-infra | #1730 merge `10e98453`; `plans/structural-identity/gap-inventory.md` | pr-013 |
| structural identity D7 | Diff posture identity characterization landed | done | repo-infra | #1732 merge `1f67fd64`; `plans/structural-identity/gap-inventory.md` | pr-014 |

## Portable Governance Lane (active)

Execution transitioned from CARGO-ALLOW-GOAL-0002 to CARGO-ALLOW-GOAL-0003 after
[CARGO-ALLOW-CLOSEOUT-0005](../spec-system/closeouts/portable-governance-transition.md).
Migration, adoption-substrate, and import-parity execution lanes are archived;
advisory ratcheting (#1474, #1472) is complete on main.

| Item | Gap | Status | Owner | Evidence | PR |
| --- | --- | --- | --- | --- | --- |
| `.allow` profile resolution (C2) | Resolver prefers `.allow/` with legacy `policy/` fallback; dogfood paths unchanged until C4 | done | repo-infra | #1748 merge `2adb0b5e`; [CARGO-ALLOW-CLOSEOUT-0006](../spec-system/closeouts/profile-resolution-c2.md) | #1748 |
| `init` to `.allow/` (C3) | No CLI path materializes spec-system state under `.allow/` yet | ready | repo-infra | CARGO-ALLOW-PLAN-0004 C3; `portable-governance-c3` in `.codex/goals/active.toml` | c3 |
| P2 multi-ledger federation (#1473) | Federation across additional legacy ledgers needs design acceptance | blocked | repo-infra | `portable-governance-f0-federation`; docs/source-of-truth/README.md | — |
| external ripr adoption | In-repo ripr-style dogfood closed; external repo migration unrequested | blocked | repo-infra | `docs/dogfood/cargo-allow-ripr-style-adoption.md`; `portable-governance-external-ripr` | — |
| full import mode (#1466) | Umbrella open; characterization slices #1713–#1718 closed | blocked | repo-infra | CARGO-ALLOW-CLOSEOUT-0004; `portable-governance-full-import` | — |

## Claim Boundary

This inventory tracks observed migration characterization and known adoption
blockers. `partial` rows are not parity claims. `closed` rows record resolved
tracked issues only. `gap` rows reference open issues or missing product
behavior.
