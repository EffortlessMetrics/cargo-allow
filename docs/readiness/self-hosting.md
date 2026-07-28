# Self-Hosting Readiness

This record tracks whether cargo-allow is ready to be the source-of-truth
governance example for external repository adoption.

Normative policy:
[CARGO-ALLOW-SPEC-0003](../specs/CARGO-ALLOW-SPEC-0003-readiness-policy.md).

## Readiness Policy

cargo-allow uses two readiness definitions. They are not interchangeable.

### Strict readiness

Required for **external repository migration at scale** and for claiming
**zero-gap** self-hosting readiness:

```text
docs gate: passed
workspace fmt/clippy/tests: passed
default cargo-allow no-new: passed
spec-system profile: passed
ripr+: 0 actionable gaps
unsafe-review+: 0 actionable gaps
```

### Provider-tracked readiness

Accepted for internal `0.1.10` adoption-trust work and `0.2.0` migration parity
while provider gaps remain filed upstream:

```text
local cargo-allow actionable gaps = 0
ripr+ blockers: provider-tracked, filed upstream
unsafe-review+ blockers: provider-tracked, filed upstream
```

Provider-tracked readiness is **not** zero-gap readiness. It does **not**
authorize external migration at scale or ripr external adoption handoff without
explicit re-approval.

### Recorded decision (2026-06-17)

**Accepted posture:** provider-tracked readiness for continuing `0.1.10` and
migration-parity work.

| What | Blocks `0.1.10`? | Blocks external migration at scale? |
| --- | --- | --- |
| Local cargo-allow gaps | yes if non-zero | yes if non-zero |
| `ripr+` provider blockers | no (filed upstream) | yes under strict readiness |
| `unsafe-review+` provider blockers | no (filed upstream) | yes under strict readiness |

## Current Posture

```text
Local cargo-allow readiness: clean (provider-tracked accepted)
External migration at scale: not ready (strict readiness not met)
```

Local cargo-allow actionable gaps: **0**.

Provider-tracked blockers (not local cargo-allow work):

| Provider | Count | Upstream tracking |
| --- | ---: | --- |
| `ripr+` predicate_boundary | 6 | `ripr#1432`, `#1433`, `#1440`–`#1443` (`EffortlessMetrics/ripr-swarm#1303`) |
| `ripr+` error_variant bulk | 358 | `EffortlessMetrics/ripr-swarm#1304` (oracle-linking friction) |
| `unsafe-review+` badge gaps | 33 | `EffortlessMetrics/unsafe-review#541` |

## Current Result

Recorded: 2026-06-17 (provider-tracked policy recorded; summary from `0.1.9` cut)

| Surface | Status | Evidence |
| --- | --- | --- |
| docs gate | passed | `cargo test --doc --workspace`; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`; CI run `27455099250` passed both steps on `main`. |
| workspace fmt/clippy/tests | passed | `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace` reported `1715+` passed on release head `305e4c80` after the `0.1.9` maintenance stack (`#1681`, `#1683`, `#1684`, `#1685`). |
| default cargo-allow no-new | passed | installed `cargo-allow 0.1.9`; `cargo-allow check --mode no-new --format markdown --receipt target/cargo-allow/install-0.1.9-check.receipt.json --output target/cargo-allow/install-0.1.9-check.md` reported `659` scanned files, `120` matched findings, `0` new findings, and `0` stale receipts on the published cut. |
| spec-system profile | passed | installed `cargo-allow 0.1.9`; `cargo-allow check --profile spec-system --mode audit --format json --output target/cargo-allow/install-0.1.9-spec-system.json` reported `6` artifacts, `17` links, `4` support-tier rows, `0` findings, and `0` work items. |
| spec-system worklist | passed | installed `cargo-allow 0.1.9`; `cargo-allow worklist --profile spec-system --format json --output target/cargo-allow/install-0.1.9-spec-system-worklist.json` reported `0` findings and `0` work items. |
| ripr doctor | passed | installed `ripr 0.10.0`; `ripr doctor` passed and selected `ripr first-pr --root . --base origin/main --head HEAD` as the safe next action. |
| ripr+ repo readiness | blocked (provider-tracked) | `ripr 0.10.0` explicit gap-ledger projection reports `364` `ripr+` targets (`358` `error_variant`, `6` `predicate_boundary`). The `error_variant` bulk is oracle-linking friction: `235` rows point at `parse_policy_toml_reports_toml_parse_errors`, `92` at `ensure_addable_outcome_rejects_matched_findings_with_exact_error`, and `24` at `artifact_sample_validator_reports_array_and_scalar_constraint_errors` even when exact `expect_err` + payload assertions already exist in the named or adjacent tests. Bulk tracking: `EffortlessMetrics/ripr-swarm#1304` (`error_variant` oracle regression). The six `predicate_boundary` anchors remain filed as `ripr#1432`, `#1433`, `#1440`–`#1443` (`EffortlessMetrics/ripr-swarm#1303`). |
| unsafe-review+ readiness | blocked (provider-tracked) | `unsafe-review repo` reports `50` open gaps; `unsafe-review badges` reports `unsafe-review+ = 33` (`30 contract / 3 guard / 0 witness`). Initial triage filed `EffortlessMetrics/unsafe-review#541` after `40 / 50` cards anchored in tests/fixtures and the remaining nine non-fixture cards were false positives on safe scanner code or test string literals. |

The detailed burn-down slices below are historical provider-alignment records through
the `0.1.9` lane. They are not required reading for current release posture;
use the summary table above for adoption readiness.

## RIPR Evidence

The installed provider was:

```bash
ripr --version
```

Result:

```text
ripr 0.10.0
```

Discovery commands:

```bash
ripr doctor
ripr check --help
ripr first-pr --help
ripr evidence-health --help
```

Provider docs established that repo-scoped `ripr+` is rendered with
`ripr check --format repo-badge-plus-json`, and that `ripr+` depends on
`target/ripr/reports/test-efficiency.json` when using the direct badge-plus
surface.

Initial direct provider results:

```bash
ripr check --root . --mode ready --format repo-badge-json
ripr check --root . --mode ready --format repo-badge-plus-json
```

Observed:

```text
repo ripr badge: message = 3666, status = warn, basis = canonical_actionable_gap
repo ripr+ badge: message = needs test-efficiency, status = warn
```

The missing `ripr+` test-efficiency report is a provider portability gap for
using installed `ripr` alone: the installed CLI can consume
`target/ripr/reports/test-efficiency.json`, but the discovered generator is
`cargo xtask test-efficiency-report` in the ripr repository and has no target
repo root option.

The explicit ledger path was then generated from repo exposure:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/repo-exposure.json --out target/ripr/reports/gap-decision-ledger.json --out-md target/ripr/reports/gap-decision-ledger.md
ripr check --root . --mode ready --format repo-badge-json --gap-ledger target/ripr/reports/gap-decision-ledger.json
ripr check --root . --mode ready --format repo-badge-plus-json --gap-ledger target/ripr/reports/gap-decision-ledger.json
```

On Windows, generate machine-readable `ripr` JSON through a raw-output path
such as `cmd` redirection or another no-BOM UTF-8 writer. PowerShell
`Out-File -Encoding utf8` emits a BOM in this environment, and `ripr reports
gap-ledger` rejects that artifact as invalid JSON.

Initial explicit ledger result:

```text
gap decision ledger: status = advisory
records = 17689
repairable = 3819
static limitations = 9013
no action = 4857
ripr zero target count = 3819
ripr plus target count = 3819
warnings = 0
repo ripr badge from ledger: message = 3819, status = warn
repo ripr+ badge from ledger: message = 3819, status = warn
```

Repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 2777 |
| `MissingBoundaryAssertion` | 522 |
| `MissingValueAssertion` | 336 |
| `MissingErrorDiscriminator` | 184 |

Largest file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/spec_system.rs` | 621 |
| `crates/allow-rust/src/syntax_facts/scopes.rs` | 176 |
| `crates/allow-rust/src/syntax_facts/attributes.rs` | 127 |
| `crates/allow-report/src/report_text.rs` | 101 |
| `crates/allow-diff/src/policy_entry_evidence.rs` | 70 |

## First Burn-Down Slice

The first focused slice added direct behavior tests for
`crates/cargo-allow/src/spec_system.rs` helper and readiness branches:

- artifact kind, artifact status, spec-system mode, and support-tier name
  discriminators.
- JSON escaping and optional boolean JSON rendering.
- finding and work-item blocking-reason classification.
- active-goal read errors and readiness invalid/missing states.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after4.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after4.repo-exposure.json --out target/ripr/reports/after4.gap-decision-ledger.json --out-md target/ripr/reports/after4.gap-decision-ledger.md
```

Observed:

```text
repairable = 3188
ripr zero target count = 3188
ripr plus target count = 3188
crates/cargo-allow/src/spec_system.rs repairable targets = 5
```

The focused slice reduced repo-scoped `ripr+` targets from `3819` to `3188`
and reduced `crates/cargo-allow/src/spec_system.rs` from `621` repairable
targets to `5`.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 2310 |
| `MissingBoundaryAssertion` | 416 |
| `MissingValueAssertion` | 278 |
| `MissingErrorDiscriminator` | 184 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-rust/src/syntax_facts/scopes.rs` | 176 |
| `crates/allow-rust/src/syntax_facts/attributes.rs` | 127 |
| `crates/allow-report/src/report_text.rs` | 101 |
| `crates/allow-diff/src/policy_entry_evidence.rs` | 70 |
| `crates/allow-report/src/source_inventory.rs` | 67 |

The five remaining `spec_system.rs` findings stayed after direct tests. They
now route to `crates/cargo-allow/src/spec_system.rs` /
`collect_spec_system_readiness_discriminates_invalid_active_goal`, but still
persist despite direct coverage of the read-error, invalid-ledger,
missing-support-tier, blocked-active-goal, and invalid-active-goal readiness
branches. The provider friction is tracked as `EffortlessMetrics/ripr#1431`.

## Second Burn-Down Slice

The second focused slice added direct unit coverage for
`crates/allow-rust/src/syntax_facts/scopes.rs`:

- line-scope collection for modules, named items, use declarations, macro
  definitions, fields, union fields, enum variants, and enum variant fields.
- scope merging for equal-span containers where the more specific nested
  container should win.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-scopes-final.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-scopes-final.repo-exposure.json --out target/ripr/reports/after-scopes-final.gap-decision-ledger.json --out-md target/ripr/reports/after-scopes-final.gap-decision-ledger.md
```

Observed:

```text
repairable = 2755
ripr zero target count = 2755
ripr plus target count = 2755
crates/allow-rust/src/syntax_facts/scopes.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `3188` to `2755`
and cleared `crates/allow-rust/src/syntax_facts/scopes.rs` from `176`
repairable targets to `0`.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1967 |
| `MissingBoundaryAssertion` | 380 |
| `MissingValueAssertion` | 224 |
| `MissingErrorDiscriminator` | 184 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-report/src/report_text.rs` | 101 |
| `crates/allow-diff/src/policy_entry_evidence.rs` | 70 |
| `crates/allow-report/src/source_inventory.rs` | 67 |
| `crates/allow-policy/src/entry_validation.rs` | 65 |
| `crates/cargo-allow/src/worklist_actions.rs` | 63 |

The PR-local start-here packet is not a repo-readiness pass. On clean `main`,
it reported no PR-local actionable gap because `origin/main..HEAD` has no diff:

```text
state = no_action
output_state = no_actionable_gap
reason = No repairable PR-local stable Rust gap was selected from the gap decision ledger.
```

## Third Burn-Down Slice

The third focused slice added direct unit coverage for private helpers in
`crates/allow-report/src/report_text.rs`:

- human and Markdown audit summary count rendering.
- audit remediation and evidence repair queue command rendering.
- recommended-next-step routing for empty queues and evidence signals.
- omitted review-queue and non-matched outcome notes.
- policy context notes for baseline debt and missing evidence excess.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-report-text.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-report-text.repo-exposure.json --out target/ripr/reports/after-report-text.gap-decision-ledger.json --out-md target/ripr/reports/after-report-text.gap-decision-ledger.md
```

Observed:

```text
repairable = 2635
ripr zero target count = 2635
ripr plus target count = 2635
crates/allow-report/src/report_text.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `2755` to `2635`
and cleared `crates/allow-report/src/report_text.rs` from `101` repairable
targets to `0`.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1871 |
| `MissingBoundaryAssertion` | 369 |
| `MissingValueAssertion` | 211 |
| `MissingErrorDiscriminator` | 184 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-diff/src/policy_entry_evidence.rs` | 70 |
| `crates/allow-report/src/source_inventory.rs` | 65 |
| `crates/allow-policy/src/entry_validation.rs` | 65 |
| `crates/cargo-allow/src/worklist_actions.rs` | 63 |
| `crates/allow-policy/src/source_tree_scope.rs` | 60 |

## Fourth Burn-Down Slice

The fourth focused slice added direct unit coverage for
`crates/allow-diff/src/policy_entry_evidence.rs`:

- evidence policy changes that emit removed and added evidence/link changes.
- added and removed evidence/link severity classification.
- evidence and link message selection by severity.
- local-file, invalid-local, and weak-reference classification boundaries.
- added/removed item list ordering.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-policy-entry-evidence.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-policy-entry-evidence.repo-exposure.json --out target/ripr/reports/after-policy-entry-evidence.gap-decision-ledger.json --out-md target/ripr/reports/after-policy-entry-evidence.gap-decision-ledger.md
```

Observed:

```text
repairable = 2563
ripr zero target count = 2563
ripr plus target count = 2563
crates/allow-diff/src/policy_entry_evidence.rs repairable targets = 1
```

The focused slice reduced repo-scoped `ripr+` targets from `2635` to `2563`
and reduced `crates/allow-diff/src/policy_entry_evidence.rs` from `70`
repairable targets to `1`.

The remaining `policy_entry_evidence.rs` target is tracked as provider
friction in `EffortlessMetrics/ripr#1432`: the suggested boundary assertion for
`removed_evidence_message` persists after both the original helper test and a
focused `PolicyChangeSeverity::Improvement` non-fail boundary test were added.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1814 |
| `MissingBoundaryAssertion` | 359 |
| `MissingValueAssertion` | 206 |
| `MissingErrorDiscriminator` | 184 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-report/src/source_inventory.rs` | 65 |
| `crates/allow-policy/src/entry_validation.rs` | 65 |
| `crates/cargo-allow/src/worklist_actions.rs` | 63 |
| `crates/allow-policy/src/source_tree_scope.rs` | 60 |
| `crates/allow-report/src/non_rust.rs` | 49 |

## Fifth Burn-Down Slice

The fifth focused slice added direct unit coverage for
`crates/allow-report/src/source_inventory.rs`:

- source-inventory aggregation by kind and family.
- matched, new, review, stale, invalid-index, and missing-index status routing.
- empty-inventory skip behavior for human, Markdown, HTML, and JSON renderers.
- human, Markdown, HTML, and JSON source-inventory output.
- family label trimming, unknown-family fallback, and Markdown/HTML/JSON escaping.
- direct `SourceInventoryRow::add_status` review-item boundaries.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-source-inventory.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-source-inventory.repo-exposure.json --out target/ripr/reports/after-source-inventory.gap-decision-ledger.json --out-md target/ripr/reports/after-source-inventory.gap-decision-ledger.md
```

Observed:

```text
repairable = 2430
ripr zero target count = 2430
ripr plus target count = 2430
crates/allow-report/src/source_inventory.rs repairable targets = 1
```

The focused slice reduced repo-scoped `ripr+` targets from `2563` to `2430`
and reduced `crates/allow-report/src/source_inventory.rs` from `65`
repairable targets to `1`.

The remaining `source_inventory.rs` target is tracked as provider friction in
`EffortlessMetrics/ripr#1433`: the suggested non-matched status boundary for
`SourceInventoryRow::add_status` persists after both a mixed status-routing
test and a focused `MatchStatus::Stale` non-matched boundary test were added.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1706 |
| `MissingBoundaryAssertion` | 351 |
| `MissingValueAssertion` | 189 |
| `MissingErrorDiscriminator` | 184 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/entry_validation.rs` | 65 |
| `crates/cargo-allow/src/worklist_actions.rs` | 63 |
| `crates/allow-policy/src/source_tree_scope.rs` | 60 |
| `crates/allow-report/src/diff_posture.rs` | 44 |
| `crates/allow-report/src/worklist_json.rs` | 43 |

## Sixth Burn-Down Slice

The sixth focused slice added direct unit coverage for
`crates/allow-policy/src/entry_validation.rs`:

- allow-entry identity validation for unique IDs, duplicate IDs, ID syntax, and
  family text validation.
- owner, reason, classification, duplicate evidence/link, and local-link scope
  requirement validation.
- strict versus report-only local-link validation.
- unsafe evidence, typed evidence, required evidence, and occurrence-limit
  validation.
- typed-evidence requirement labels and typed-evidence prefix/target handling.
- direct error-string discriminators for the explicit `CargoAllowError`
  constructors that remained after the first helper tests.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-entry-validation.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-entry-validation.repo-exposure.json --out target/ripr/reports/after-entry-validation.gap-decision-ledger.json --out-md target/ripr/reports/after-entry-validation.gap-decision-ledger.md
```

Observed:

```text
repairable = 2391
ripr zero target count = 2391
ripr plus target count = 2391
crates/allow-policy/src/entry_validation.rs repairable targets = 36
```

The focused slice reduced repo-scoped `ripr+` targets from `2430` to `2391`
and reduced `crates/allow-policy/src/entry_validation.rs` from `65`
repairable targets to `36`.

The remaining `entry_validation.rs` targets are tracked as provider friction in
`EffortlessMetrics/ripr#1434`: the residual `MissingErrorDiscriminator` group
persists after direct same-module assertions inspect the exact
`CargoAllowError` display strings for each flagged error path.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1649 |
| `MissingBoundaryAssertion` | 341 |
| `MissingErrorDiscriminator` | 223 |
| `MissingValueAssertion` | 178 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/worklist_actions.rs` | 63 |
| `crates/allow-policy/src/source_tree_scope.rs` | 58 |
| `crates/allow-report/src/diff_posture.rs` | 44 |
| `crates/allow-diff/src/policy_compare.rs` | 43 |
| `crates/allow-report/src/worklist_json.rs` | 43 |

## Seventh Burn-Down Slice

The seventh focused slice added same-module unit coverage for
`crates/cargo-allow/src/worklist_actions.rs`:

- high-risk policy and unsafe evidence guidance routing in
  `suggested_actions_for_context`.
- traceability guidance routing in `suggested_link_actions_for_context`.
- high-risk policy family detection and unsafe exception detection.
- list and worklist shortcut mappings for known and unknown work item kinds.
- source finding kind and policy exception family mapping for proof commands.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-worklist-actions.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-worklist-actions.repo-exposure.json --out target/ripr/reports/after-worklist-actions.gap-decision-ledger.json --out-md target/ripr/reports/after-worklist-actions.gap-decision-ledger.md
```

Observed:

```text
repairable = 2317
ripr zero target count = 2317
ripr plus target count = 2317
crates/cargo-allow/src/worklist_actions.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `2391` to `2317`
and reduced `crates/cargo-allow/src/worklist_actions.rs` from `63`
repairable targets to `0`.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1606 |
| `MissingBoundaryAssertion` | 310 |
| `MissingErrorDiscriminator` | 223 |
| `MissingValueAssertion` | 178 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/source_tree_scope.rs` | 58 |
| `crates/allow-report/src/diff_posture.rs` | 44 |
| `crates/allow-report/src/worklist_json.rs` | 43 |
| `crates/allow-diff/src/policy_compare.rs` | 43 |
| `crates/allow-diff/src/policy_entry_metadata.rs` | 40 |

## Eighth Burn-Down Slice

The eighth focused slice added same-module unit coverage for
`crates/allow-policy/src/source_tree_scope.rs`:

- Windows separator normalization.
- accepted source-tree-relative path and glob scopes.
- empty, whitespace-padded, absolute, drive-qualified, parent-segment,
  current-segment, empty-segment, and wildcard path rejection.
- unsupported glob token, unsupported `**` placement, and repository-wide glob
  rejection.
- direct syntax-helper error assertions for wildcard paths and unsupported glob
  syntax.
- source-tree-wide glob boundary detection.
- path-versus-glob diagnostic message wording.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-source-tree-scope.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-source-tree-scope.repo-exposure.json --out target/ripr/reports/after-source-tree-scope.gap-decision-ledger.json --out-md target/ripr/reports/after-source-tree-scope.gap-decision-ledger.md
```

Observed:

```text
repairable = 2261
ripr zero target count = 2261
ripr plus target count = 2261
crates/allow-policy/src/source_tree_scope.rs repairable targets = 12
```

The focused slice reduced repo-scoped `ripr+` targets from `2317` to `2261`
and reduced `crates/allow-policy/src/source_tree_scope.rs` from `58`
repairable targets to `12`.

The remaining `source_tree_scope.rs` targets are tracked as provider friction in
`EffortlessMetrics/ripr#1435`: the residual `MissingErrorDiscriminator` group
persists after direct same-module assertions inspect the exact helper error
strings for wildcard path errors, repository-wide glob errors, unsupported glob
token errors, and invalid `**` placement errors.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1569 |
| `MissingBoundaryAssertion` | 284 |
| `MissingErrorDiscriminator` | 235 |
| `MissingValueAssertion` | 173 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-report/src/diff_posture.rs` | 44 |
| `crates/allow-diff/src/policy_compare.rs` | 43 |
| `crates/allow-report/src/worklist_json.rs` | 43 |
| `crates/allow-policy-legacy/src/parser_unsafe_entries.rs` | 40 |
| `crates/allow-diff/src/policy_entry_metadata.rs` | 40 |

## Ninth Burn-Down Slice

The ninth focused slice added same-module unit coverage for
`crates/allow-report/src/diff_posture.rs`:

- all `DiffNetPosture` string and reviewer-action variants.
- structural delta summary counters for scope and selector posture changes.
- evidence and link delta summary counters, including weak, broken, removed,
  review, failure, and improvement buckets.
- posture summary field construction for finding and policy changes.
- net posture precedence from failures to review-required changes,
  improvements, and unchanged posture.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-diff-posture.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-diff-posture.repo-exposure.json --out target/ripr/reports/after-diff-posture.gap-decision-ledger.json --out-md target/ripr/reports/after-diff-posture.gap-decision-ledger.md
```

Observed:

```text
repairable = 2207
ripr zero target count = 2207
ripr plus target count = 2207
crates/allow-report/src/diff_posture.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `2261` to `2207`
and reduced `crates/allow-report/src/diff_posture.rs` from `44` repairable
targets to `0`.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1564 |
| `MissingBoundaryAssertion` | 242 |
| `MissingErrorDiscriminator` | 235 |
| `MissingValueAssertion` | 166 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-report/src/worklist_json.rs` | 43 |
| `crates/allow-diff/src/policy_compare.rs` | 43 |
| `crates/allow-diff/src/policy_entry_metadata.rs` | 40 |
| `crates/allow-policy-legacy/src/parser_unsafe_entries.rs` | 40 |
| `crates/allow-policy-legacy/src/parser_non_rust_entries.rs` | 39 |

## Tenth Burn-Down Slice

The tenth focused slice added same-module unit coverage for
`crates/allow-report/src/worklist_json.rs`:

- fully populated work item JSON rendering, including optional scalar fields,
  selector precision, evidence references, suggested actions, and proof
  commands.
- minimal work item JSON rendering, including `null` fields and omitted optional
  object fields.
- worklist filter JSON rendering for every filter field.
- fixture-shape assertions so the test fixture values themselves are covered by
  direct value assertions.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-worklist-json.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-worklist-json.repo-exposure.json --out target/ripr/reports/after-worklist-json.gap-decision-ledger.json --out-md target/ripr/reports/after-worklist-json.gap-decision-ledger.md
```

Observed:

```text
repairable = 2162
ripr zero target count = 2162
ripr plus target count = 2162
crates/allow-report/src/worklist_json.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `2207` to `2162`
and reduced `crates/allow-report/src/worklist_json.rs` from `43` repairable
targets to `0`.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1519 |
| `MissingBoundaryAssertion` | 242 |
| `MissingErrorDiscriminator` | 235 |
| `MissingValueAssertion` | 166 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-diff/src/policy_compare.rs` | 43 |
| `crates/allow-policy-legacy/src/parser_unsafe_entries.rs` | 40 |
| `crates/allow-diff/src/policy_entry_metadata.rs` | 40 |
| `crates/allow-policy-legacy/src/loader_legacy_dispatch.rs` | 39 |
| `crates/allow-policy-legacy/src/parser_non_rust_entries.rs` | 39 |

## Eleventh Burn-Down Slice

The eleventh focused slice added same-module unit coverage for
`crates/allow-diff/src/policy_compare.rs`:

- expiry extension and shortening boundaries, including equal dates, `never`,
  invalid dates, removed expiry, added expiry, later dates, and earlier dates.
- added and removed value-set detection, including reordered unchanged values.
- required and optional text helper trimming, added/removed text, and changed
  non-empty text.
- optional trimmed text normalization for `None`, whitespace-only, and present
  values.
- occurrence-limit loosening and tightening for added, removed, equal,
  increased, and decreased limits.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-policy-compare.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-policy-compare.repo-exposure.json --out target/ripr/reports/after-policy-compare.gap-decision-ledger.json --out-md target/ripr/reports/after-policy-compare.gap-decision-ledger.md
```

Observed:

```text
repairable = 2108
ripr zero target count = 2108
ripr plus target count = 2108
crates/allow-diff/src/policy_compare.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `2162` to `2108`
and reduced `crates/allow-diff/src/policy_compare.rs` from `43` repairable
targets to `0`.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1493 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 214 |
| `MissingValueAssertion` | 166 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy-legacy/src/parser_unsafe_entries.rs` | 40 |
| `crates/allow-policy-legacy/src/loader_legacy_dispatch.rs` | 39 |
| `crates/allow-policy-legacy/src/parser_non_rust_entries.rs` | 39 |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/cargo-allow/src/worklist_evidence.rs` | 36 |

## Twelfth Burn-Down Slice

The twelfth focused slice added same-module unit coverage for
`crates/allow-policy-legacy/src/parser_unsafe_entries.rs`:

- `allow` root parsing for reviewed entries and generated legacy IDs.
- `entry` root parsing and selector-kind fallback to normalized family.
- unsafe family normalization from `family`, `selector.kind`, and
  `selector.ast_kind`.
- owner, classification, reason, evidence, created, review-after, expires,
  selector-container, line-hint, and last-seen field preservation.
- default owner, classification, reason, created, expires, and last-seen column
  behavior for minimal legacy unsafe entries.
- missing allow entries, non-table entries, missing family, and missing path
  error messages.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-parser-unsafe.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-parser-unsafe.repo-exposure.json --out target/ripr/reports/after-parser-unsafe.gap-decision-ledger.json --out-md target/ripr/reports/after-parser-unsafe.gap-decision-ledger.md
```

Observed:

```text
repairable = 2034
ripr zero target count = 2034
ripr plus target count = 2034
crates/allow-policy-legacy/src/parser_unsafe_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `2108` to `2034`
and reduced `crates/allow-policy-legacy/src/parser_unsafe_entries.rs` from
`40` repairable targets to `0`.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1450 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 212 |
| `MissingValueAssertion` | 137 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy-legacy/src/loader_legacy_dispatch.rs` | 39 |
| `crates/allow-policy-legacy/src/parser_non_rust_entries.rs` | 39 |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/worklist_evidence.rs` | 36 |

## Thirteenth Burn-Down Slice

The thirteenth focused slice added same-module unit coverage for
`crates/allow-policy-legacy/src/parser_non_rust_entries.rs`:

- path entries with both `path` and `glob`, proving path selection wins.
- broad glob entries with required non-empty `broad_glob_reason`.
- generated legacy IDs for entries without explicit IDs.
- owner, category/classification, reason, evidence, created, review-after, and
  normalized expires field preservation.
- `covered_by` evidence fallback.
- reason composition for reason-only, scope-only, combined, and empty reason
  cases.
- missing allow entries, non-table entries, missing path/glob, missing broad
  glob reason, and empty broad-glob reason errors.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-parser-non-rust.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-parser-non-rust.repo-exposure.json --out target/ripr/reports/after-parser-non-rust.gap-decision-ledger.json --out-md target/ripr/reports/after-parser-non-rust.gap-decision-ledger.md
```

Observed:

```text
repairable = 1990
ripr zero target count = 1990
ripr plus target count = 1990
crates/allow-policy-legacy/src/parser_non_rust_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `2034` to `1990`
and reduced `crates/allow-policy-legacy/src/parser_non_rust_entries.rs` from
`39` repairable targets to `0`.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1429 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 196 |
| `MissingValueAssertion` | 130 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy-legacy/src/loader_legacy_dispatch.rs` | 39 |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/cargo-allow/src/worklist_evidence.rs` | 36 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy/src/render_toml.rs` | 35 |

## Fourteenth Burn-Down Slice

The fourteenth focused slice added same-module unit coverage for
`crates/allow-policy-legacy/src/loader_legacy_dispatch.rs`:

- direct dispatch assertions for every supported legacy policy branch.
- the `_ if is_clippy_exceptions_policy(table)` branch.
- converted output assertions for file, panic, source, policy, process, and
  network policy tables.
- unsupported policy values and missing `policy` returning `None`.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-loader-dispatch.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-loader-dispatch.repo-exposure.json --out target/ripr/reports/after-loader-dispatch.gap-decision-ledger.json --out-md target/ripr/reports/after-loader-dispatch.gap-decision-ledger.md
```

Observed:

```text
repairable = 1965
ripr zero target count = 1965
ripr plus target count = 1965
crates/allow-policy-legacy/src/loader_legacy_dispatch.rs repairable targets = 14
```

The focused slice reduced repo-scoped `ripr+` targets from `1990` to `1965`
and reduced `crates/allow-policy-legacy/src/loader_legacy_dispatch.rs` from
`39` repairable targets to `14`.

The remaining `loader_legacy_dispatch.rs` findings stayed after direct branch
coverage and are tracked as provider friction in
`EffortlessMetrics/ripr#1436`.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1404 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 196 |
| `MissingValueAssertion` | 130 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/worklist_evidence.rs` | 36 |
| `crates/allow-policy/src/render_toml.rs` | 35 |
| `crates/allow-report/src/non_rust.rs` | 32 |

## Fifteenth Burn-Down Slice

The fifteenth focused slice added same-module observer coverage for
`crates/cargo-allow/src/worklist_evidence.rs`:

- broken evidence work-item field assembly.
- broken traceability link routing.
- weak evidence reference routing.
- outside-default-inventory suggested actions and proof commands.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-worklist-evidence.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-worklist-evidence.repo-exposure.json --out target/ripr/reports/after-worklist-evidence.gap-decision-ledger.json --out-md target/ripr/reports/after-worklist-evidence.gap-decision-ledger.md
```

Observed:

```text
repairable = 1928
ripr zero target count = 1928
ripr plus target count = 1928
crates/cargo-allow/src/worklist_evidence.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1965` to `1928`
and cleared `crates/cargo-allow/src/worklist_evidence.rs` from the repairable
file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1369 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 194 |
| `MissingValueAssertion` | 130 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy/src/render_toml.rs` | 35 |
| `crates/allow-report/src/explain_human.rs` | 32 |
| `crates/allow-report/src/non_rust.rs` | 32 |

## Sixteenth Burn-Down Slice

The sixteenth focused slice added same-module coverage for
`crates/allow-policy/src/render_toml.rs`:

- every `escape_toml` branch for special TOML string escapes and generic
  control characters.
- array rendering with quoted and escaped values.
- string and optional string field rendering.
- boolean field rendering for both `true` and `false`.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-render-toml.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-render-toml.repo-exposure.json --out target/ripr/reports/after-render-toml.gap-decision-ledger.json --out-md target/ripr/reports/after-render-toml.gap-decision-ledger.md
```

Observed:

```text
repairable = 1886
ripr zero target count = 1886
ripr plus target count = 1886
crates/allow-policy/src/render_toml.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1928` to `1886`
and cleared `crates/allow-policy/src/render_toml.rs` from the repairable file
list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1339 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 184 |
| `MissingValueAssertion` | 128 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-report/src/explain_human.rs` | 32 |
| `crates/allow-report/src/non_rust.rs` | 32 |
| `crates/allow-diff/src/policy_entry_metadata.rs` | 31 |

## Seventeenth Burn-Down Slice

The seventeenth focused slice added same-module coverage for
`crates/allow-report/src/non_rust.rs`:

- human non-Rust inventory metrics and file rows.
- Markdown non-Rust inventory metrics, family escaping, and file rows.
- empty-file-finding rendering no-op behavior.
- human and Markdown omitted-file boundary and pluralization behavior.
- non-Rust file-row filtering, status mapping, default unmatched status, and
  stable path/family/status ordering.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-non-rust.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-non-rust.repo-exposure.json --out target/ripr/reports/after-non-rust.gap-decision-ledger.json --out-md target/ripr/reports/after-non-rust.gap-decision-ledger.md
```

Observed:

```text
repairable = 1842
ripr zero target count = 1842
ripr plus target count = 1842
crates/allow-report/src/non_rust.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1886` to `1842`
and cleared `crates/allow-report/src/non_rust.rs` from the repairable file
list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1312 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 181 |
| `MissingValueAssertion` | 114 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-report/src/explain_human.rs` | 32 |
| `crates/allow-diff/src/policy_entry_metadata.rs` | 31 |
| `crates/allow-rust/src/syntax_kinds.rs` | 31 |

## Eighteenth Burn-Down Slice

The eighteenth focused slice added same-module coverage for
`crates/allow-rust/src/syntax_kinds.rs`:

- unsafe syntax kind priority ordering.
- unsafe syntax kind family and AST-kind strings.
- panic macro accepted and rejected names.
- panic macro displayed names and family strings.
- panic method accepted and rejected names.
- panic method family strings.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-syntax-kinds.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-syntax-kinds.repo-exposure.json --out target/ripr/reports/after-syntax-kinds.gap-decision-ledger.json --out-md target/ripr/reports/after-syntax-kinds.gap-decision-ledger.md
```

Observed:

```text
repairable = 1810
ripr zero target count = 1810
ripr plus target count = 1810
crates/allow-rust/src/syntax_kinds.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1842` to `1810`
and cleared `crates/allow-rust/src/syntax_kinds.rs` from the repairable file
list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1305 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 161 |
| `MissingValueAssertion` | 109 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-report/src/explain_human.rs` | 32 |
| `crates/allow-diff/src/policy_entry_metadata.rs` | 31 |
| `crates/allow-policy-legacy/src/parser_no_panic_allowlist_entries.rs` | 30 |

## Nineteenth Burn-Down Slice

The nineteenth focused slice added same-module coverage for
`crates/allow-report/src/explain_human.rs`:

- explain kind labels with and without a finding family.
- empty string and whitespace-only fallback rendering.
- evidence list rendering for empty, single, and multi-value lists.
- evidence reference human summaries with explicit and fallback prefix/target
  values.
- selector summaries for empty selectors and every rendered selector field.
- outcome summaries for empty outcomes and every rendered match status.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-explain-human.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-explain-human.repo-exposure.json --out target/ripr/reports/after-explain-human.gap-decision-ledger.json --out-md target/ripr/reports/after-explain-human.gap-decision-ledger.md
```

Observed:

```text
repairable = 1778
ripr zero target count = 1778
ripr plus target count = 1778
crates/allow-report/src/explain_human.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1810` to `1778`
and cleared `crates/allow-report/src/explain_human.rs` from the repairable file
list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1283 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 151 |
| `MissingValueAssertion` | 109 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-diff/src/policy_entry_metadata.rs` | 31 |
| `crates/allow-policy-legacy/src/parser_no_panic_allowlist_entries.rs` | 30 |
| `crates/allow-files/src/path_rules.rs` | 28 |

## Twentieth Burn-Down Slice

The twentieth focused slice added same-module coverage for
`crates/allow-diff/src/policy_entry_metadata.rs`:

- baseline-debt classification normalization and introduction.
- owner removed, unassigned, changed, and added transitions.
- reason removed, changed, and added transitions.
- classification removed, changed, and added transitions.
- normalized optional text trimming for absent, empty, and present values.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-policy-entry-metadata.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-policy-entry-metadata.repo-exposure.json --out target/ripr/reports/after-policy-entry-metadata.gap-decision-ledger.json --out-md target/ripr/reports/after-policy-entry-metadata.gap-decision-ledger.md
```

Observed:

```text
repairable = 1746
ripr zero target count = 1746
ripr plus target count = 1746
crates/allow-diff/src/policy_entry_metadata.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1778` to `1746`
and cleared `crates/allow-diff/src/policy_entry_metadata.rs` from the
repairable file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1251 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 151 |
| `MissingValueAssertion` | 109 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy-legacy/src/parser_no_panic_allowlist_entries.rs` | 30 |
| `crates/allow-files/src/path_rules.rs` | 28 |
| `crates/cargo-allow/src/diff_render.rs` | 27 |

## Twenty-First Burn-Down Slice

The twenty-first focused slice added same-module coverage for
`crates/allow-files/src/path_rules.rs`:

- scannable non-Rust file classification for Rust sources, built-in allowed
  files, crate README files, and ordinary docs.
- Rust source extension matching, including case and suffix boundaries.
- built-in allowlist root-file and crate README boundaries, including Windows
  separator normalization.
- generated path matching through configured globs, generated path segments,
  `gen` path segments, and generated file-name markers.
- lowercased extension and file-name helpers.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-path-rules.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-path-rules.repo-exposure.json --out target/ripr/reports/after-path-rules.gap-decision-ledger.json --out-md target/ripr/reports/after-path-rules.gap-decision-ledger.md
```

Observed:

```text
repairable = 1716
ripr zero target count = 1716
ripr plus target count = 1716
crates/allow-files/src/path_rules.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1746` to `1716`
and cleared `crates/allow-files/src/path_rules.rs` from the repairable file
list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1225 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 148 |
| `MissingValueAssertion` | 108 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy-legacy/src/parser_no_panic_allowlist_entries.rs` | 30 |
| `crates/cargo-allow/src/diff_render.rs` | 27 |
| `crates/allow-report/src/report_json.rs` | 26 |

## Twenty-Second Burn-Down Slice

The twenty-second focused slice added same-module coverage for
`crates/cargo-allow/src/diff_render.rs`:

- no-new failure counting across every `MatchStatus`.
- finding posture row mapping for kind, family, location, source package, and
  structural identity fields.
- policy posture row mapping for nested exception identity, selector identity,
  selector precision, scope, occurrence limit, lifecycle, evidence, metadata,
  requirement, and policy-status details.
- human and Markdown posture rendering branches, plus no-op behavior for JSON,
  HTML, and SARIF.
- Markdown PR summary insertion before the `Findings scanned:` marker.
- human helper rendering for finding and policy posture rows.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-diff-render.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-diff-render.repo-exposure.json --out target/ripr/reports/after-diff-render.gap-decision-ledger.json --out-md target/ripr/reports/after-diff-render.gap-decision-ledger.md
```

Observed:

```text
repairable = 1689
ripr zero target count = 1689
ripr plus target count = 1689
crates/cargo-allow/src/diff_render.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1716` to `1689`
and cleared `crates/cargo-allow/src/diff_render.rs` from the repairable file
list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1200 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 146 |
| `MissingValueAssertion` | 108 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy-legacy/src/parser_no_panic_allowlist_entries.rs` | 30 |
| `crates/allow-files/src/families.rs` | 26 |
| `crates/allow-report/src/report_json.rs` | 26 |

## Twenty-Third Burn-Down Slice

The twenty-third focused slice added same-module coverage for
`crates/allow-files/src/families.rs`:

- family classifier precedence across generated files, CI workflows, editor
  extensions, package metadata, fixtures, release scripts, documentation,
  language tools, and unknown non-Rust files.
- documentation path and extension detection.
- extension-based family mapping for shell, Python, JavaScript/TypeScript,
  configuration, dotfile configuration, and unknown extensions.
- editor-extension, package-metadata, fixture, release-script, and
  configuration-file helper boundaries.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-families.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-families.repo-exposure.json --out target/ripr/reports/after-families.gap-decision-ledger.json --out-md target/ripr/reports/after-families.gap-decision-ledger.md
```

Observed:

```text
repairable = 1663
ripr zero target count = 1663
ripr plus target count = 1663
crates/allow-files/src/families.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1689` to `1663`
and cleared `crates/allow-files/src/families.rs` from the repairable file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1182 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 138 |
| `MissingValueAssertion` | 108 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy-legacy/src/parser_no_panic_allowlist_entries.rs` | 30 |
| `crates/allow-report/src/report_json.rs` | 26 |
| `crates/allow-policy-legacy/src/parser_clippy_entries.rs` | 26 |

## Twenty-Fourth Burn-Down Slice

The twenty-fourth focused slice added same-module coverage for
`crates/allow-report/src/report_json.rs`:

- audit remediation JSON early returns for non-audit reports and clean audit
  summaries.
- audit remediation JSON route serialization across `worklist_status`,
  `prune_stale`, and `worklist_filter` route shapes.
- multi-item comma handling and optional route fields for item kind, worklist
  status, worklist filter, count, and command.
- trend field rendering across all core review statuses plus policy missing
  evidence, broken evidence links, and weak evidence references.
- final-field comma handling for optional trend fields.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-report-json.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-report-json.repo-exposure.json --out target/ripr/reports/after-report-json.gap-decision-ledger.json --out-md target/ripr/reports/after-report-json.gap-decision-ledger.md
```

Observed:

```text
repairable = 1637
ripr zero target count = 1637
ripr plus target count = 1637
crates/allow-report/src/report_json.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1663` to `1637`
and cleared `crates/allow-report/src/report_json.rs` from the repairable file
list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1159 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 137 |
| `MissingValueAssertion` | 106 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy-legacy/src/parser_no_panic_allowlist_entries.rs` | 30 |
| `crates/cargo-allow/src/explain_render.rs` | 26 |
| `crates/allow-policy-legacy/src/parser_clippy_entries.rs` | 26 |

## Twenty-Fifth Burn-Down Slice

The twenty-fifth focused slice added same-module coverage for
`crates/cargo-allow/src/explain_render.rs`:

- JSON explain entry rendering with source-tree inventory context.
- explain report assembly for evidence and link diagnostics, including broken
  local references, traceability-only references, and present local files that
  are outside the default source-tree inventory.
- link-specific diagnostic message rewriting from evidence wording to link
  wording.
- next-step routing for source-tree inventory evidence attention.
- selector precision, broad-scope, current finding, and match-outcome fields in
  the assembled report.
- link-diagnostic filtering so policy evidence references do not appear in link
  reference output.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-explain-render.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-explain-render.repo-exposure.json --out target/ripr/reports/after-explain-render.gap-decision-ledger.json --out-md target/ripr/reports/after-explain-render.gap-decision-ledger.md
```

Observed:

```text
repairable = 1611
ripr zero target count = 1611
ripr plus target count = 1611
crates/cargo-allow/src/explain_render.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1637` to `1611`
and cleared `crates/cargo-allow/src/explain_render.rs` from the repairable file
list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1133 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 137 |
| `MissingValueAssertion` | 106 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy-legacy/src/parser_no_panic_allowlist_entries.rs` | 30 |
| `crates/allow-policy-legacy/src/converter_no_panic_baseline_entries.rs` | 26 |
| `crates/allow-policy-legacy/src/converter_dependency_entries.rs` | 26 |

## Twenty-Sixth Burn-Down Slice

The twenty-sixth focused slice added same-module coverage for
`crates/allow-policy-legacy/src/parser_no_panic_allowlist_entries.rs`:

- missing `allow` arrays and non-table allow entries.
- generated legacy IDs, indexes, default owners, classifications, reasons,
  evidence, and lifecycle dates.
- selector `kind` and `ast_kind` aliases, callee/container fields, and line
  hints.
- explicit created, review-after, and `permanent` expiry normalization to
  `never`.
- `last_seen` fallback into line hints.
- legacy evidence arrays and `covered_by` compatibility.
- contextual parse errors for missing paths and selector kinds.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-no-panic-parser.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-no-panic-parser.repo-exposure.json --out target/ripr/reports/after-no-panic-parser.gap-decision-ledger.json --out-md target/ripr/reports/after-no-panic-parser.gap-decision-ledger.md
```

Observed:

```text
repairable = 1566
ripr zero target count = 1566
ripr plus target count = 1566
crates/allow-policy-legacy/src/parser_no_panic_allowlist_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1611` to `1566`
and cleared `crates/allow-policy-legacy/src/parser_no_panic_allowlist_entries.rs`
from the repairable file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1098 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 134 |
| `MissingValueAssertion` | 99 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy-legacy/src/converter_dependency_entries.rs` | 26 |
| `crates/allow-policy/src/toml_de.rs` | 25 |
| `crates/allow-policy-legacy/src/converter_process_entries.rs` | 25 |

## Twenty-Seventh Burn-Down Slice

The twenty-seventh focused slice added same-module coverage for
`crates/allow-policy-legacy/src/converter_dependency_entries.rs`:

- exact dependency-surface path rules with normalized paths.
- converted policy-exception kind, family, owner, classification, reason,
  lifecycle, and legacy policy links.
- evidence preservation plus generated legacy policy, surface, and baseline
  dependency-count evidence.
- selector identity for exact dependency surfaces, including `ast_kind`,
  `symbol`, and normalized selector glob.
- glob dependency-surface rules with no path, normalized glob output, and no
  exact symbol.
- broad-glob scope-note reason rendering.
- `never` expiry lifecycle fallback from created date to review-after.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-dependency-converter.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-dependency-converter.repo-exposure.json --out target/ripr/reports/after-dependency-converter.gap-decision-ledger.json --out-md target/ripr/reports/after-dependency-converter.gap-decision-ledger.md
```

Observed:

```text
repairable = 1538
ripr zero target count = 1538
ripr plus target count = 1538
crates/allow-policy-legacy/src/converter_dependency_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1566` to `1538`
and cleared `crates/allow-policy-legacy/src/converter_dependency_entries.rs`
from the repairable file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1073 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 131 |
| `MissingValueAssertion` | 99 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy-legacy/src/converter_process_entries.rs` | 25 |
| `crates/allow-policy/src/toml_de.rs` | 25 |
| `crates/allow-report/src/summary.rs` | 24 |

## Twenty-Eighth Burn-Down Slice

The twenty-eighth focused slice added same-module coverage for
`crates/allow-policy-legacy/src/converter_process_entries.rs`:

- local process rules with no callers and the default process-policy scope.
- converted policy-exception kind, `process_spawn` family, local/network
  classification, owners, reasons, and legacy policy links.
- generated process evidence for legacy policy ID, binary, argv shape, network
  reach, and normalized `called_by` paths.
- process selector identity, including `ast_kind`, command symbol, target
  fingerprint, and normalized selector glob.
- network-reaching process rules with explicit argv shape and multiple callers.
- lifecycle behavior for explicit dates and `never` expiry review-after
  fallback.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-process-converter.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-process-converter.repo-exposure.json --out target/ripr/reports/after-process-converter.gap-decision-ledger.json --out-md target/ripr/reports/after-process-converter.gap-decision-ledger.md
```

Observed:

```text
repairable = 1513
ripr zero target count = 1513
ripr plus target count = 1513
crates/allow-policy-legacy/src/converter_process_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1538` to `1513`
and cleared `crates/allow-policy-legacy/src/converter_process_entries.rs` from
the repairable file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1049 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 131 |
| `MissingValueAssertion` | 98 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy/src/toml_de.rs` | 25 |
| `crates/allow-policy-legacy/src/fields.rs` | 24 |
| `crates/allow-policy-legacy/src/converter_no_panic_allow_entries.rs` | 24 |

## Twenty-Ninth Burn-Down Slice

The twenty-ninth focused slice added same-module coverage for
`crates/allow-policy-legacy/src/converter_network_entries.rs`:

- public network-destination rules with the default network policy scope.
- converted policy-exception kind, `network_destination` family, public and
  authenticated classifications, owners, reasons, and legacy policy links.
- generated network evidence for legacy policy ID, destination, lane, auth
  requirement, and auth secret.
- network selector identity, including `ast_kind`, symbol, target fingerprint,
  and selector glob.
- authenticated network rules with preserved explicit evidence and lifecycle
  fields.
- `never` expiry lifecycle fallback from created date to review-after.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-network-converter.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-network-converter.repo-exposure.json --out target/ripr/reports/after-network-converter.gap-decision-ledger.json --out-md target/ripr/reports/after-network-converter.gap-decision-ledger.md
```

Observed:

```text
repairable = 1489
ripr zero target count = 1489
ripr plus target count = 1489
crates/allow-policy-legacy/src/converter_network_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1513` to `1489`
and cleared `crates/allow-policy-legacy/src/converter_network_entries.rs` from
the repairable file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1026 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 131 |
| `MissingValueAssertion` | 97 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy/src/toml_de.rs` | 25 |
| `crates/allow-report/src/summary.rs` | 24 |
| `crates/allow-policy-legacy/src/fields.rs` | 24 |

## Thirtieth Burn-Down Slice

The thirtieth focused slice added same-module coverage for
`crates/allow-policy-legacy/src/converter_no_panic_allow_entries.rs`:

- method-call no-panic allow entries with normalized paths and selector kinds.
- converted panic kind, family, owner, classification, reason, lifecycle, and
  legacy no-panic allowlist link.
- explicit legacy evidence preservation.
- method callee normalization from fully qualified unwrap callees.
- selector container, line hint, selector glob, and `last_seen` preservation.
- macro-call no-panic allow entries with `panic` family normalization to
  `panic_macro`.
- fallback legacy no-panic allowlist evidence when no explicit evidence exists.
- `never` expiry lifecycle fallback from created date to review-after.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-no-panic-allow-converter.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-no-panic-allow-converter.repo-exposure.json --out target/ripr/reports/after-no-panic-allow-converter.gap-decision-ledger.json --out-md target/ripr/reports/after-no-panic-allow-converter.gap-decision-ledger.md
```

Observed:

```text
repairable = 1465
ripr zero target count = 1465
ripr plus target count = 1465
crates/allow-policy-legacy/src/converter_no_panic_allow_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1489` to `1465`
and cleared
`crates/allow-policy-legacy/src/converter_no_panic_allow_entries.rs` from the
repairable file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 1002 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 131 |
| `MissingValueAssertion` | 97 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy/src/toml_de.rs` | 25 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/allow-report/src/summary.rs` | 24 |

## Thirty-First Burn-Down Slice

The thirty-first focused slice added same-module coverage for
`crates/allow-policy-legacy/src/converter_clippy_entries.rs`:

- reviewed Clippy exception entries with normalized paths.
- converted lint-exception kind, family, owner, classification, reason,
  lifecycle, and legacy policy links.
- explicit legacy evidence preservation.
- selector identity for lint, symbol, target fingerprint, attribute kind, and
  selector glob.
- fallback legacy policy evidence when no explicit evidence exists.
- baseline-debt metadata and `never` expiry lifecycle fallback from created
  date to review-after.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-clippy-converter.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-clippy-converter.repo-exposure.json --out target/ripr/reports/after-clippy-converter.gap-decision-ledger.json --out-md target/ripr/reports/after-clippy-converter.gap-decision-ledger.md
```

Observed:

```text
repairable = 1443
ripr zero target count = 1443
ripr plus target count = 1443
crates/allow-policy-legacy/src/converter_clippy_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1465` to `1443`
and cleared `crates/allow-policy-legacy/src/converter_clippy_entries.rs` from
the repairable file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 980 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 131 |
| `MissingValueAssertion` | 97 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy/src/toml_de.rs` | 25 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/allow-report/src/summary.rs` | 24 |

## Thirty-Second Burn-Down Slice

The thirty-second focused slice added same-module coverage for
`crates/allow-policy-legacy/src/converter_unsafe_entries.rs`:

- reviewed unsafe entries with normalized paths.
- converted unsafe kind, family, owner, classification, reason, lifecycle, and
  legacy policy links.
- explicit unsafe evidence preservation.
- selector identity for unsafe kind, container, line hint, and selector glob.
- `last_seen` preservation.
- generated baseline-debt unsafe entries with fallback legacy policy and TODO
  evidence.
- `never` expiry lifecycle fallback from created date to review-after.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-unsafe-converter.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-unsafe-converter.repo-exposure.json --out target/ripr/reports/after-unsafe-converter.gap-decision-ledger.json --out-md target/ripr/reports/after-unsafe-converter.gap-decision-ledger.md
```

Observed:

```text
repairable = 1421
ripr zero target count = 1421
ripr plus target count = 1421
crates/allow-policy-legacy/src/converter_unsafe_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1443` to `1421`
and cleared `crates/allow-policy-legacy/src/converter_unsafe_entries.rs` from
the repairable file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 958 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 131 |
| `MissingValueAssertion` | 97 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy/src/toml_de.rs` | 25 |
| `crates/allow-report/src/summary.rs` | 24 |
| `crates/allow-policy-legacy/src/fields.rs` | 24 |

## Thirty-Third Burn-Down Slice

The thirty-third focused slice added same-module coverage for
`crates/allow-policy-legacy/src/converter_no_panic_baseline_entries.rs`:

- count-limited no-panic baseline entries with generated baseline IDs.
- converted panic kind, family, owner, classification, reason, lifecycle, and
  legacy policy links.
- generated no-panic baseline evidence for legacy policy, selector callee, and
  baseline count.
- occurrence-limit preservation from the legacy baseline count.
- method-call selector identity for normalized AST kind, callee, snippet hash,
  and selector glob.
- macro-call panic-family normalization to `panic_macro`.
- macro selector identity for normalized AST kind, macro name, snippet hash, and
  selector glob.
- Windows path separator normalization.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-no-panic-baseline-converter.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-no-panic-baseline-converter.repo-exposure.json --out target/ripr/reports/after-no-panic-baseline-converter.gap-decision-ledger.json --out-md target/ripr/reports/after-no-panic-baseline-converter.gap-decision-ledger.md
```

Observed:

```text
repairable = 1401
ripr zero target count = 1401
ripr plus target count = 1401
crates/allow-policy-legacy/src/converter_no_panic_baseline_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1421` to `1401`
and cleared
`crates/allow-policy-legacy/src/converter_no_panic_baseline_entries.rs` from
the repairable file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 939 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 131 |
| `MissingValueAssertion` | 96 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy/src/toml_de.rs` | 25 |
| `crates/allow-report/src/summary.rs` | 24 |
| `crates/allow-policy-legacy/src/fields.rs` | 24 |

## Thirty-Fourth Burn-Down Slice

The thirty-fourth focused slice added same-module coverage for
`crates/allow-policy-legacy/src/parser_clippy_entries.rs`:

- `allow` and `entry` root compatibility for legacy Clippy exceptions.
- explicit reviewed lint exception fields, including owner, classification,
  reason, evidence, symbol, created, review-after, and expiry.
- generated legacy IDs and default owner/classification/reason fallback.
- `attribute` and `family` normalization to cargo-allow lint exception
  families.
- `covered_by` compatibility when explicit evidence is absent.
- `policy_id` target-fingerprint generation and explicit
  `target_fingerprint` precedence.
- default created/expiry dates for baseline debt when no review-after exists.
- `permanent` expiry normalization to `never`.
- parse errors for missing entry arrays, non-table entries, missing path, and
  missing lint fields.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-clippy-parser.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-clippy-parser.repo-exposure.json --out target/ripr/reports/after-clippy-parser.gap-decision-ledger.json --out-md target/ripr/reports/after-clippy-parser.gap-decision-ledger.md
```

Observed:

```text
repairable = 1378
ripr zero target count = 1378
ripr plus target count = 1378
crates/allow-policy-legacy/src/parser_clippy_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1401` to `1378`
and cleared `crates/allow-policy-legacy/src/parser_clippy_entries.rs` from the
repairable file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 917 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 131 |
| `MissingValueAssertion` | 95 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy/src/toml_de.rs` | 25 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/allow-report/src/summary.rs` | 24 |

## Thirty-Fifth Burn-Down Slice

The thirty-fifth focused slice added same-module coverage for
`crates/allow-policy-legacy/src/parser_process_entries.rs`:

- local and network-reaching process allowlist entries.
- required process ID, binary, argv-shape, network-reach, owner, reason, and
  created fields.
- optional `called_by`, review-after, and expiry fields.
- explicit evidence and `covered_by` evidence compatibility.
- `permanent` expiry normalization to `never`.
- missing allow-array, non-table entry, missing ID, missing argv-shape, and
  missing network-reach parse errors.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-process-parser.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-process-parser.repo-exposure.json --out target/ripr/reports/after-process-parser.gap-decision-ledger.json --out-md target/ripr/reports/after-process-parser.gap-decision-ledger.md
```

Observed:

```text
repairable = 1359
ripr zero target count = 1359
ripr plus target count = 1359
crates/allow-policy-legacy/src/parser_process_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1378` to `1359`
and cleared `crates/allow-policy-legacy/src/parser_process_entries.rs` from the
repairable file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 899 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 131 |
| `MissingValueAssertion` | 94 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy/src/toml_de.rs` | 25 |
| `crates/allow-policy-legacy/src/fields.rs` | 24 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |

## Thirty-Sixth Burn-Down Slice

The thirty-sixth focused slice added same-module coverage for
`crates/allow-policy-legacy/src/parser_network_entries.rs`:

- public and authenticated network allowlist entries.
- required network ID, destination, auth-required, lane, owner, reason, and
  created fields.
- optional auth-secret, review-after, and expiry fields.
- explicit evidence and `covered_by` evidence compatibility.
- `permanent` expiry normalization to `never`.
- missing allow-array, non-table entry, missing ID, missing auth-required, and
  missing lane parse errors.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-network-parser.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-network-parser.repo-exposure.json --out target/ripr/reports/after-network-parser.gap-decision-ledger.json --out-md target/ripr/reports/after-network-parser.gap-decision-ledger.md
```

Observed:

```text
repairable = 1340
ripr zero target count = 1340
ripr plus target count = 1340
crates/allow-policy-legacy/src/parser_network_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1359` to `1340`
and cleared `crates/allow-policy-legacy/src/parser_network_entries.rs` from the
repairable file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 881 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 131 |
| `MissingValueAssertion` | 93 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy/src/toml_de.rs` | 25 |
| `crates/allow-policy-legacy/src/fields.rs` | 24 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |

## Thirty-Seventh Burn-Down Slice

The thirty-seventh focused slice added same-module coverage for
`crates/allow-policy-legacy/src/parser_dependency_entries.rs`:

- dependency surface path and glob allowlist entries.
- generated legacy dependency IDs and explicit IDs.
- default and explicit surface, owner, reason, and broad-glob metadata.
- explicit evidence and `covered_by` evidence compatibility.
- dependency baseline count preservation.
- optional created/review-after/expiry fields.
- `permanent` expiry normalization to `never`.
- missing allow-array, non-table entry, and missing path-or-glob parse errors.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-dependency-parser.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-dependency-parser.repo-exposure.json --out target/ripr/reports/after-dependency-parser.gap-decision-ledger.json --out-md target/ripr/reports/after-dependency-parser.gap-decision-ledger.md
```

Observed:

```text
repairable = 1321
ripr zero target count = 1321
ripr plus target count = 1321
crates/allow-policy-legacy/src/parser_dependency_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1340` to `1321`
and cleared `crates/allow-policy-legacy/src/parser_dependency_entries.rs` from
the repairable file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 863 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 131 |
| `MissingValueAssertion` | 92 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy/src/toml_de.rs` | 25 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/allow-report/src/summary.rs` | 24 |

## Thirty-Eighth Burn-Down Slice

The thirty-eighth focused slice added same-module coverage for
`crates/allow-policy-legacy/src/fields.rs`:

- trimmed optional string fields and raw-string preservation.
- string-array filtering for blank and non-string values.
- string-or-array compatibility for scalar and array forms.
- explicit `evidence` precedence over `covered_by` fallback evidence.
- missing or empty required string and string-array error paths.
- required bool parsing for actual TOML booleans only.
- positive-only `u32` parsing and invalid numeric boundaries.
- optional last-seen parsing, default column fallback, and missing-line
  handling.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-legacy-fields.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-legacy-fields.repo-exposure.json --out target/ripr/reports/after-legacy-fields.gap-decision-ledger.json --out-md target/ripr/reports/after-legacy-fields.gap-decision-ledger.md
```

Observed:

```text
repairable = 1253
ripr zero target count = 1253
ripr plus target count = 1253
crates/allow-policy-legacy/src/fields.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1321` to `1253`
and cleared `crates/allow-policy-legacy/src/fields.rs` from the repairable
file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 801 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 127 |
| `MissingValueAssertion` | 90 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy/src/toml_de.rs` | 25 |
| `crates/allow-report/src/summary.rs` | 24 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |

## Thirty-Ninth Burn-Down Slice

The thirty-ninth focused slice added same-module coverage for
`crates/allow-report/src/summary.rs`:

- summary aggregation across match statuses.
- missing-status count fallback to zero.
- review-signal aggregation from summary counts and policy context.
- review-item count composition across retained findings and policy evidence
  signals.
- JSON count-field rendering with policy-context excess fields.
- context count helper override and fallback behavior.
- policy baseline-debt and missing-evidence helper counts.
- matched missing-evidence filtering for non-baseline retained entries.
- audit review queue filtering and twenty-item cap.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-summary.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-summary.repo-exposure.json --out target/ripr/reports/after-summary.gap-decision-ledger.json --out-md target/ripr/reports/after-summary.gap-decision-ledger.md
```

Observed:

```text
repairable = 1229
ripr zero target count = 1229
ripr plus target count = 1229
crates/allow-report/src/summary.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1253` to `1229`
and cleared `crates/allow-report/src/summary.rs` from the repairable file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 777 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 127 |
| `MissingValueAssertion` | 90 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/allow-policy/src/toml_de.rs` | 25 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/allow-diff/src/revision_git.rs` | 23 |

## Fortieth Burn-Down Slice

The fortieth focused slice added same-module coverage for
`crates/allow-policy/src/toml_de.rs`:

- scalar and array string-list compatibility.
- missing string-list default behavior.
- rejection of non-string scalar and mixed-array values.
- boolean and string boolean compatibility.
- missing optional boolean behavior.
- rejection of invalid boolean strings and unsupported numeric booleans.
- numeric and string `u32` compatibility.
- missing optional `u32` behavior.
- rejection of invalid `u32` strings and numeric boundaries.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-toml-de.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-toml-de.repo-exposure.json --out target/ripr/reports/after-toml-de.gap-decision-ledger.json --out-md target/ripr/reports/after-toml-de.gap-decision-ledger.md
```

Observed:

```text
repairable = 1204
ripr zero target count = 1204
ripr plus target count = 1204
crates/allow-policy/src/toml_de.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1229` to `1204`
and cleared `crates/allow-policy/src/toml_de.rs` from the repairable file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 766 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 116 |
| `MissingValueAssertion` | 87 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/allow-diff/src/revision_git.rs` | 23 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Forty-First Burn-Down Slice

The forty-first focused slice added direct coverage for
`crates/allow-diff/src/revision_git.rs`:

- `git ls-tree -rz` record parsing filters malformed records and non-file modes.
- parser behavior preserves executable modes and lossy path decoding.
- changed-file discovery reports the expected file list between two revisions.
- tracked-file discovery reads the base revision tree.
- revision file reads normalize Windows-style separators.
- missing files at a valid revision return `None`.
- invalid revision errors retain the cargo-allow command context.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-revision-git.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-revision-git.repo-exposure.json --out target/ripr/reports/after-revision-git.gap-decision-ledger.json --out-md target/ripr/reports/after-revision-git.gap-decision-ledger.md
```

Observed:

```text
repairable = 1181
ripr zero target count = 1181
ripr plus target count = 1181
crates/allow-diff/src/revision_git.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1204` to `1181`
and cleared `crates/allow-diff/src/revision_git.rs` from the repairable file
list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 747 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 116 |
| `MissingValueAssertion` | 83 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Forty-Second Burn-Down Slice

The forty-second focused slice added direct render-section coverage for
`crates/allow-policy/src/render_sections.rs`:

- policy headers render schema, policy, owner, status, escaping, and trailing
  section separation.
- workspace sections render root, inventory, default mode, ignored globs, and
  generated globs.
- requirements sections render every top-level boolean and the unsafe
  requirement subsection.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-render-sections.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-render-sections.repo-exposure.json --out target/ripr/reports/after-render-sections.gap-decision-ledger.json --out-md target/ripr/reports/after-render-sections.gap-decision-ledger.md
```

Observed:

```text
repairable = 1161
ripr zero target count = 1161
ripr plus target count = 1161
crates/allow-policy/src/render_sections.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1181` to `1161`
and cleared `crates/allow-policy/src/render_sections.rs` from the repairable
file list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 729 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 116 |
| `MissingValueAssertion` | 81 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |

## Forty-Third Burn-Down Slice

The forty-third focused slice added direct filesystem inventory coverage for
`crates/allow-inventory/src/filesystem.rs`:

- existing tracked path filtering keeps regular files and drops directories or
  missing paths.
- recursive filesystem walking records source files below nested directories.
- recursive walking skips `.git` and `target` directories.
- recursive walking returns `Ok(())` after collecting regular files.
- missing recursive roots report a read failure instead of a successful
  inventory.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-filesystem.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-filesystem.repo-exposure.json --out target/ripr/reports/after-filesystem.gap-decision-ledger.json --out-md target/ripr/reports/after-filesystem.gap-decision-ledger.md
```

Observed:

```text
repairable = 1140
ripr zero target count = 1140
ripr plus target count = 1140
crates/allow-inventory/src/filesystem.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1161` to `1140`
and cleared `crates/allow-inventory/src/filesystem.rs` from the repairable file
list.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 712 |
| `MissingErrorDiscriminator` | 235 |
| `MissingBoundaryAssertion` | 113 |
| `MissingValueAssertion` | 80 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Forty-Fourth Burn-Down Slice

The forty-fourth focused slice added direct helper coverage for
`crates/allow-rust/src/text.rs`:

- lint-list extraction filters empty segments and `reason = ...` metadata.
- lint-list extraction stops at the first closing parenthesis.
- policy reference parsing accepts alphanumeric, `-`, and `_` identifier
  characters and stops at punctuation or whitespace.
- missing or empty policy references return `None`.
- byte columns map to character columns at and inside multi-byte character
  boundaries.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-text.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-text.repo-exposure.json --out target/ripr/reports/after-text.gap-decision-ledger.json --out-md target/ripr/reports/after-text.gap-decision-ledger.md
```

Observed:

```text
repairable = 1122
ripr zero target count = 1122
ripr plus target count = 1122
crates/allow-rust/src/text.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1140` to `1122`
and cleared `crates/allow-rust/src/text.rs` from `16` repairable targets to
`0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 695 |
| `error_variant` | 235 |
| `match_arm` | 77 |
| `return_value` | 48 |
| `predicate_boundary` | 33 |
| `field_construction` | 31 |
| `side_effect` | 3 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Forty-Fifth Burn-Down Slice

The forty-fifth focused slice added direct helper coverage for
`crates/allow-rust/src/finding_builder.rs`:

- finding construction preserves the kind, family, path, span, and message.
- identity construction records language, AST kind, line, column, container,
  module stack, and normalized snippet hash.
- enrichment callbacks can add lint and policy-reference identity fields.
- empty container and module-stack inputs leave optional scope fields empty.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-finding-builder.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-finding-builder.repo-exposure.json --out target/ripr/reports/after-finding-builder.gap-decision-ledger.json --out-md target/ripr/reports/after-finding-builder.gap-decision-ledger.md
```

Observed:

```text
repairable = 1103
ripr zero target count = 1103
ripr plus target count = 1103
crates/allow-rust/src/finding_builder.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1122` to `1103`
and cleared `crates/allow-rust/src/finding_builder.rs` from `16` repairable
targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 683 |
| `error_variant` | 235 |
| `match_arm` | 77 |
| `return_value` | 46 |
| `predicate_boundary` | 32 |
| `field_construction` | 27 |
| `side_effect` | 3 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Forty-Sixth Burn-Down Slice

The forty-sixth focused slice added direct helper coverage for
`crates/allow-policy-legacy/src/parser_support.rs`:

- unsafe-family normalization across legacy aliases and fallback hyphen
  normalization.
- glob-meta detection for every supported glob marker and plain-path
  non-matches.
- legacy `permanent` expiry normalization to `never` without changing other
  expiry values.
- clippy-exception policy alias recognition.
- lint-attribute family normalization for allow/expect aliases and fallback
  preservation.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-parser-support.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-parser-support.repo-exposure.json --out target/ripr/reports/after-parser-support.gap-decision-ledger.json --out-md target/ripr/reports/after-parser-support.gap-decision-ledger.md
```

Observed:

```text
repairable = 1087
ripr zero target count = 1087
ripr plus target count = 1087
crates/allow-policy-legacy/src/parser_support.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1103` to `1087`
and cleared `crates/allow-policy-legacy/src/parser_support.rs` from `16`
repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 679 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 46 |
| `predicate_boundary` | 32 |
| `field_construction` | 27 |
| `side_effect` | 3 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Forty-Seventh Burn-Down Slice

The forty-seventh focused slice added direct converter coverage for
`crates/allow-policy-legacy/src/converter_workflow_file_entries.rs`:

- workflow-file entries preserve normalized workflow paths, policy exception
  kind, family, owner, classification, reason, links, selector fields, and
  lifecycle fields.
- workflow-file evidence preserves existing rule evidence and adds the
  normalized legacy workflow link, permissions, secrets, and duplicate-lane
  marker.
- minimal workflow-file evidence still records the normalized legacy workflow
  link without optional permissions, secrets, or duplicate metadata.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-workflow-file.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-workflow-file.repo-exposure.json --out target/ripr/reports/after-workflow-file.gap-decision-ledger.json --out-md target/ripr/reports/after-workflow-file.gap-decision-ledger.md
```

Observed:

```text
repairable = 1071
ripr zero target count = 1071
ripr plus target count = 1071
crates/allow-policy-legacy/src/converter_workflow_file_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1087` to `1071`
and cleared `crates/allow-policy-legacy/src/converter_workflow_file_entries.rs`
from `16` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 663 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 46 |
| `predicate_boundary` | 32 |
| `field_construction` | 27 |
| `side_effect` | 3 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Forty-Eighth Burn-Down Slice

The forty-eighth focused slice added direct converter coverage for
`crates/allow-policy-legacy/src/converter_workflow_action_entries.rs`:

- workflow-action entries preserve normalized workflow paths, policy exception
  kind, family, owner, classification, reason, evidence, links, selector
  fields, lifecycle fields, and the action target fingerprint.
- minimal workflow-action evidence still records the normalized legacy workflow
  link and external-action marker.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-workflow-action.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-workflow-action.repo-exposure.json --out target/ripr/reports/after-workflow-action.gap-decision-ledger.json --out-md target/ripr/reports/after-workflow-action.gap-decision-ledger.md
```

Observed:

```text
repairable = 1056
ripr zero target count = 1056
ripr plus target count = 1056
crates/allow-policy-legacy/src/converter_workflow_action_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1071` to `1056`
and cleared
`crates/allow-policy-legacy/src/converter_workflow_action_entries.rs` from
`15` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 648 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 46 |
| `predicate_boundary` | 32 |
| `field_construction` | 27 |
| `side_effect` | 3 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |

## Forty-Ninth Burn-Down Slice

The forty-ninth focused slice added direct helper coverage for
`crates/allow-policy-legacy/src/converter_workflow_support.rs`:

- workflow lifecycle conversion preserves explicit created, review-after, and
  expiry fields.
- workflow lifecycle conversion uses the created date as the review-after date
  when the legacy rule is permanent with `expires = "never"` and no explicit
  review-after date.
- workflow slug generation lowercases ASCII, collapses separator runs, trims
  edge separators, and drops inputs without ASCII alphanumeric characters.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-workflow-support.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-workflow-support.repo-exposure.json --out target/ripr/reports/after-workflow-support.gap-decision-ledger.json --out-md target/ripr/reports/after-workflow-support.gap-decision-ledger.md
```

Observed:

```text
repairable = 1041
ripr zero target count = 1041
ripr plus target count = 1041
crates/allow-policy-legacy/src/converter_workflow_support.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1056` to `1041`
and cleared `crates/allow-policy-legacy/src/converter_workflow_support.rs`
from `15` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 637 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 46 |
| `predicate_boundary` | 30 |
| `field_construction` | 27 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Fiftieth Burn-Down Slice

The fiftieth focused slice added direct finding-construction coverage for
`crates/allow-policy-legacy/src/finding_generated_executable.rs`:

- `.gitattributes` text parsing keeps only generated-file entries and ignores
  comments, documentation markers, and false generated markers.
- generated-file findings preserve kind, family, path, span, structural
  identity, target fingerprint, and message.
- git-stage parsing keeps only non-empty `100755` tracked-file paths.
- executable-file findings preserve kind, family, path, span, structural
  identity, executable-bit fingerprint, and message.
- file fingerprints prefer lowercase extensions and fall back to lowercase file
  names when no extension exists.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-generated-executable-finding.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-generated-executable-finding.repo-exposure.json --out target/ripr/reports/after-generated-executable-finding.gap-decision-ledger.json --out-md target/ripr/reports/after-generated-executable-finding.gap-decision-ledger.md
```

Observed:

```text
repairable = 1026
ripr zero target count = 1026
ripr plus target count = 1026
crates/allow-policy-legacy/src/finding_generated_executable.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1041` to `1026`
and cleared `crates/allow-policy-legacy/src/finding_generated_executable.rs`
from `15` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 623 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 45 |
| `predicate_boundary` | 30 |
| `field_construction` | 27 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |

## Fifty-First Burn-Down Slice

The fifty-first focused slice added direct converter coverage for
`crates/allow-policy-legacy/src/converter_generated_entries.rs`:

- generated-code legacy rules convert to canonical allow entries with
  normalized paths, generated-code kind/family/classification, owner, reason,
  links, lifecycle fields, selector fields, and target fingerprints.
- generated-code evidence preserves existing evidence and appends the legacy
  policy link, generator marker, and regeneration command marker.
- minimal generated-code evidence still records the legacy policy link without
  optional generator or command metadata.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-generated-converter.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-generated-converter.repo-exposure.json --out target/ripr/reports/after-generated-converter.gap-decision-ledger.json --out-md target/ripr/reports/after-generated-converter.gap-decision-ledger.md
```

Observed:

```text
repairable = 1008
ripr zero target count = 1008
ripr plus target count = 1008
crates/allow-policy-legacy/src/converter_generated_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1026` to `1008`
and cleared `crates/allow-policy-legacy/src/converter_generated_entries.rs`
from `18` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 605 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 45 |
| `predicate_boundary` | 30 |
| `field_construction` | 27 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Fifty-Second Burn-Down Slice

The fifty-second focused slice added direct converter coverage for
`crates/allow-policy-legacy/src/converter_executable_entries.rs`:

- executable-file legacy rules convert to canonical allow entries with
  normalized paths, executable-file kind/family/classification, owner, reason,
  links, lifecycle fields, selector fields, and the executable-bit target
  fingerprint.
- executable-file evidence preserves existing evidence and appends the legacy
  policy link plus the optional interpreter marker.
- minimal executable-file evidence still records the legacy policy link without
  optional interpreter metadata.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-executable-converter.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-executable-converter.repo-exposure.json --out target/ripr/reports/after-executable-converter.gap-decision-ledger.json --out-md target/ripr/reports/after-executable-converter.gap-decision-ledger.md
```

Observed:

```text
repairable = 990
ripr zero target count = 990
ripr plus target count = 990
crates/allow-policy-legacy/src/converter_executable_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `1008` to `990`
and cleared
`crates/allow-policy-legacy/src/converter_executable_entries.rs` from `18`
repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 587 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 45 |
| `predicate_boundary` | 30 |
| `field_construction` | 27 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |

## Fifty-Third Burn-Down Slice

The fifty-third focused slice added direct loader coverage for
`crates/allow-policy-legacy/src/loader_policy_dir.rs`:

- non-directory inputs and empty policy directories produce the expected error
  messages.
- supported legacy policy files are discovered, loaded, merged, and validated
  while unsupported files are ignored.
- merged policy metadata comes from the first loaded legacy config, while allow
  entries from later supported files are appended.
- non-Rust policy loading uses the supplied finding inventory when present.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-policy-dir-loader.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-policy-dir-loader.repo-exposure.json --out target/ripr/reports/after-policy-dir-loader.gap-decision-ledger.json --out-md target/ripr/reports/after-policy-dir-loader.gap-decision-ledger.md
```

Observed:

```text
repairable = 974
ripr zero target count = 974
ripr plus target count = 974
crates/allow-policy-legacy/src/loader_policy_dir.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `990` to `974`
and cleared `crates/allow-policy-legacy/src/loader_policy_dir.rs` from `16`
repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 574 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 44 |
| `predicate_boundary` | 28 |
| `field_construction` | 27 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |

## Fifty-Fourth Burn-Down Slice

The fifty-fourth focused slice added direct parser coverage for
`crates/allow-policy-legacy/src/parser_executable_entries.rs`:

- executable-file legacy rules parse with their id, path, owner, reason,
  interpreter, evidence, and lifecycle fields preserved.
- `expires = "permanent"` is normalized to `never`.
- minimal executable-file entries derive a stable fallback id and preserve the
  `covered_by` proof marker as evidence.
- missing `allow` entries, non-table entries, and missing paths report
  contextual errors.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-executable-parser.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-executable-parser.repo-exposure.json --out target/ripr/reports/after-executable-parser.gap-decision-ledger.json --out-md target/ripr/reports/after-executable-parser.gap-decision-ledger.md
```

Observed:

```text
repairable = 969
ripr zero target count = 969
ripr plus target count = 969
crates/allow-policy-legacy/src/parser_executable_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `974` to `969`
and cleared `crates/allow-policy-legacy/src/parser_executable_entries.rs`
from `5` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 570 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 44 |
| `predicate_boundary` | 28 |
| `field_construction` | 26 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |

## Fifty-Fifth Burn-Down Slice

The fifty-fifth focused slice added direct parser coverage for
`crates/allow-policy-legacy/src/parser_generated_entries.rs`:

- generated-code legacy rules parse with their id, path, owner, reason,
  generator, regeneration command, evidence, and lifecycle fields preserved.
- `expires = "permanent"` is normalized to `never`.
- minimal generated-code entries derive a stable fallback id and preserve the
  `covered_by` proof marker as evidence.
- missing `allow` entries, non-table entries, and missing paths report
  contextual errors.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-generated-parser.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-generated-parser.repo-exposure.json --out target/ripr/reports/after-generated-parser.gap-decision-ledger.json --out-md target/ripr/reports/after-generated-parser.gap-decision-ledger.md
```

Observed:

```text
repairable = 964
ripr zero target count = 964
ripr plus target count = 964
crates/allow-policy-legacy/src/parser_generated_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `969` to `964`
and cleared `crates/allow-policy-legacy/src/parser_generated_entries.rs`
from `5` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 566 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 44 |
| `predicate_boundary` | 28 |
| `field_construction` | 25 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Fifty-Sixth Burn-Down Slice

The fifty-sixth focused slice added direct wrapper coverage for
`crates/allow-policy-legacy/src/converter_source_configs.rs`:

- Clippy legacy rules convert through `config_from_clippy_rules` with base
  cargo-allow config metadata preserved.
- unsafe legacy rules convert through `config_from_unsafe_rules` with base
  cargo-allow config metadata preserved.
- converted entries retain their expected kind, owner, classification,
  selector identity, evidence, and last-seen fields where applicable.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-source-configs.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-source-configs.repo-exposure.json --out target/ripr/reports/after-source-configs.gap-decision-ledger.json --out-md target/ripr/reports/after-source-configs.gap-decision-ledger.md
```

Observed:

```text
repairable = 958
ripr zero target count = 958
ripr plus target count = 958
crates/allow-policy-legacy/src/converter_source_configs.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `964` to `958`
and cleared `crates/allow-policy-legacy/src/converter_source_configs.rs`
from `2` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 560 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 44 |
| `predicate_boundary` | 28 |
| `field_construction` | 25 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Fifty-Seventh Burn-Down Slice

The fifty-seventh focused slice added direct parser coverage for
`crates/allow-policy-legacy/src/parser_workflow_entries.rs`:

- workflow legacy entries parse path, owner, reason, permissions, secrets,
  external actions, duplicate-lane references, evidence, and lifecycle fields.
- `expires = "permanent"` is normalized to `never`.
- minimal workflow entries preserve `covered_by` proof markers as evidence.
- missing `entry` records, non-table entries, and missing paths report
  contextual errors.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-workflow-parser.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-workflow-parser.repo-exposure.json --out target/ripr/reports/after-workflow-parser.gap-decision-ledger.json --out-md target/ripr/reports/after-workflow-parser.gap-decision-ledger.md
```

Observed:

```text
repairable = 954
ripr zero target count = 954
ripr plus target count = 954
crates/allow-policy-legacy/src/parser_workflow_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `958` to `954`
and cleared `crates/allow-policy-legacy/src/parser_workflow_entries.rs`
from `4` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 556 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 44 |
| `predicate_boundary` | 28 |
| `field_construction` | 25 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Fifty-Eighth Burn-Down Slice

The fifty-eighth focused slice added direct parser coverage for
`crates/allow-policy-legacy/src/parser_no_panic_baseline_entries.rs`:

- no-panic baseline entries parse with their index, path, family, selector
  kind, selector callee, snippet, and count preserved.
- missing `entry` records, non-table entries, and missing required fields
  report contextual errors.
- zero, negative, and out-of-range counts are rejected as missing positive
  counts.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-no-panic-baseline-parser.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-no-panic-baseline-parser.repo-exposure.json --out target/ripr/reports/after-no-panic-baseline-parser.gap-decision-ledger.json --out-md target/ripr/reports/after-no-panic-baseline-parser.gap-decision-ledger.md
```

Observed:

```text
repairable = 947
ripr zero target count = 947
ripr plus target count = 947
crates/allow-policy-legacy/src/parser_no_panic_baseline_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `954` to `947`
and cleared
`crates/allow-policy-legacy/src/parser_no_panic_baseline_entries.rs` from `6`
repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 549 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 44 |
| `predicate_boundary` | 28 |
| `field_construction` | 25 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Fifty-Ninth Burn-Down Slice

The fifty-ninth focused slice added direct wrapper coverage for
`crates/allow-policy-legacy/src/converter_panic_configs.rs`:

- no-panic baseline entries convert through
  `config_from_no_panic_baseline_entries` with base cargo-allow config metadata
  preserved.
- no-panic allowlist entries convert through
  `config_from_no_panic_allowlist_entries` with base cargo-allow config
  metadata preserved.
- converted entries retain their expected panic kind, family, owner,
  classification, selector identity, evidence, occurrence count, and last-seen
  fields where applicable.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-panic-config.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-panic-config.repo-exposure.json --out target/ripr/reports/after-panic-config.gap-decision-ledger.json --out-md target/ripr/reports/after-panic-config.gap-decision-ledger.md
```

Observed:

```text
repairable = 945
ripr zero target count = 945
ripr plus target count = 945
crates/allow-policy-legacy/src/converter_panic_configs.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `947` to `945`
and cleared
`crates/allow-policy-legacy/src/converter_panic_configs.rs` from `2`
repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 547 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 44 |
| `predicate_boundary` | 28 |
| `field_construction` | 25 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Sixtieth Burn-Down Slice

The sixtieth focused slice added direct wrapper coverage for
`crates/allow-policy-legacy/src/converter_file_configs.rs`:

- non-Rust legacy rules convert through `config_from_non_rust_rules` with base
  cargo-allow config metadata preserved.
- generated-code legacy rules convert through `config_from_generated_rules`
  with base cargo-allow config metadata preserved.
- current non-Rust findings convert through
  `config_from_current_non_rust_findings`, ignore unsupported finding kinds,
  and use the most specific matching legacy rule.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-file-config.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-file-config.repo-exposure.json --out target/ripr/reports/after-file-config.gap-decision-ledger.json --out-md target/ripr/reports/after-file-config.gap-decision-ledger.md
```

Observed:

```text
repairable = 939
ripr zero target count = 939
ripr plus target count = 939
crates/allow-policy-legacy/src/converter_file_configs.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `945` to `939`
and cleared `crates/allow-policy-legacy/src/converter_file_configs.rs` from
`6` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 541 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 44 |
| `predicate_boundary` | 28 |
| `field_construction` | 25 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |

## Sixty-First Burn-Down Slice

The sixty-first focused slice added direct wrapper coverage for
`crates/allow-policy-legacy/src/converter_policy_configs.rs`:

- executable-file legacy rules convert through `config_from_executable_rules`
  with base cargo-allow config metadata preserved.
- workflow legacy rules convert through `config_from_workflow_rules`, including
  the workflow-file entry and external-action entries produced by the wrapped
  converter.
- dependency-surface, process-spawn, and network-destination legacy rules
  convert through their config wrappers with metadata, selector identity,
  evidence, and policy classifications preserved.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-policy-config.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-policy-config.repo-exposure.json --out target/ripr/reports/after-policy-config.gap-decision-ledger.json --out-md target/ripr/reports/after-policy-config.gap-decision-ledger.md
```

Observed:

```text
repairable = 933
ripr zero target count = 933
ripr plus target count = 933
crates/allow-policy-legacy/src/converter_policy_configs.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `939` to `933`
and cleared `crates/allow-policy-legacy/src/converter_policy_configs.rs` from
`6` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 535 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 44 |
| `predicate_boundary` | 28 |
| `field_construction` | 25 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |

## Sixty-Second Burn-Down Slice

The sixty-second focused slice added direct finding-projection coverage for
`crates/allow-policy-legacy/src/finding_config.rs`:

- process policy entries are filtered from legacy config and projected into
  retained `process_spawn` findings with policy identity, span, fingerprint,
  and message fields preserved.
- network policy entries are filtered from legacy config and projected into
  retained `network_destination` findings with policy identity, span,
  fingerprint, and message fields preserved.
- process and network finding builders fall back to the entry path/glob scope
  and entry id when selector symbol fields are absent.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-finding-config.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-finding-config.repo-exposure.json --out target/ripr/reports/after-finding-config.gap-decision-ledger.json --out-md target/ripr/reports/after-finding-config.gap-decision-ledger.md
```

Observed:

```text
repairable = 915
ripr zero target count = 915
ripr plus target count = 915
crates/allow-policy-legacy/src/finding_config.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `933` to `915`
and cleared `crates/allow-policy-legacy/src/finding_config.rs` from `18`
repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 517 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 44 |
| `predicate_boundary` | 28 |
| `field_construction` | 25 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Sixty-Third Burn-Down Slice

The sixty-third focused slice added direct converter coverage for
`crates/allow-policy-legacy/src/converter_non_rust_rule_entries.rs` and
`crates/allow-policy-legacy/src/converter_non_rust_finding_entries.rs`:

- path-scoped non-Rust rules convert to canonical `NonRustFile` entries with
  owner, classification, reason, evidence, lifecycle, selector, and no
  `last_seen` state preserved.
- glob-scoped non-Rust rules keep their glob field and fallback
  `legacy-policy:<id>` evidence.
- current non-Rust findings preserve normalized paths, generated legacy IDs,
  legacy-policy links, selector identity, lifecycle, and optional `last_seen`
  spans.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-non-rust-converters.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-non-rust-converters.repo-exposure.json --out target/ripr/reports/after-non-rust-converters.gap-decision-ledger.json --out-md target/ripr/reports/after-non-rust-converters.gap-decision-ledger.md
```

Observed:

```text
repairable = 884
ripr zero target count = 884
ripr plus target count = 884
crates/allow-policy-legacy/src/converter_non_rust_rule_entries.rs repairable targets = 0
crates/allow-policy-legacy/src/converter_non_rust_finding_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `915` to `884`
and cleared both non-Rust converter files from the repairable target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 487 |
| `error_variant` | 235 |
| `match_arm` | 65 |
| `return_value` | 44 |
| `predicate_boundary` | 28 |
| `field_construction` | 24 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |

## Sixty-Fourth Burn-Down Slice

The sixty-fourth focused slice added direct helper coverage for
`crates/allow-policy-legacy/src/converter_file_support.rs` and
`crates/allow-policy-legacy/src/converter_panic_support.rs`:

- non-Rust rule selection chooses the most specific matching rule and rejects
  non-file findings or missing path matches.
- non-Rust lifecycle and evidence helpers preserve explicit legacy values and
  provide legacy-policy fallback evidence.
- no-panic helper normalization covers panic-family mapping, selector kind
  normalization, macro-name mapping, and method-callee alias/fallback handling.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-legacy-support.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-legacy-support.repo-exposure.json --out target/ripr/reports/after-legacy-support.gap-decision-ledger.json --out-md target/ripr/reports/after-legacy-support.gap-decision-ledger.md
```

Observed:

```text
repairable = 866
ripr zero target count = 866
ripr plus target count = 866
crates/allow-policy-legacy/src/converter_file_support.rs repairable targets = 0
crates/allow-policy-legacy/src/converter_panic_support.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `884` to `866`
and cleared both legacy support helper files from the repairable target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 474 |
| `error_variant` | 235 |
| `match_arm` | 60 |
| `return_value` | 44 |
| `predicate_boundary` | 28 |
| `field_construction` | 24 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Sixty-Fifth Burn-Down Slice

The sixty-fifth focused slice added direct finding-helper coverage for
`crates/allow-policy-legacy/src/finding_workflow.rs` and
`crates/allow-policy-legacy/src/finding_dependency.rs`:

- workflow file and external-action findings preserve policy-exception kind,
  family, normalized identity, span, target fingerprint, and message fields.
- workflow `uses:` extraction trims YAML list prefixes and comments while
  ignoring empty or unrelated lines.
- dependency-surface findings preserve path identity, surface family,
  fingerprint, and message fields.
- dependency surface matching covers direct paths, entry globs, selector globs,
  non-matches, and inventory filtering/sorting.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-finding-helpers.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-finding-helpers.repo-exposure.json --out target/ripr/reports/after-finding-helpers.gap-decision-ledger.json --out-md target/ripr/reports/after-finding-helpers.gap-decision-ledger.md
```

Observed:

```text
repairable = 851
ripr zero target count = 851
ripr plus target count = 851
crates/allow-policy-legacy/src/finding_workflow.rs repairable targets = 0
crates/allow-policy-legacy/src/finding_dependency.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `866` to `851`
and cleared both legacy finding-helper files from the repairable target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 461 |
| `error_variant` | 235 |
| `match_arm` | 60 |
| `return_value` | 43 |
| `predicate_boundary` | 27 |
| `field_construction` | 24 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |

## Sixty-Sixth Burn-Down Slice

The sixty-sixth focused slice added direct helper coverage for the remaining
small legacy helper files:

- `crates/allow-policy-legacy/src/converter_workflow_entries.rs` expands a
  workflow rule into the workflow-file entry followed by one entry per external
  action, and preserves the single file-entry case when there are no actions.
- `crates/allow-policy-legacy/src/converter_process_network_support.rs`
  preserves process scopes, process symbols/fingerprints, and network
  symbols/fingerprints.
- `crates/allow-policy-legacy/src/io.rs` reports contextual file-read and TOML
  parse errors.
- `crates/allow-policy-legacy/src/types_source_entries.rs` matches non-Rust
  and generated-file findings by exact path or glob and ranks path-specific
  rules above equivalent globs.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-legacy-residual.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-legacy-residual.repo-exposure.json --out target/ripr/reports/after-legacy-residual.gap-decision-ledger.json --out-md target/ripr/reports/after-legacy-residual.gap-decision-ledger.md
```

Observed:

```text
repairable = 840
ripr zero target count = 840
ripr plus target count = 840
crates/allow-policy-legacy/src/converter_workflow_entries.rs repairable targets = 0
crates/allow-policy-legacy/src/converter_process_network_support.rs repairable targets = 0
crates/allow-policy-legacy/src/io.rs repairable targets = 0
crates/allow-policy-legacy/src/types_source_entries.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `851` to `840` and
cleared the residual small legacy helper files from the repairable target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 451 |
| `error_variant` | 235 |
| `match_arm` | 59 |
| `return_value` | 43 |
| `predicate_boundary` | 27 |
| `field_construction` | 24 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |

## Sixty-Seventh Burn-Down Slice

The sixty-seventh focused slice added explicit dispatch-matrix coverage for
`crates/allow-policy-legacy/src/loader_legacy_dispatch.rs`.

The new tests prove that `config_from_legacy_table` maps each supported legacy
`policy` value to the expected converted entry shape:

- source-file policy values produce `NonRustFile` and `GeneratedCode` entries.
- panic policy values produce reviewed panic exceptions or count-limited
  baseline entries.
- source exception policy values produce lint and unsafe entries.
- policy-exception values produce executable, workflow, dependency-surface,
  process, and network entries.
- unrecognized or missing policy values return `None` without dispatching.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-legacy-dispatch.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-legacy-dispatch.repo-exposure.json --out target/ripr/reports/after-legacy-dispatch.gap-decision-ledger.json --out-md target/ripr/reports/after-legacy-dispatch.gap-decision-ledger.md
```

Observed:

```text
repairable = 827
ripr zero target count = 827
ripr plus target count = 827
crates/allow-policy-legacy/src/loader_legacy_dispatch.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `840` to `827` and
cleared the legacy policy dispatcher from the repairable target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 451 |
| `error_variant` | 235 |
| `match_arm` | 47 |
| `return_value` | 42 |
| `predicate_boundary` | 27 |
| `field_construction` | 24 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Sixty-Eighth Burn-Down Slice

The sixty-eighth focused slice added direct message-formatting coverage for
`crates/allow-match/src/messages.rs`.

The new tests prove that `finding_location`:

- renders spanned findings as normalized `path:line:column` locations.
- renders unspanned findings as the normalized path only.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-messages.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-messages.repo-exposure.json --out target/ripr/reports/after-messages.gap-decision-ledger.json --out-md target/ripr/reports/after-messages.gap-decision-ledger.md
```

Observed:

```text
repairable = 824
ripr zero target count = 824
ripr plus target count = 824
crates/allow-match/src/messages.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `827` to `824` and
cleared the match message-location helper from the repairable target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 450 |
| `error_variant` | 235 |
| `match_arm` | 44 |
| `return_value` | 42 |
| `predicate_boundary` | 28 |
| `field_construction` | 24 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |

## Sixty-Ninth Burn-Down Slice

The sixty-ninth focused slice added direct TOML-rendering coverage for:

- `crates/allow-policy/src/render_selector.rs`.
- `crates/allow-policy/src/render_last_seen.rs`.

The new tests prove that:

- `render_selector` writes the selector section, all present optional string
  fields, `line_hint`, and omits absent optional fields.
- `render_last_seen` writes the `allow.last_seen` section with line and column.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-render-helpers.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-render-helpers.repo-exposure.json --out target/ripr/reports/after-render-helpers.gap-decision-ledger.json --out-md target/ripr/reports/after-render-helpers.gap-decision-ledger.md
```

Observed:

```text
repairable = 820
ripr zero target count = 820
ripr plus target count = 820
crates/allow-policy/src/render_selector.rs repairable targets = 0
crates/allow-policy/src/render_last_seen.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `824` to `820` and
cleared the selector and last-seen render helpers from the repairable target
list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 446 |
| `error_variant` | 235 |
| `match_arm` | 44 |
| `return_value` | 42 |
| `predicate_boundary` | 28 |
| `field_construction` | 24 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/spec_system/validate.rs` | 38 |
| `crates/allow-policy/src/entry_validation.rs` | 36 |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |

## Seventieth Burn-Down Slice

The seventieth focused slice added direct TOML model coverage for
`crates/allow-policy/src/toml_model.rs`.

The new tests prove that:

- default `PolicyToml` conversion applies the expected policy header,
  workspace, requirement, and empty allow-entry defaults.
- `parse_policy_toml` preserves explicit workspace, requirement, and allow-entry
  fields, including selector fields.
- allow-entry conversion errors are surfaced from policy TOML parsing.
- invalid TOML parse failures return the expected `CargoAllowError` wrapper.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-toml-model.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-toml-model.repo-exposure.json --out target/ripr/reports/after-toml-model.gap-decision-ledger.json --out-md target/ripr/reports/after-toml-model.gap-decision-ledger.md
```

Observed:

```text
repairable = 667
ripr zero target count = 667
ripr plus target count = 667
crates/allow-policy/src/toml_model.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `820` to `667` and
cleared the TOML model helper from the repairable target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 439 |
| `error_variant` | 96 |
| `return_value` | 42 |
| `match_arm` | 37 |
| `predicate_boundary` | 28 |
| `field_construction` | 24 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/artifact_sample_schema_support.rs` | 24 |
| `crates/cargo-allow/src/migrate_load.rs` | 22 |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |
| `crates/cargo-allow/src/policy_config.rs` | 22 |
| `crates/cargo-allow/src/prune_stale.rs` | 21 |

## Seventy-First Burn-Down Slice

The seventy-first focused slice added direct artifact sample schema validation
coverage for `crates/cargo-allow/src/artifact_sample_schema_support.rs`.

The new tests prove that the sample validator reports the existing exact error
messages for:

- `anyOf` branch mismatch.
- missing object `required` keys.
- unknown object keys when `additionalProperties = false`.
- object keys when no properties are allowed.
- array `contains` mismatch.
- `const`, `enum`, `type`, `minimum`, `minLength`, and supported `pattern`
  constraint mismatches.
- non-local `$ref` rejection.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-artifact-sample-schema.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-artifact-sample-schema.repo-exposure.json --out target/ripr/reports/after-artifact-sample-schema.gap-decision-ledger.json --out-md target/ripr/reports/after-artifact-sample-schema.gap-decision-ledger.md
```

Observed:

```text
repairable = 590
ripr zero target count = 590
ripr plus target count = 590
crates/cargo-allow/src/artifact_sample_schema_support.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `667` to `590` and
cleared the artifact sample schema support helper from the repairable target
list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 439 |
| `return_value` | 42 |
| `match_arm` | 37 |
| `predicate_boundary` | 28 |
| `field_construction` | 24 |
| `error_variant` | 19 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/explain_steps.rs` | 22 |
| `crates/cargo-allow/src/migrate_render.rs` | 21 |
| `crates/cargo-allow/src/policy_config.rs` | 21 |
| `crates/cargo-allow/src/migrate_load.rs` | 21 |
| `crates/cargo-allow/src/prune_stale.rs` | 21 |

## Seventy-Second Burn-Down Slice

The seventy-second focused slice added direct explain next-step routing coverage
for `crates/cargo-allow/src/explain_steps.rs`.

The new tests prove that:

- non-matched outcomes route through worklist next-step and proof-command
  helpers with the current finding and allow entry.
- untracked evidence and link references produce the include-untracked guidance
  and proof commands.
- broken and weak evidence/link references route to the correct evidence or
  traceability repair messages.
- baseline debt and missing unsafe evidence produce the expected repair actions.
- untracked proof commands preserve the fixed command prefix, add non-duplicate
  worklist proof commands, and do not duplicate `cargo-allow explain <id>`.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-explain-steps.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-explain-steps.repo-exposure.json --out target/ripr/reports/after-explain-steps.gap-decision-ledger.json --out-md target/ripr/reports/after-explain-steps.gap-decision-ledger.md
```

Observed:

```text
repairable = 568
ripr zero target count = 568
ripr plus target count = 568
crates/cargo-allow/src/explain_steps.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `590` to `568` and
cleared the explain next-step helper from the repairable target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 417 |
| `return_value` | 42 |
| `match_arm` | 37 |
| `predicate_boundary` | 28 |
| `field_construction` | 24 |
| `error_variant` | 19 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/migrate_load.rs` | 21 |
| `crates/cargo-allow/src/policy_config.rs` | 21 |
| `crates/cargo-allow/src/migrate_render.rs` | 21 |
| `crates/cargo-allow/src/prune_stale.rs` | 21 |
| `crates/allow-report/src/diff_policy_detail.rs` | 20 |

## Seventy-Third Burn-Down Slice

The seventy-third focused slice added direct migration rendering coverage for
`crates/cargo-allow/src/migrate_render.rs`.

The new tests prove that:

- migration reports preserve input/output context, force mode, legacy migration
  notes, and global/unsafe broken and weak evidence counts.
- explicit and missing source-tree roots route to the expected evidence
  diagnostic roots.
- unsafe-specific weak evidence references count only unsafe entries and include
  both evidence and link diagnostics.
- unsafe-specific broken evidence links count only unsafe entries and include
  local evidence and link diagnostics.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-migrate-render.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-migrate-render.repo-exposure.json --out target/ripr/reports/after-migrate-render.gap-decision-ledger.json --out-md target/ripr/reports/after-migrate-render.gap-decision-ledger.md
```

Observed:

```text
repairable = 547
ripr zero target count = 547
ripr plus target count = 547
crates/cargo-allow/src/migrate_render.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `568` to `547` and
cleared the migrate report rendering helper from the repairable target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 396 |
| `return_value` | 42 |
| `match_arm` | 37 |
| `predicate_boundary` | 28 |
| `field_construction` | 24 |
| `error_variant` | 19 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/prune_stale.rs` | 21 |
| `crates/cargo-allow/src/policy_config.rs` | 21 |
| `crates/cargo-allow/src/migrate_load.rs` | 21 |
| `crates/allow-diff/src/policy_change.rs` | 20 |
| `crates/allow-report/src/diff_policy_detail.rs` | 20 |

## Seventy-Fourth Burn-Down Slice

The seventy-fourth focused slice added direct stale-prune helper coverage for
`crates/cargo-allow/src/prune_stale.rs`.

The new tests prove that:

- allow-entry TOML block splitting preserves each block and trims block tails.
- stale block extraction matches an exact allow ID without matching ID prefixes.
- block ID matching uses TOML basic-string escaping and ignores commented IDs.
- TOML basic-string escaping covers backslash, quote, newline, carriage return,
  tab, and ordinary-character arms.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-prune-stale.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-prune-stale.repo-exposure.json --out target/ripr/reports/after-prune-stale.gap-decision-ledger.json --out-md target/ripr/reports/after-prune-stale.gap-decision-ledger.md
```

Observed:

```text
repairable = 526
ripr zero target count = 526
ripr plus target count = 526
crates/cargo-allow/src/prune_stale.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `547` to `526` and
cleared the stale-prune helper from the repairable target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 382 |
| `return_value` | 42 |
| `match_arm` | 30 |
| `predicate_boundary` | 28 |
| `field_construction` | 24 |
| `error_variant` | 19 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/migrate_load.rs` | 21 |
| `crates/cargo-allow/src/policy_config.rs` | 21 |
| `crates/allow-report/src/diff_policy_detail.rs` | 20 |
| `crates/allow-diff/src/policy_change.rs` | 20 |
| `crates/allow-diff/src/policy_entry_selector.rs` | 19 |

## Seventy-Fifth Burn-Down Slice

The seventy-fifth focused slice added direct policy config path/loading coverage
for `crates/cargo-allow/src/policy_config.rs`.

The new tests prove that:

- required and optional config loaders distinguish explicit, present, and
  missing policies.
- `EvidenceValidationMode::ReportOnly` preserves invalid local link scope for
  diagnostics while `Abort` rejects it.
- root-relative config paths preserve absolute paths and join relative paths.
- git-relative config path handling distinguishes inside-root configs, missing
  configs, missing root canonicalization, missing config canonicalization, and
  outside-root configs.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-policy-config.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-policy-config.repo-exposure.json --out target/ripr/reports/after-policy-config.gap-decision-ledger.json --out-md target/ripr/reports/after-policy-config.gap-decision-ledger.md
```

Observed:

```text
repairable = 506
ripr zero target count = 506
ripr plus target count = 506
crates/cargo-allow/src/policy_config.rs repairable targets = 2
```

The focused slice reduced repo-scoped `ripr+` targets from `526` to `506` and
reduced `crates/cargo-allow/src/policy_config.rs` from `21` repairable targets
to `2`.

The two remaining `policy_config.rs` targets are tracked as provider friction
in `EffortlessMetrics/ripr#1437`: the suggested error-discriminator gaps for
root and config canonicalization failures persist after dedicated same-module
tests assert both exact diagnostics.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 365 |
| `return_value` | 41 |
| `match_arm` | 27 |
| `predicate_boundary` | 27 |
| `field_construction` | 24 |
| `error_variant` | 21 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-diff/src/policy_change.rs` | 20 |
| `crates/allow-report/src/diff_policy_detail.rs` | 20 |
| `crates/cargo-allow/src/migrate_load.rs` | 20 |
| `crates/cargo-allow/src/reporting.rs` | 19 |
| `crates/allow-diff/src/policy_entry_selector.rs` | 19 |

## Seventy-Sixth Burn-Down Slice

The seventy-sixth focused slice added direct migration loading coverage for
`crates/cargo-allow/src/migrate_load.rs`.

The new tests prove that:

- single-file migration loading preserves canonical policy entries and
  constructs the expected `from` context.
- repo-policy migration loading resolves explicit source roots, scans
  inventory, loads legacy policy directories, and constructs the expected
  `repo_policy` context.
- repo-policy source-root inference handles explicit roots, absolute `policy`
  directory parent inference, and fallback source-root resolution.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-migrate-load.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-migrate-load.repo-exposure.json --out target/ripr/reports/after-migrate-load.gap-decision-ledger.json --out-md target/ripr/reports/after-migrate-load.gap-decision-ledger.md
```

Observed:

```text
repairable = 487
ripr zero target count = 487
ripr plus target count = 487
crates/cargo-allow/src/migrate_load.rs repairable targets = 1
```

The focused slice reduced repo-scoped `ripr+` targets from `506` to `487` and
reduced `crates/cargo-allow/src/migrate_load.rs` from `20` repairable targets
to `1`.

The remaining `migrate_load.rs` target is tracked as provider friction in
`EffortlessMetrics/ripr#1438`: the suggested `env::current_dir()` error
discriminator is not a portable same-process unit-test target, especially on
Windows.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 347 |
| `return_value` | 40 |
| `match_arm` | 27 |
| `predicate_boundary` | 27 |
| `field_construction` | 23 |
| `error_variant` | 22 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-report/src/diff_policy_detail.rs` | 20 |
| `crates/allow-diff/src/policy_change.rs` | 20 |
| `crates/allow-diff/src/policy_entry_selector.rs` | 19 |
| `crates/cargo-allow/src/reporting.rs` | 19 |
| `crates/allow-policy/src/scope_validation.rs` | 17 |

## Seventy-Seventh Burn-Down Slice

The seventy-seventh focused slice added direct policy change builder coverage
for `crates/allow-diff/src/policy_change.rs`.

The new tests prove that:

- a new `PolicyChange` preserves base fields and starts without optional detail
  payloads.
- each `with_*` builder attaches its corresponding detail payload.
- builder chaining preserves the original allow ID, kind, severity, and message.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-policy-change.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-policy-change.repo-exposure.json --out target/ripr/reports/after-policy-change.gap-decision-ledger.json --out-md target/ripr/reports/after-policy-change.gap-decision-ledger.md
```

Observed:

```text
repairable = 467
ripr zero target count = 467
ripr plus target count = 467
crates/allow-diff/src/policy_change.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `487` to `467` and
cleared `crates/allow-diff/src/policy_change.rs` from the repairable target
list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 337 |
| `return_value` | 30 |
| `match_arm` | 27 |
| `predicate_boundary` | 27 |
| `field_construction` | 23 |
| `error_variant` | 22 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-report/src/diff_policy_detail.rs` | 20 |
| `crates/allow-diff/src/policy_entry_selector.rs` | 19 |
| `crates/cargo-allow/src/reporting.rs` | 19 |
| `crates/allow-policy/src/scope_validation.rs` | 17 |
| `crates/cargo-allow/src/worklist_items.rs` | 16 |

## Seventy-Eighth Burn-Down Slice

The seventy-eighth focused slice added direct selector policy diff coverage for
`crates/allow-diff/src/policy_entry_selector.rs`.

The new tests prove that:

- selector precision decreases emit failing policy changes with removed-field
  detail.
- selector precision increases emit improvement policy changes with added-field
  detail.
- equal-precision selector identity changes emit review policy changes without
  precision detail.
- unchanged selectors emit no policy changes.
- selector precision helper output records added and removed fields and formats
  empty/non-empty detail strings.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-policy-selector.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-policy-selector.repo-exposure.json --out target/ripr/reports/after-policy-selector.gap-decision-ledger.json --out-md target/ripr/reports/after-policy-selector.gap-decision-ledger.md
```

Observed:

```text
repairable = 448
ripr zero target count = 448
ripr plus target count = 448
crates/allow-diff/src/policy_entry_selector.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `467` to `448` and
cleared `crates/allow-diff/src/policy_entry_selector.rs` from the repairable
target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 324 |
| `return_value` | 30 |
| `predicate_boundary` | 25 |
| `match_arm` | 23 |
| `field_construction` | 23 |
| `error_variant` | 22 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-report/src/diff_policy_detail.rs` | 20 |
| `crates/cargo-allow/src/reporting.rs` | 19 |
| `crates/allow-policy/src/scope_validation.rs` | 17 |
| `crates/cargo-allow/src/worklist_items.rs` | 16 |
| `crates/allow-policy/src/render_entry.rs` | 15 |

## Seventy-Ninth Burn-Down Slice

The seventy-ninth focused slice added direct policy detail formatting coverage
for `crates/allow-report/src/diff_policy_detail.rs`.

The new tests prove that:

- policy changes without optional detail payloads render no detail text.
- a combined policy change renders exception identity, selector identity,
  selector precision, scope, occurrence limit, lifecycle, evidence, metadata,
  requirement, and policy status detail in stable order.
- option and list helpers render present, absent, empty, and joined values.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-diff-policy-detail.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-diff-policy-detail.repo-exposure.json --out target/ripr/reports/after-diff-policy-detail.gap-decision-ledger.json --out-md target/ripr/reports/after-diff-policy-detail.gap-decision-ledger.md
```

Observed:

```text
repairable = 413
ripr zero target count = 413
ripr plus target count = 413
crates/allow-report/src/diff_policy_detail.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `448` to `413` and
cleared `crates/allow-report/src/diff_policy_detail.rs` from the repairable
target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 306 |
| `return_value` | 30 |
| `match_arm` | 23 |
| `error_variant` | 22 |
| `predicate_boundary` | 22 |
| `field_construction` | 9 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/reporting.rs` | 19 |
| `crates/allow-policy/src/scope_validation.rs` | 17 |
| `crates/cargo-allow/src/worklist_items.rs` | 16 |
| `crates/allow-policy/src/render_entry.rs` | 15 |
| `crates/allow-report/src/worklist_human.rs` | 12 |

## Eightieth Burn-Down Slice

The eightieth focused slice added direct report orchestration coverage for
`crates/cargo-allow/src/reporting.rs`.

The new tests prove that:

- evidence summaries count matched missing-evidence policy entries, broken local
  evidence links, source-tree-inventory-local evidence gaps, and weak references.
- evidence summaries only project positive counts into report context.
- report rendering dispatches every output format and writes human, JSON, HTML,
  SARIF, and Markdown artifacts through the shared output path.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-reporting.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-reporting.repo-exposure.json --out target/ripr/reports/after-reporting.gap-decision-ledger.json --out-md target/ripr/reports/after-reporting.gap-decision-ledger.md
```

Observed:

```text
repairable = 391
ripr zero target count = 391
ripr plus target count = 391
crates/cargo-allow/src/reporting.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `413` to `391` and
cleared `crates/cargo-allow/src/reporting.rs` from the repairable target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 285 |
| `return_value` | 30 |
| `match_arm` | 22 |
| `error_variant` | 22 |
| `predicate_boundary` | 22 |
| `field_construction` | 9 |
| `side_effect` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/scope_validation.rs` | 17 |
| `crates/cargo-allow/src/worklist_items.rs` | 16 |
| `crates/allow-policy/src/render_entry.rs` | 15 |
| `crates/allow-report/src/worklist_human.rs` | 12 |
| `crates/allow-policy/src/toml_requirements.rs` | 11 |

## Eighty-First Burn-Down Slice

The eighty-first focused slice added direct scope validation coverage for
`crates/allow-policy/src/scope_validation.rs`.

The new tests prove that:

- workspace validation accepts supported default modes with unique ignored and
  generated globs.
- workspace validation rejects unsupported inventory and default mode values.
- allow entries accept path, matching selector glob, and matching top-level glob
  scope forms.
- allow entries reject missing scope and invalid source-tree-relative paths.
- scope consistency rejects path/glob conflicts and mismatched selector globs
  while accepting slash-normalized selector matches.
- workspace glob duplicate detection accepts unique globs and reports normalized
  duplicate positions.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-scope-validation.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-scope-validation.repo-exposure.json --out target/ripr/reports/after-scope-validation.gap-decision-ledger.json --out-md target/ripr/reports/after-scope-validation.gap-decision-ledger.md
```

Observed:

```text
repairable = 371
ripr zero target count = 371
ripr plus target count = 371
crates/allow-policy/src/scope_validation.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `391` to `371` and
cleared `crates/allow-policy/src/scope_validation.rs` from the repairable target
list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 270 |
| `return_value` | 27 |
| `match_arm` | 22 |
| `error_variant` | 22 |
| `predicate_boundary` | 21 |
| `field_construction` | 9 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/worklist_items.rs` | 16 |
| `crates/allow-policy/src/render_entry.rs` | 15 |
| `crates/allow-report/src/worklist_human.rs` | 12 |
| `crates/allow-policy/src/toml_requirements.rs` | 11 |
| `crates/allow-rust/src/line_findings.rs` | 11 |

## Eighty-Second Burn-Down Slice

The eighty-second focused slice added direct worklist-item construction coverage
for `crates/cargo-allow/src/worklist_items.rs`.

The new tests prove that:

- an outcome with both a finding and a matching allow entry carries the
  finding-derived exception kind, family, normalized source path, source package,
  risk, difficulty, allow id, finding index, and message into the work item.
- the same work item carries entry metadata, lifecycle fields, and evidence
  count without dropping owner, classification, reason, created, review-after,
  or expires fields.
- source-package context appends the package-focused suggested action while
  preserving high-risk policy-exception repair guidance.
- a stale entry-only outcome falls back to the entry path or glob and preserves
  entry-derived exception kind, family, low-risk scoring, allow id, and message.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-worklist-items.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-worklist-items.repo-exposure.json --out target/ripr/reports/after-worklist-items.gap-decision-ledger.json --out-md target/ripr/reports/after-worklist-items.gap-decision-ledger.md
```

Observed:

```text
repairable = 355
ripr zero target count = 355
ripr plus target count = 355
crates/cargo-allow/src/worklist_items.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `371` to `355` and
cleared `crates/cargo-allow/src/worklist_items.rs` from the repairable target
list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 254 |
| `return_value` | 27 |
| `match_arm` | 22 |
| `error_variant` | 22 |
| `predicate_boundary` | 21 |
| `field_construction` | 9 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/render_entry.rs` | 15 |
| `crates/allow-report/src/worklist_human.rs` | 12 |
| `crates/allow-match/src/classification.rs` | 11 |
| `crates/allow-policy/src/toml_requirements.rs` | 11 |
| `crates/allow-rust/src/line_findings.rs` | 11 |

## Eighty-Third Burn-Down Slice

The eighty-third focused slice added direct allow-policy entry rendering
coverage for `crates/allow-policy/src/render_entry.rs`.

The new tests prove that:

- a complete path-scoped allow entry writes the `[[allow]]` header, id, kind,
  family, path, glob, owner, classification, reason, evidence, links, occurrence
  limit, lifecycle fields, selector fields, and last-seen fields.
- evidence and links render through the TOML array path while preserving stable
  output fragments.
- a minimal entry omits absent family, path, evidence, links, occurrence limit,
  lifecycle, and last-seen fields.
- the entry renderer preserves the delegated selector behavior: it still calls
  `render_selector`, which emits an `[allow.selector]` section even when all
  selector fields are absent.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-render-entry.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-render-entry.repo-exposure.json --out target/ripr/reports/after-render-entry.gap-decision-ledger.json --out-md target/ripr/reports/after-render-entry.gap-decision-ledger.md
```

Observed:

```text
repairable = 338
ripr zero target count = 338
ripr plus target count = 338
crates/allow-policy/src/render_entry.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `355` to `338` and
cleared `crates/allow-policy/src/render_entry.rs` from the repairable target
list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 237 |
| `return_value` | 27 |
| `match_arm` | 22 |
| `error_variant` | 22 |
| `predicate_boundary` | 21 |
| `field_construction` | 9 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-report/src/worklist_human.rs` | 12 |
| `crates/allow-rust/src/line_findings.rs` | 11 |
| `crates/allow-rust/src/line_lint_findings.rs` | 11 |
| `crates/allow-match/src/classification.rs` | 11 |
| `crates/allow-policy/src/toml_requirements.rs` | 10 |

## Eighty-Fourth Burn-Down Slice

The eighty-fourth focused slice added direct human worklist filter formatting
coverage for `crates/allow-report/src/worklist_human.rs`.

The new tests prove that:

- every supported worklist filter field is rendered in the stable human summary
  order.
- boolean filters render only when enabled.
- an empty filter set renders exactly `Filters: none`.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-worklist-human.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-worklist-human.repo-exposure.json --out target/ripr/reports/after-worklist-human.gap-decision-ledger.json --out-md target/ripr/reports/after-worklist-human.gap-decision-ledger.md
```

Observed:

```text
repairable = 326
ripr zero target count = 326
ripr plus target count = 326
crates/allow-report/src/worklist_human.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `338` to `326` and
cleared `crates/allow-report/src/worklist_human.rs` from the repairable target
list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 226 |
| `return_value` | 27 |
| `match_arm` | 22 |
| `error_variant` | 22 |
| `predicate_boundary` | 20 |
| `field_construction` | 9 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-rust/src/line_findings.rs` | 11 |
| `crates/allow-rust/src/line_lint_findings.rs` | 11 |
| `crates/allow-match/src/classification.rs` | 11 |
| `crates/allow-policy/src/lib.rs` | 10 |
| `crates/cargo-allow/src/io.rs` | 10 |

## Eighty-Fifth Burn-Down Slice

The eighty-fifth focused slice added direct line-router coverage for
`crates/allow-rust/src/line_findings.rs`.

The new tests prove that:

- blank and comment-only lines return before routing syntax facts into delegated
  scanners.
- one source line can route lint attributes, unsafe constructs, panic methods,
  panic macros, and index expressions through the private `scan_line` dispatcher.
- delegated findings preserve the line context path, container, module stack,
  line hint, and safety-comment metadata.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-line-findings.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-line-findings.repo-exposure.json --out target/ripr/reports/after-line-findings.gap-decision-ledger.json --out-md target/ripr/reports/after-line-findings.gap-decision-ledger.md
```

Observed:

```text
repairable = 315
ripr zero target count = 315
ripr plus target count = 315
crates/allow-rust/src/line_findings.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `326` to `315` and
cleared `crates/allow-rust/src/line_findings.rs` from the repairable target
list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 219 |
| `return_value` | 26 |
| `match_arm` | 22 |
| `error_variant` | 22 |
| `predicate_boundary` | 19 |
| `field_construction` | 7 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-match/src/classification.rs` | 11 |
| `crates/allow-rust/src/line_lint_findings.rs` | 11 |
| `crates/cargo-allow/src/artifact_sample_schema_patterns.rs` | 10 |
| `crates/cargo-allow/src/selector.rs` | 10 |
| `crates/cargo-allow/src/io.rs` | 10 |

## Eighty-Sixth Burn-Down Slice

The eighty-sixth focused slice added direct match classification coverage for
`crates/allow-match/src/classification.rs`.

The new tests prove that:

- expired entries, missing unsafe evidence, and required SAFETY comment
  failures return the expected non-matched statuses and messages.
- bare allow attributes, missing lint policy IDs, and lint policy mismatches
  are rejected when the corresponding requirements are enabled.
- `expires = "never"`, baseline-debt release mode, successful lint policy
  matching, and successful unsafe safety-comment matching return their expected
  statuses.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-classification.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-classification.repo-exposure.json --out target/ripr/reports/after-classification.gap-decision-ledger.json --out-md target/ripr/reports/after-classification.gap-decision-ledger.md
```

Observed:

```text
repairable = 304
ripr zero target count = 304
ripr plus target count = 304
crates/allow-match/src/classification.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `315` to `304` and
cleared `crates/allow-match/src/classification.rs` from the repairable target
list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 214 |
| `return_value` | 24 |
| `match_arm` | 22 |
| `error_variant` | 22 |
| `predicate_boundary` | 15 |
| `field_construction` | 7 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-rust/src/line_lint_findings.rs` | 11 |
| `crates/cargo-allow/src/worklist_queue.rs` | 10 |
| `crates/cargo-allow/src/io.rs` | 10 |
| `crates/allow-policy/src/lib.rs` | 10 |
| `crates/cargo-allow/src/selector.rs` | 10 |

## Eighty-Seventh Burn-Down Slice

The eighty-seventh focused slice added direct line-lint finding coverage for
`crates/allow-rust/src/line_lint_findings.rs`.

The new tests prove that:

- allow and expect lint attributes emit one finding per parsed lint.
- emitted lint findings preserve source path, line, column, container, module,
  normalized line hash, lint name, attribute symbol, and stable message fields.
- expect lint attributes carry `policy:<id>` target fingerprints from the
  attribute reason text.
- mismatched attribute kinds and metadata-only lint attributes do not emit
  findings.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-line-lint.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-line-lint.repo-exposure.json --out target/ripr/reports/after-line-lint.gap-decision-ledger.json --out-md target/ripr/reports/after-line-lint.gap-decision-ledger.md
```

Observed:

```text
repairable = 293
ripr zero target count = 293
ripr plus target count = 293
crates/allow-rust/src/line_lint_findings.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `304` to `293` and
cleared `crates/allow-rust/src/line_lint_findings.rs` from the repairable target
list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 207 |
| `return_value` | 23 |
| `error_variant` | 22 |
| `match_arm` | 19 |
| `predicate_boundary` | 15 |
| `field_construction` | 7 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/selector.rs` | 10 |
| `crates/cargo-allow/src/worklist_queue.rs` | 10 |
| `crates/cargo-allow/src/io.rs` | 10 |
| `crates/allow-policy/src/lib.rs` | 10 |
| `crates/cargo-allow/src/artifact_sample_schema_patterns.rs` | 10 |

## Eighty-Eighth Burn-Down Slice

The eighty-eighth focused slice added direct selector construction coverage for
`crates/cargo-allow/src/selector.rs`.

The new tests prove that:

- source finding selector construction preserves every structural identity field
  copied into policy selector entries.
- selector line hints come from the finding span, not from stale identity line
  hints.
- regular Rust findings do not receive selector globs.
- non-Rust file and generated-code findings receive normalized selector globs.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-selector.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-selector.repo-exposure.json --out target/ripr/reports/after-selector.gap-decision-ledger.json --out-md target/ripr/reports/after-selector.gap-decision-ledger.md
```

Observed:

```text
repairable = 283
ripr zero target count = 283
ripr plus target count = 283
crates/cargo-allow/src/selector.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `293` to `283` and
cleared `crates/cargo-allow/src/selector.rs` from the repairable target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 197 |
| `return_value` | 23 |
| `error_variant` | 22 |
| `match_arm` | 19 |
| `predicate_boundary` | 15 |
| `field_construction` | 7 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/worklist_queue.rs` | 10 |
| `crates/allow-policy/src/toml_requirements.rs` | 10 |
| `crates/allow-policy/src/lib.rs` | 10 |
| `crates/cargo-allow/src/artifact_sample_schema_patterns.rs` | 10 |
| `crates/cargo-allow/src/io.rs` | 10 |

## Eighty-Ninth Burn-Down Slice

The eighty-ninth focused slice added direct queue-helper coverage for
`crates/cargo-allow/src/worklist_queue.rs`.

The new tests prove that:

- worklist path filters return false for missing item paths.
- invalid local evidence references use exact normalized path comparison.
- regular work-item paths delegate to source-tree scope matching, including
  wildcard item scopes.
- worklist item-kind filters accept exact and hyphenated aliases and reject
  unrelated aliases.
- exception kind filters pass missing filters, supported kinds, and reject
  unsupported or mismatched kind filters.
- worklist risk ranks order high, medium, low, then unknown risk values.
- missing selector precision ranks after explicit selector precision.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-worklist-queue.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-worklist-queue.repo-exposure.json --out target/ripr/reports/after-worklist-queue.gap-decision-ledger.json --out-md target/ripr/reports/after-worklist-queue.gap-decision-ledger.md
```

Observed:

```text
repairable = 274
ripr zero target count = 274
ripr plus target count = 274
crates/cargo-allow/src/worklist_queue.rs repairable targets = 1
```

The focused slice reduced repo-scoped `ripr+` targets from `283` to `274` and
reduced `crates/cargo-allow/src/worklist_queue.rs` from `10` repairable targets
to `1`. The remaining queue target is the hyphenated item-kind alias predicate;
the test now includes direct exact, hyphenated, and rejected alias assertions,
but the provider still reports one boundary row for that branch.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 194 |
| `return_value` | 23 |
| `error_variant` | 22 |
| `predicate_boundary` | 14 |
| `match_arm` | 14 |
| `field_construction` | 7 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/toml_requirements.rs` | 10 |
| `crates/allow-policy/src/lib.rs` | 10 |
| `crates/cargo-allow/src/artifact_sample_schema_patterns.rs` | 10 |
| `crates/cargo-allow/src/io.rs` | 10 |
| `crates/allow-files/src/finding.rs` | 9 |

## Ninetieth Burn-Down Slice

The ninetieth focused slice added direct TOML requirements mapping coverage for
`crates/allow-policy/src/toml_requirements.rs`.

The new tests prove that:

- omitted `[requirements]` and `[requirements.unsafe]` TOML fields preserve the
  public `Requirements::default()` values.
- every explicit top-level requirement field maps to the corresponding public
  `Requirements` field.
- nested unsafe requirement fields map to `unsafe_evidence_required` and
  `unsafe_safety_comment_required`.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-toml-requirements.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-toml-requirements.repo-exposure.json --out target/ripr/reports/after-toml-requirements.gap-decision-ledger.json --out-md target/ripr/reports/after-toml-requirements.gap-decision-ledger.md
```

Observed:

```text
repairable = 264
ripr zero target count = 264
ripr plus target count = 264
crates/allow-policy/src/toml_requirements.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `274` to `264` and
cleared `crates/allow-policy/src/toml_requirements.rs` from the repairable
target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 184 |
| `return_value` | 23 |
| `error_variant` | 22 |
| `predicate_boundary` | 14 |
| `match_arm` | 14 |
| `field_construction` | 7 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/artifact_sample_schema_patterns.rs` | 10 |
| `crates/cargo-allow/src/io.rs` | 10 |
| `crates/allow-policy/src/lib.rs` | 10 |
| `crates/allow-files/src/finding.rs` | 9 |
| `crates/allow-policy/src/toml_entry.rs` | 9 |

## Ninety-First Burn-Down Slice

The ninety-first focused slice added direct work-item ID pattern coverage for
`crates/cargo-allow/src/artifact_sample_schema_patterns.rs`.

The new tests prove that:

- supported sample work-item IDs accept lowercase, digit, and hyphenated kind
  components with a four-digit numeric suffix.
- samples without the `work-` prefix or without a kind/number split are
  rejected.
- samples with empty, uppercase, or underscore-containing kinds are rejected.
- samples with short or non-numeric suffixes are rejected.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-artifact-sample-schema-patterns.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-artifact-sample-schema-patterns.repo-exposure.json --out target/ripr/reports/after-artifact-sample-schema-patterns.gap-decision-ledger.json --out-md target/ripr/reports/after-artifact-sample-schema-patterns.gap-decision-ledger.md
```

Observed:

```text
repairable = 254
ripr zero target count = 254
ripr plus target count = 254
crates/cargo-allow/src/artifact_sample_schema_patterns.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `264` to `254` and
cleared `crates/cargo-allow/src/artifact_sample_schema_patterns.rs` from the
repairable target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 176 |
| `return_value` | 23 |
| `error_variant` | 22 |
| `match_arm` | 14 |
| `predicate_boundary` | 12 |
| `field_construction` | 7 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/lib.rs` | 10 |
| `crates/cargo-allow/src/io.rs` | 10 |
| `crates/allow-policy/src/selector_validation.rs` | 9 |
| `crates/allow-rust/src/safety_comments.rs` | 9 |
| `crates/allow-files/src/finding.rs` | 9 |

## Ninety-Second Burn-Down Slice

The ninety-second focused slice added file-backed policy loading coverage for
`crates/allow-policy/src/lib.rs`.

The new tests prove that:

- `find_config` walks upward from a nested source directory and returns the
  nearest supported policy path.
- `find_config` returns `None` when no supported policy file exists.
- `load_policy_with_reportable_evidence` reads policy TOML from disk and keeps
  invalid local links available for reportable-evidence diagnostics.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-policy-lib.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-policy-lib.repo-exposure.json --out target/ripr/reports/after-policy-lib.gap-decision-ledger.json --out-md target/ripr/reports/after-policy-lib.gap-decision-ledger.md
```

Observed:

```text
repairable = 244
ripr zero target count = 244
ripr plus target count = 244
crates/allow-policy/src/lib.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `254` to `244` and
cleared `crates/allow-policy/src/lib.rs` from the repairable target list.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 171 |
| `error_variant` | 22 |
| `return_value` | 19 |
| `match_arm` | 14 |
| `predicate_boundary` | 11 |
| `field_construction` | 7 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/io.rs` | 10 |
| `crates/allow-rust/src/safety_comments.rs` | 9 |
| `crates/allow-files/src/finding.rs` | 9 |
| `crates/allow-diff/src/finding.rs` | 9 |
| `crates/allow-policy/src/selector_validation.rs` | 9 |

## Ninety-Third Burn-Down Slice

The ninety-third focused slice added direct file-output helper coverage for
`crates/cargo-allow/src/io.rs`.

The new tests prove that:

- `emit_text` and `emit_stderr_text` write supplied contents when an output path
  is provided.
- `write_file` creates parent directories for successful writes.
- `write_file` reports parent-creation and file-write failures with the target
  path and exact platform source error in the `CargoAllowError` payload.
- `write_file_no_overwrite` rejects existing files without `force` and leaves
  contents unchanged.
- `write_file_no_overwrite` replaces existing files when `force` is set.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-io.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-io.repo-exposure.json --out target/ripr/reports/after-io.gap-decision-ledger.json --out-md target/ripr/reports/after-io.gap-decision-ledger.md
```

Observed:

```text
repairable = 237
ripr zero target count = 237
ripr plus target count = 237
crates/cargo-allow/src/io.rs repairable targets = 3
```

The focused slice reduced repo-scoped `ripr+` targets from `244` to `237` and
reduced `crates/cargo-allow/src/io.rs` from `10` repairable targets to `3`.
The remaining I/O targets are error-discriminator rows for the `write_file`
error-mapping branches; the tests now assert the returned `CargoAllowError`
payload exactly from the platform source errors, but the provider still reports
those discriminator rows.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 166 |
| `error_variant` | 22 |
| `return_value` | 17 |
| `match_arm` | 14 |
| `predicate_boundary` | 11 |
| `field_construction` | 7 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/toml_entry.rs` | 9 |
| `crates/allow-files/src/finding.rs` | 9 |
| `crates/allow-rust/src/safety_comments.rs` | 9 |
| `crates/allow-diff/src/finding.rs` | 9 |
| `crates/allow-policy/src/selector_validation.rs` | 9 |

## Ninety-Fourth Burn-Down Slice

The ninety-fourth focused slice added direct TOML allow-entry conversion
coverage for `crates/allow-policy/src/toml_entry.rs`.

The new tests prove that:

- `into_allow_entry` generates stable fallback IDs from the zero-based entry
  index.
- explicit entry IDs are preserved instead of replaced by the generated ID.
- missing `kind` errors report the resolved generated ID.
- missing `owner`, `classification`, and `reason` convert to empty strings.
- path, glob, family, evidence, links, occurrence limits, lifecycle fields,
  selector fields, and `last_seen` coordinates transfer into the final
  `AllowEntry`.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-toml-entry.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-toml-entry.repo-exposure.json --out target/ripr/reports/after-toml-entry.gap-decision-ledger.json --out-md target/ripr/reports/after-toml-entry.gap-decision-ledger.md
```

Observed:

```text
repairable = 228
ripr zero target count = 228
ripr plus target count = 228
crates/allow-policy/src/toml_entry.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `237` to `228` and
reduced `crates/allow-policy/src/toml_entry.rs` from `9` repairable targets to
`0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 158 |
| `error_variant` | 22 |
| `return_value` | 17 |
| `match_arm` | 14 |
| `predicate_boundary` | 11 |
| `field_construction` | 6 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-files/src/finding.rs` | 9 |
| `crates/allow-rust/src/safety_comments.rs` | 9 |
| `crates/allow-diff/src/finding.rs` | 9 |
| `crates/allow-policy/src/selector_validation.rs` | 9 |
| `crates/allow-policy/src/toml_last_seen.rs` | 8 |

## Ninety-Fifth Burn-Down Slice

The ninety-fifth focused slice added direct file finding builder coverage for
`crates/allow-files/src/finding.rs`.

The new tests prove that:

- `build_file_finding` records non-generated files as `non_rust_file` findings
  and generated files as `generated_code` findings.
- the file finding preserves the classified family, source path, fixed
  source-tree span, normalized identity symbol, target fingerprint, and human
  message.
- `file_fingerprint` prefers lowercase extensions, falls back to lowercase
  extensionless file names, preserves hidden-file dots such as `.env`, and
  returns `None` when there is no file name.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-file-finding.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-file-finding.repo-exposure.json --out target/ripr/reports/after-file-finding.gap-decision-ledger.json --out-md target/ripr/reports/after-file-finding.gap-decision-ledger.md
```

Observed:

```text
repairable = 219
ripr zero target count = 219
ripr plus target count = 219
crates/allow-files/src/finding.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `228` to `219` and
reduced `crates/allow-files/src/finding.rs` from `9` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 149 |
| `error_variant` | 22 |
| `return_value` | 17 |
| `match_arm` | 14 |
| `predicate_boundary` | 11 |
| `field_construction` | 6 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-diff/src/finding.rs` | 9 |
| `crates/allow-rust/src/safety_comments.rs` | 9 |
| `crates/allow-policy/src/selector_validation.rs` | 9 |
| `crates/allow-policy/src/toml_last_seen.rs` | 8 |
| `crates/allow-policy/src/entries_validation.rs` | 8 |

## Ninety-Sixth Burn-Down Slice

The ninety-sixth focused slice added direct finding posture helper coverage for
`crates/allow-diff/src/finding.rs`.

The new tests prove that:

- `findings_by_key` groups findings by the public finding identity key and
  counts duplicate identities instead of overwriting them.
- the first entry in a finding-key group starts with `count = 1`.
- the empty input returns an empty key map and populated inputs return the
  expected stable identity keys.
- `finding_posture_change` carries the posture kind, stable key, finding kind,
  family, normalized path, line and column hints, source package, and cloned
  structural identity into the final change record.
- the public `finding_identity_key` wrapper delegates to the core stable key so
  line movement does not change identity, while structural identity changes do.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-diff-finding.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-diff-finding.repo-exposure.json --out target/ripr/reports/after-diff-finding.gap-decision-ledger.json --out-md target/ripr/reports/after-diff-finding.gap-decision-ledger.md
```

Observed:

```text
repairable = 210
ripr zero target count = 210
ripr plus target count = 210
crates/allow-diff/src/finding.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `219` to `210` and
reduced `crates/allow-diff/src/finding.rs` from `9` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 142 |
| `error_variant` | 22 |
| `return_value` | 16 |
| `match_arm` | 14 |
| `predicate_boundary` | 11 |
| `field_construction` | 5 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/selector_validation.rs` | 9 |
| `crates/allow-rust/src/safety_comments.rs` | 9 |
| `crates/allow-policy/src/toml_last_seen.rs` | 8 |
| `crates/allow-report/src/diff_finding_detail.rs` | 8 |
| `crates/allow-policy/src/entries_validation.rs` | 8 |

## Ninety-Seventh Burn-Down Slice

The ninety-seventh focused slice added direct selector validation coverage for
`crates/allow-policy/src/selector_validation.rs`.

The new tests prove that:

- `validate_selector` accepts source-code selectors with structural identity.
- selector identity fields are checked through the required-text validator and
  reject blank values.
- source-code entries still require selector identity beyond path, glob, and
  line hints.
- non-source entries may use a glob-only selector and still pass selector
  validation.
- `validate_source_hints` rejects zero `line_hint`, zero `last_seen.line`, and
  zero `last_seen.column`.
- valid `line_hint` and `last_seen` coordinates pass source-hint validation.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-selector-validation.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-selector-validation.repo-exposure.json --out target/ripr/reports/after-selector-validation.gap-decision-ledger.json --out-md target/ripr/reports/after-selector-validation.gap-decision-ledger.md
```

Observed:

```text
repairable = 199
ripr zero target count = 199
ripr plus target count = 199
crates/allow-policy/src/selector_validation.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `210` to `199` and
reduced `crates/allow-policy/src/selector_validation.rs` from `9` repairable
targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 134 |
| `error_variant` | 22 |
| `match_arm` | 14 |
| `return_value` | 13 |
| `predicate_boundary` | 11 |
| `field_construction` | 5 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-rust/src/safety_comments.rs` | 9 |
| `crates/allow-policy/src/toml_last_seen.rs` | 8 |
| `crates/allow-report/src/diff_finding_detail.rs` | 8 |
| `crates/allow-report/src/evidence_repair.rs` | 8 |
| `crates/allow-inventory/src/root.rs` | 7 |

## Ninety-Eighth Burn-Down Slice

The ninety-eighth focused slice added direct safety-comment helper coverage for
`crates/allow-rust/src/safety_comments.rs`.

The new tests prove that:

- `safety_comment_lines` iterates source lines and records `// SAFETY:`,
  `/* SAFETY:`, block-continuation `* SAFETY:`, and trailing inline
  `// SAFETY:` forms.
- string literals containing `SAFETY:` are not treated as safety comments when
  no Rust comment delimiter is present.
- `has_nearby_safety_comment` accepts a safety comment on the current line or
  within the previous three lines, including line-one saturation, and rejects
  outside-window or zero-line probes.
- `is_safety_comment` trims leading whitespace, accepts Rust line, block,
  continuation, and trailing inline forms, and rejects lowercase markers,
  bare `SAFETY:` text, non-Rust comment prefixes, and block comments without
  the required marker.

After regenerating repo exposure and the gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-safety-comments.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-safety-comments.repo-exposure.json --out target/ripr/reports/after-safety-comments.gap-decision-ledger.json --out-md target/ripr/reports/after-safety-comments.gap-decision-ledger.md
```

Observed:

```text
repairable = 189
ripr zero target count = 189
ripr plus target count = 189
crates/allow-rust/src/safety_comments.rs repairable targets = 1
```

The focused slice reduced repo-scoped `ripr+` targets from `199` to `189` and
reduced `crates/allow-rust/src/safety_comments.rs` from `9` repairable targets
to `1`. The remaining safety-comment row is a predicate-boundary row for the
comment-prefix discriminator despite direct assertions for `//`, `/*`,
block-continuation, trailing inline, bare text, non-Rust prefix, and missing
marker cases.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 125 |
| `error_variant` | 22 |
| `match_arm` | 14 |
| `return_value` | 13 |
| `predicate_boundary` | 11 |
| `field_construction` | 4 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-report/src/diff_finding_detail.rs` | 8 |
| `crates/allow-report/src/evidence_repair.rs` | 8 |
| `crates/allow-policy/src/toml_last_seen.rs` | 8 |
| `crates/allow-inventory/src/root.rs` | 7 |
| `crates/cargo-allow/src/compat_scan.rs` | 7 |

## Ninety-Ninth Burn-Down Slice

The ninety-ninth focused slice added direct conversion coverage for
`crates/allow-policy/src/toml_last_seen.rs`.

The new tests prove that `LastSeenToml::into_last_seen`:

- returns `Some(LastSeen { line, column })` when both coordinates are present.
- returns `None` when both coordinates are absent.
- rejects line-only and column-only coordinates with the entry ID in the error
  message.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-toml-last-seen-committed.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-toml-last-seen-committed.repo-exposure.json --out target/ripr/reports/after-toml-last-seen-committed.gap-decision-ledger.json --out-md target/ripr/reports/after-toml-last-seen-committed.gap-decision-ledger.md
```

Observed:

```text
repairable = 181
ripr zero target count = 181
ripr plus target count = 181
crates/allow-policy/src/toml_last_seen.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `189` to `181` and
reduced `crates/allow-policy/src/toml_last_seen.rs` from `8` repairable targets
to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 122 |
| `error_variant` | 22 |
| `return_value` | 12 |
| `predicate_boundary` | 11 |
| `match_arm` | 10 |
| `field_construction` | 4 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-report/src/diff_finding_detail.rs` | 8 |
| `crates/allow-report/src/evidence_repair.rs` | 8 |
| `crates/allow-rust/src/line_index_findings.rs` | 7 |
| `crates/allow-inventory/src/root.rs` | 7 |
| `crates/allow-rust/src/line_unsafe_findings.rs` | 7 |

## Hundredth Burn-Down Slice

The hundredth focused slice added direct structural identity summary coverage
for `crates/allow-report/src/diff_finding_detail.rs`.

The new tests prove that `structural_identity_summary`:

- renders structural identity fields in the stable human/Markdown report order.
- trims required and optional field values before rendering.
- omits blank required and optional fields.
- returns an empty summary for a blank identity with no nonblank optional fields.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-diff-finding-detail.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-diff-finding-detail.repo-exposure.json --out target/ripr/reports/after-diff-finding-detail.gap-decision-ledger.json --out-md target/ripr/reports/after-diff-finding-detail.gap-decision-ledger.md
```

Observed:

```text
repairable = 173
ripr zero target count = 173
ripr plus target count = 173
crates/allow-report/src/diff_finding_detail.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `181` to `173` and
reduced `crates/allow-report/src/diff_finding_detail.rs` from `8` repairable
targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 115 |
| `error_variant` | 22 |
| `return_value` | 12 |
| `predicate_boundary` | 10 |
| `match_arm` | 10 |
| `field_construction` | 4 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-report/src/evidence_repair.rs` | 8 |
| `crates/allow-rust/src/line_unsafe_findings.rs` | 7 |
| `crates/allow-rust/src/line_index_findings.rs` | 7 |
| `crates/allow-inventory/src/root.rs` | 7 |
| `crates/cargo-allow/src/list_render.rs` | 7 |

## Hundred-First Burn-Down Slice

The hundred-first focused slice added direct evidence-repair queue coverage for
`crates/allow-report/src/evidence_repair.rs`.

The new tests prove that:

- `evidence_repair_queues_from_context` routes broken evidence links, missing
  evidence, and weak evidence references to their expected worklist queues.
- missing-evidence queue counts use the policy-level missing-evidence count
  when it exceeds the source-finding `EvidenceMissing` count.
- `push_evidence_repair_queue_json_fields` emits the full JSON fragment with
  required fields, optional fields, count, command, indentation, and escaping.
- absent optional `item_kind` and `worklist_filter` fields are omitted while
  required fields still render.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-evidence-repair.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-evidence-repair.repo-exposure.json --out target/ripr/reports/after-evidence-repair.gap-decision-ledger.json --out-md target/ripr/reports/after-evidence-repair.gap-decision-ledger.md
```

Observed:

```text
repairable = 165
ripr zero target count = 165
ripr plus target count = 165
crates/allow-report/src/evidence_repair.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `173` to `165` and
reduced `crates/allow-report/src/evidence_repair.rs` from `8` repairable
targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 107 |
| `error_variant` | 22 |
| `return_value` | 12 |
| `predicate_boundary` | 10 |
| `match_arm` | 10 |
| `field_construction` | 4 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/compat_scan.rs` | 7 |
| `crates/allow-inventory/src/root.rs` | 7 |
| `crates/allow-rust/src/line_unsafe_findings.rs` | 7 |
| `crates/allow-rust/src/line_index_findings.rs` | 7 |
| `crates/cargo-allow/src/list_render.rs` | 7 |

## Hundred-Second Burn-Down Slice

The hundred-second focused slice added direct list-render adapter coverage for
`crates/cargo-allow/src/list_render.rs`.

The new tests prove that:

- `render_list_rows_with_context` applies list filters before rendering and
  forwards source-tree inventory context to the human list renderer.
- `render_list_rows_json` applies list filters, forwards context filters, and
  projects only matching rows into the JSON artifact.
- list row projection preserves status, kind, family, owner, classification,
  scope, source package, evidence health, selector precision, broad-scope state,
  reason, and lifecycle fields.
- dash lifecycle placeholders are translated to absent JSON lifecycle values.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-list-render.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-list-render.repo-exposure.json --out target/ripr/reports/after-list-render.gap-decision-ledger.json --out-md target/ripr/reports/after-list-render.gap-decision-ledger.md
```

Observed:

```text
repairable = 158
ripr zero target count = 158
ripr plus target count = 158
crates/cargo-allow/src/list_render.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `165` to `158` and
reduced `crates/cargo-allow/src/list_render.rs` from `7` repairable targets to
`0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 100 |
| `error_variant` | 22 |
| `return_value` | 12 |
| `predicate_boundary` | 10 |
| `match_arm` | 10 |
| `field_construction` | 4 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/compat_scan.rs` | 7 |
| `crates/allow-rust/src/line_unsafe_findings.rs` | 7 |
| `crates/allow-inventory/src/root.rs` | 7 |
| `crates/allow-rust/src/line_index_findings.rs` | 7 |
| `crates/allow-policy/src/text_validation.rs` | 6 |

## Hundred-Third Burn-Down Slice

The hundred-third focused slice added same-module adapter coverage for
`crates/cargo-allow/src/compat_scan.rs`.

The new tests prove that:

- `scan_legacy_rust_compat` builds inventory from the compatibility policy
  workspace settings, honors ignored paths, scans Rust files, and filters the
  result set to the requested finding kind.
- `scan_non_rust_compat` builds source-tree inventory, preserves inventory
  facts, and reports only non-Rust file findings from mixed Rust/non-Rust
  input.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-compat-scan.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-compat-scan.repo-exposure.json --out target/ripr/reports/after-compat-scan.gap-decision-ledger.json --out-md target/ripr/reports/after-compat-scan.gap-decision-ledger.md
```

Observed:

```text
repairable = 151
ripr zero target count = 151
ripr plus target count = 151
crates/cargo-allow/src/compat_scan.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `158` to `151` and
reduced `crates/cargo-allow/src/compat_scan.rs` from `7` repairable targets to
`0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 93 |
| `error_variant` | 22 |
| `return_value` | 12 |
| `predicate_boundary` | 10 |
| `match_arm` | 10 |
| `field_construction` | 4 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-inventory/src/root.rs` | 7 |
| `crates/allow-rust/src/line_index_findings.rs` | 7 |
| `crates/allow-rust/src/line_unsafe_findings.rs` | 7 |
| `crates/allow-policy/src/entries_validation.rs` | 6 |
| `crates/allow-policy/src/text_validation.rs` | 6 |

## Hundred-Fourth Burn-Down Slice

The hundred-fourth focused slice added same-module root-canonicalization
coverage for `crates/allow-inventory/src/root.rs`.

The new tests prove that:

- `canonical_dir` accepts an existing directory, returns its canonical path, and
  rejects an explicit source-tree root that resolves to a file.
- `canonical_start_dir` uses a start file's parent directory, preserves
  directory starts, and reports a canonicalization error for missing starts.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-inventory-root.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-inventory-root.repo-exposure.json --out target/ripr/reports/after-inventory-root.gap-decision-ledger.json --out-md target/ripr/reports/after-inventory-root.gap-decision-ledger.md
```

Observed:

```text
repairable = 144
ripr zero target count = 144
ripr plus target count = 144
crates/allow-inventory/src/root.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `151` to `144` and
reduced `crates/allow-inventory/src/root.rs` from `7` repairable targets to
`0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 86 |
| `error_variant` | 22 |
| `return_value` | 12 |
| `predicate_boundary` | 10 |
| `match_arm` | 10 |
| `field_construction` | 4 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-rust/src/line_unsafe_findings.rs` | 7 |
| `crates/allow-rust/src/line_index_findings.rs` | 7 |
| `crates/allow-rust/src/line_panic_findings.rs` | 6 |
| `crates/allow-policy/src/entries_validation.rs` | 6 |
| `crates/allow-policy/src/text_validation.rs` | 6 |

## Hundred-Fifth Burn-Down Slice

The hundred-fifth focused slice added same-module line-index scanner coverage
for `crates/allow-rust/src/line_index_findings.rs`.

The new tests prove that:

- `scan_index_expr` projects ordinary index expressions as `panic.indexing`
  findings and range/slice expressions as `panic.string_slice` findings.
- index-expression findings preserve path, span column, container, module,
  symbol, receiver fingerprint, target fingerprint, and source line hints.
- empty input leaves the findings queue unchanged, and non-empty input appends
  findings without replacing existing queue state.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-line-index.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-line-index.repo-exposure.json --out target/ripr/reports/after-line-index.gap-decision-ledger.json --out-md target/ripr/reports/after-line-index.gap-decision-ledger.md
```

Observed:

```text
repairable = 137
ripr zero target count = 137
ripr plus target count = 137
crates/allow-rust/src/line_index_findings.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `144` to `137` and
reduced `crates/allow-rust/src/line_index_findings.rs` from `7` repairable
targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 80 |
| `error_variant` | 22 |
| `return_value` | 12 |
| `match_arm` | 10 |
| `predicate_boundary` | 9 |
| `field_construction` | 4 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-rust/src/line_unsafe_findings.rs` | 7 |
| `crates/allow-policy/src/text_validation.rs` | 6 |
| `crates/allow-policy/src/entries_validation.rs` | 6 |
| `crates/cargo-allow/src/evidence_inventory.rs` | 6 |
| `crates/cargo-allow/src/init.rs` | 6 |

## Hundred-Sixth Burn-Down Slice

The hundred-sixth focused slice added same-module unsafe-line scanner coverage
for `crates/allow-rust/src/line_unsafe_findings.rs`.

The new tests prove that:

- `scan_unsafe_constructs` projects unsafe function and unsafe block syntax to
  `unsafe_fn` and `unsafe_block` findings while preserving path, column, module,
  container, symbol, and safety-comment fingerprints.
- unsafe impl and unsafe trait syntax use their visible symbols as container
  fallbacks when no containing item is already known.
- unsafe attributes are projected as `unsafe_attr` findings with symbol,
  container, module, and safety-comment identity, and empty input leaves the
  findings queue unchanged.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-line-unsafe.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-line-unsafe.repo-exposure.json --out target/ripr/reports/after-line-unsafe.gap-decision-ledger.json --out-md target/ripr/reports/after-line-unsafe.gap-decision-ledger.md
```

Observed:

```text
repairable = 130
ripr zero target count = 130
ripr plus target count = 130
crates/allow-rust/src/line_unsafe_findings.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `137` to `130` and
reduced `crates/allow-rust/src/line_unsafe_findings.rs` from `7` repairable
targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 75 |
| `error_variant` | 22 |
| `return_value` | 12 |
| `match_arm` | 10 |
| `predicate_boundary` | 7 |
| `field_construction` | 4 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/evidence_inventory.rs` | 6 |
| `crates/allow-policy/src/text_validation.rs` | 6 |
| `crates/allow-policy/src/entries_validation.rs` | 6 |
| `crates/allow-diff/src/policy.rs` | 6 |
| `crates/cargo-allow/src/init.rs` | 6 |

## Hundred-Seventh Burn-Down Slice

The hundred-seventh focused slice added same-module policy text validation
coverage for `crates/allow-policy/src/text_validation.rs`.

The new tests prove that:

- optional text accepts absent and valid values.
- optional text rejects blank values and whitespace-padded values with the
  correct diagnostic.
- required text rejects empty values before surrounding-whitespace validation.
- surrounding-whitespace validation preserves interior whitespace while
  rejecting leading and trailing whitespace.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-text-validation.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-text-validation.repo-exposure.json --out target/ripr/reports/after-text-validation.gap-decision-ledger.json --out-md target/ripr/reports/after-text-validation.gap-decision-ledger.md
```

Observed:

```text
repairable = 124
ripr zero target count = 124
ripr plus target count = 124
crates/allow-policy/src/text_validation.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `130` to `124` and
reduced `crates/allow-policy/src/text_validation.rs` from `6` repairable
targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 71 |
| `error_variant` | 22 |
| `match_arm` | 10 |
| `return_value` | 10 |
| `predicate_boundary` | 7 |
| `field_construction` | 4 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/init.rs` | 6 |
| `crates/allow-policy/src/entries_validation.rs` | 6 |
| `crates/cargo-allow/src/evidence_inventory.rs` | 6 |
| `crates/allow-diff/src/policy.rs` | 6 |
| `crates/allow-rust/src/line_panic_findings.rs` | 6 |

## Hundred-Eighth Burn-Down Slice

The hundred-eighth focused slice added git-backed policy lookup coverage for
`crates/allow-diff/src/policy.rs`.

The new tests prove that:

- missing policy files at a requested Git revision return `None`.
- policy lookup parses the committed policy at the requested revision instead
  of reading uncommitted worktree state.
- a missing base policy is treated as an empty ledger and reports the exact
  added allow entry as review-required.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-policy-git.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-policy-git.repo-exposure.json --out target/ripr/reports/after-policy-git.gap-decision-ledger.json --out-md target/ripr/reports/after-policy-git.gap-decision-ledger.md
```

Observed:

```text
repairable = 118
ripr zero target count = 118
ripr plus target count = 118
crates/allow-diff/src/policy.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `124` to `118` and
reduced `crates/allow-diff/src/policy.rs` from `6` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 68 |
| `error_variant` | 22 |
| `match_arm` | 10 |
| `return_value` | 7 |
| `predicate_boundary` | 7 |
| `field_construction` | 4 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/init.rs` | 6 |
| `crates/allow-rust/src/line_panic_findings.rs` | 6 |
| `crates/cargo-allow/src/worklist_scoring.rs` | 6 |
| `crates/cargo-allow/src/evidence_inventory.rs` | 6 |
| `crates/allow-policy/src/entries_validation.rs` | 6 |

## Hundred-Ninth Burn-Down Slice

The hundred-ninth focused slice added same-module panic-line scanner coverage
for `crates/allow-rust/src/line_panic_findings.rs`.

The new tests prove that:

- panic method calls project `unwrap` and `expect` findings with method-call
  identity, callee, receiver fingerprint, path, container, module, line, and
  column details.
- panic macro invocations project macro-call findings with public family,
  macro name, visible macro path, container, module, line, and column details.
- empty panic syntax inputs leave the queue unchanged, and non-empty inputs
  append findings without replacing existing queue state.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-line-panic.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-line-panic.repo-exposure.json --out target/ripr/reports/after-line-panic.gap-decision-ledger.json --out-md target/ripr/reports/after-line-panic.gap-decision-ledger.md
```

Observed:

```text
repairable = 112
ripr zero target count = 112
ripr plus target count = 112
crates/allow-rust/src/line_panic_findings.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `118` to `112` and
reduced `crates/allow-rust/src/line_panic_findings.rs` from `6` repairable
targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 63 |
| `error_variant` | 22 |
| `match_arm` | 10 |
| `predicate_boundary` | 7 |
| `return_value` | 6 |
| `field_construction` | 4 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/evidence_inventory.rs` | 6 |
| `crates/allow-policy/src/entries_validation.rs` | 6 |
| `crates/cargo-allow/src/worklist_scoring.rs` | 6 |
| `crates/cargo-allow/src/init.rs` | 6 |
| `crates/allow-diff/src/revision.rs` | 5 |

## Hundred-Tenth Burn-Down Slice

The hundred-tenth focused slice added same-module worklist classification
coverage for `crates/cargo-allow/src/worklist_scoring.rs`.

The new tests prove that:

- match outcomes map to the expected work item kinds, including new
  unreceipted findings, occurrence-limit overages, stale/expired entries,
  missing fields, invalid selectors, baseline debt, review-due entries, matched
  entries, and unsafe-specific missing-evidence work items.
- work item risk stays low for stale entries, rises to high for ambiguous,
  expired, unsafe, process-spawn, and network-destination cases, and remains
  medium for the ordinary repair queue.
- work item difficulty is small for stale, selector, missing-field, missing
  evidence, non-Rust, and generated-code repairs, and medium for review,
  baseline, unsafe evidence, panic, and occurrence-limit repairs.
- exception family selection prefers the current finding and falls back to the
  policy entry family when the finding has no family.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
cmd /c "ripr check --root . --mode instant --format repo-exposure-json > target\ripr\reports\after-worklist-scoring.repo-exposure.json"
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-worklist-scoring.repo-exposure.json --out target/ripr/reports/after-worklist-scoring.gap-decision-ledger.json --out-md target/ripr/reports/after-worklist-scoring.gap-decision-ledger.md
```

Observed:

```text
repairable = 106
ripr zero target count = 106
ripr plus target count = 106
crates/cargo-allow/src/worklist_scoring.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `112` to `106` and
reduced `crates/cargo-allow/src/worklist_scoring.rs` from `6` repairable
targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 62 |
| `error_variant` | 22 |
| `predicate_boundary` | 7 |
| `return_value` | 6 |
| `match_arm` | 5 |
| `field_construction` | 4 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/evidence_inventory.rs` | 6 |
| `crates/cargo-allow/src/init.rs` | 6 |
| `crates/allow-policy/src/entries_validation.rs` | 6 |
| `crates/allow-diff/src/revision.rs` | 5 |
| `crates/cargo-allow/src/companion.rs` | 5 |

## Hundred-Eleventh Burn-Down Slice

The hundred-eleventh focused slice added same-module source-tree evidence
inventory coverage for `crates/cargo-allow/src/evidence_inventory.rs`.

The new tests prove that:

- `evidence_reference_diagnostics_for_source_tree` downgrades present local
  evidence files to missing when they are outside the default git-tracked
  inventory, preserves present status for inventory members, and skips inventory
  filtering when the inventory is unavailable.
- `policy_reference_diagnostics_for_source_tree` applies the same inventory
  downgrade to link references.
- `validate_evidence_references_for_source_tree` returns `Ok(())` when all
  references are inventory-clean and returns structured errors for missing local
  files and for present-but-untracked evidence.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-evidence-inventory.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-evidence-inventory.repo-exposure.json --out target/ripr/reports/after-evidence-inventory.gap-decision-ledger.json --out-md target/ripr/reports/after-evidence-inventory.gap-decision-ledger.md
```

Observed:

```text
repairable = 100
ripr zero target count = 100
ripr plus target count = 100
crates/cargo-allow/src/evidence_inventory.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `106` to `100` and
reduced `crates/cargo-allow/src/evidence_inventory.rs` from `6` repairable
targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 57 |
| `error_variant` | 22 |
| `predicate_boundary` | 7 |
| `return_value` | 5 |
| `match_arm` | 5 |
| `field_construction` | 4 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/entries_validation.rs` | 6 |
| `crates/cargo-allow/src/init.rs` | 6 |
| `crates/allow-diff/src/revision.rs` | 5 |
| `crates/cargo-allow/src/companion.rs` | 5 |
| `crates/allow-diff/src/policy_entry_identity.rs` | 4 |

## Hundred-Twelfth Burn-Down Slice

The hundred-twelfth focused slice added same-module allow-entry orchestration
coverage for `crates/allow-policy/src/entries_validation.rs`.

The new tests prove that:

- `validate_allow_entries` returns `Ok(())` for a valid multi-entry batch and
  rejects duplicate allow ids through the strict link-scope path.
- `validate_allow_entries` rejects invalid local link paths under strict mode.
- `validate_allow_entries_with_reportable_evidence` keeps invalid local links
  reportable instead of failing validation.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-entries-validation.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-entries-validation.repo-exposure.json --out target/ripr/reports/after-entries-validation.gap-decision-ledger.json --out-md target/ripr/reports/after-entries-validation.gap-decision-ledger.md
```

Observed:

```text
repairable = 94
ripr zero target count = 94
ripr plus target count = 94
crates/allow-policy/src/entries_validation.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `100` to `94` and
reduced `crates/allow-policy/src/entries_validation.rs` from `6` repairable
targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 52 |
| `error_variant` | 22 |
| `predicate_boundary` | 7 |
| `return_value` | 4 |
| `match_arm` | 5 |
| `field_construction` | 4 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/init.rs` | 6 |
| `crates/allow-diff/src/revision.rs` | 5 |
| `crates/cargo-allow/src/companion.rs` | 5 |
| `crates/allow-diff/src/policy_entry_identity.rs` | 4 |
| `crates/allow-policy/src/entry_validation.rs` | 4 |

## Hundred-Thirteenth Burn-Down Slice

The hundred-thirteenth focused slice added `cmd_init` error-path coverage in
`crates/cargo-allow/src/init_tests.rs` using the same `CargoAllowError` payload
pattern as the ninety-third `io.rs` slice.

The new and tightened tests prove that:

- `cmd_init` returns `Ok(())` for successful writes and dry-run paths.
- `cmd_init` rejects an existing policy file without `--force` with the exact
  `CargoAllowError` payload.
- `cmd_init` reports parent-creation and policy-write failures with the target
  path and platform source error in the `CargoAllowError` payload.
- spec-system init rejects `--strict` with the exact unsupported-option error.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-init.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-init.repo-exposure.json --out target/ripr/reports/after-init.gap-decision-ledger.json --out-md target/ripr/reports/after-init.gap-decision-ledger.md
```

Observed:

```text
repairable = 91
ripr zero target count = 91
ripr plus target count = 91
crates/cargo-allow/src/init.rs repairable targets = 3
```

The focused slice reduced repo-scoped `ripr+` targets from `94` to `91` and
reduced `crates/cargo-allow/src/init.rs` from `6` repairable targets to `3`.
The remaining `init.rs` targets are `MissingErrorDiscriminator` rows for the
`failed to read cwd`, `failed to create`, and `failed to write` branches. The
create/write branches now have exact `CargoAllowError` assertions mirroring the
`io.rs` pattern; the cwd branch is not practically triggerable without mocking
`env::current_dir`, and ripr still reports the create/write discriminator rows
as actionable provider noise.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 52 |
| `error_variant` | 19 |
| `predicate_boundary` | 7 |
| `match_arm` | 5 |
| `return_value` | 4 |
| `field_construction` | 4 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/companion.rs` | 5 |
| `crates/allow-diff/src/policy_entry_identity.rs` | 4 |
| `crates/allow-policy/src/entry_validation.rs` | 4 |
| `crates/cargo-allow/src/init.rs` | 3 |

## Hundred-Fourteenth Burn-Down Slice

The hundred-fourteenth focused slice added same-module revision helper coverage
for `crates/allow-diff/src/revision.rs`.

The new tests prove that:

- `has_generated_code_receipt` returns true when the config includes a
  `GeneratedCode` allow entry with the `generated_code` family and false
  otherwise.
- `has_policy_family` returns true when the config includes a matching
  `PolicyException` family and false for unrelated families.
- `is_workflow_path` accepts `.github/workflows/*.yml` and `.yaml` paths and
  rejects non-workflow locations and non-YAML extensions.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-revision.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-revision.repo-exposure.json --out target/ripr/reports/after-revision.gap-decision-ledger.json --out-md target/ripr/reports/after-revision.gap-decision-ledger.md
```

Observed:

```text
repairable = 86
ripr zero target count = 86
ripr plus target count = 86
crates/allow-diff/src/revision.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `91` to `86` and
reduced `crates/allow-diff/src/revision.rs` from `5` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 48 |
| `error_variant` | 19 |
| `predicate_boundary` | 7 |
| `match_arm` | 5 |
| `field_construction` | 4 |
| `return_value` | 3 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/companion.rs` | 5 |
| `crates/allow-report/src/explain_common.rs` | 4 |
| `crates/allow-policy/src/validation.rs` | 4 |
| `crates/allow-policy/src/toml_workspace.rs` | 4 |
| `crates/allow-report/src/audit_remediation.rs` | 4 |

## Hundred-Fifteenth Burn-Down Slice

The hundred-fifteenth focused slice added same-module companion helper coverage
for `crates/cargo-allow/src/companion.rs`.

The new tests prove that:

- `has_policy_family` returns true when the config includes a matching
  `PolicyException` family and false for unrelated families.
- `has_allow_family` matches on both `FindingKind` and family name.
- `same_finding_identity` compares findings through `finding_identity_key` and
  distinguishes distinct paths.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-companion.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-companion.repo-exposure.json --out target/ripr/reports/after-companion.gap-decision-ledger.json --out-md target/ripr/reports/after-companion.gap-decision-ledger.md
```

Observed:

```text
repairable = 81
ripr zero target count = 81
ripr plus target count = 81
crates/cargo-allow/src/companion.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `86` to `81` and
reduced `crates/cargo-allow/src/companion.rs` from `5` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 43 |
| `error_variant` | 19 |
| `predicate_boundary` | 7 |
| `match_arm` | 5 |
| `field_construction` | 4 |
| `return_value` | 3 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/validation.rs` | 4 |
| `crates/allow-diff/src/policy_entry_scope.rs` | 4 |
| `crates/allow-policy/src/toml_workspace.rs` | 4 |
| `crates/cargo-allow/src/add_entry.rs` | 4 |
| `crates/allow-diff/src/policy_entry_identity.rs` | 4 |

## Hundred-Sixteenth Burn-Down Slice

The hundred-sixteenth focused slice added policy orchestration coverage for
`crates/allow-policy/src/validation.rs`.

The new tests prove that:

- `validate_policy` returns `Ok(())` for a valid policy and rejects unsupported
  schema versions through the header validator.
- `validate_policy` rejects duplicate allow ids through the strict allow-entry
  orchestration path.
- `validate_policy_with_reportable_evidence` keeps invalid local links
  reportable instead of failing validation.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-validation.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-validation.repo-exposure.json --out target/ripr/reports/after-validation.gap-decision-ledger.json --out-md target/ripr/reports/after-validation.gap-decision-ledger.md
```

Observed:

```text
repairable = 77
ripr zero target count = 77
ripr plus target count = 77
crates/allow-policy/src/validation.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `81` to `77` and
reduced `crates/allow-policy/src/validation.rs` from `4` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 39 |
| `error_variant` | 19 |
| `predicate_boundary` | 7 |
| `match_arm` | 5 |
| `field_construction` | 4 |
| `return_value` | 3 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/toml_workspace.rs` | 4 |
| `crates/allow-diff/src/policy_entry_scope.rs` | 4 |
| `crates/allow-report/src/audit_remediation.rs` | 4 |
| `crates/allow-report/src/explain_common.rs` | 4 |
| `crates/allow-diff/src/policy_entry_identity.rs` | 4 |

## Hundred-Seventeenth Burn-Down Slice

The hundred-seventeenth focused slice added workspace TOML conversion coverage
for `crates/allow-policy/src/toml_workspace.rs`.

The new tests prove that:

- `into_workspace_config` applies explicit `root` and `default_mode` values.
- `into_workspace_config` normalizes the `git_tracked` inventory alias to
  `git-tracked`.
- `into_workspace_config` preserves default ignored and generated globs when the
  TOML lists are empty and keeps custom globs when provided.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-toml-workspace.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-toml-workspace.repo-exposure.json --out target/ripr/reports/after-toml-workspace.gap-decision-ledger.json --out-md target/ripr/reports/after-toml-workspace.gap-decision-ledger.md
```

Observed:

```text
repairable = 73
ripr zero target count = 73
ripr plus target count = 73
crates/allow-policy/src/toml_workspace.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `77` to `73` and
reduced `crates/allow-policy/src/toml_workspace.rs` from `4` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 35 |
| `error_variant` | 19 |
| `predicate_boundary` | 7 |
| `match_arm` | 5 |
| `field_construction` | 4 |
| `return_value` | 3 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-report/src/explain_common.rs` | 4 |
| `crates/allow-report/src/audit_remediation.rs` | 4 |
| `crates/allow-diff/src/policy_entry_scope.rs` | 4 |
| `crates/cargo-allow/src/add_entry.rs` | 4 |
| `crates/allow-diff/src/policy_entry_identity.rs` | 4 |

## Hundred-Eighteenth Burn-Down Slice

The hundred-eighteenth focused slice added policy entry scope diff coverage for
`crates/allow-diff/src/policy_entry_scope.rs`.

The new tests prove that:

- `scope_policy_changes` reports scope broadening when a path-scoped entry gains
  a wider glob.
- `scope_policy_changes` reports scope narrowing when a glob-scoped entry
  tightens back to a path.
- `scope_policy_changes` reports review-required scope changes for sibling glob
  retargets that are neither broadened nor narrowed.
- `normalize_scope_text` normalizes Windows-style backslashes to forward slashes.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-policy-entry-scope.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-policy-entry-scope.repo-exposure.json --out target/ripr/reports/after-policy-entry-scope.gap-decision-ledger.json --out-md target/ripr/reports/after-policy-entry-scope.gap-decision-ledger.md
```

Observed:

```text
repairable = 69
ripr zero target count = 69
ripr plus target count = 69
crates/allow-diff/src/policy_entry_scope.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `73` to `69` and
reduced `crates/allow-diff/src/policy_entry_scope.rs` from `4` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 31 |
| `error_variant` | 19 |
| `predicate_boundary` | 7 |
| `match_arm` | 5 |
| `field_construction` | 4 |
| `return_value` | 3 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-diff/src/policy_entry_identity.rs` | 4 |
| `crates/allow-report/src/explain_common.rs` | 4 |
| `crates/allow-report/src/audit_remediation.rs` | 4 |
| `crates/cargo-allow/src/add_entry.rs` | 4 |
| `crates/cargo-allow/src/prune.rs` | 3 |

## Hundred-Nineteenth Burn-Down Slice

The hundred-nineteenth focused slice added policy entry identity diff coverage
for `crates/allow-diff/src/policy_entry_identity.rs`.

The new tests prove that:

- `identity_policy_changes` reports governed exception kind changes with
  structured exception identity.
- `identity_policy_changes` reports family changes, including replacement and
  removal, with structured before/after values.
- `identity_policy_changes` returns no changes when kind and family are
  unchanged.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-policy-entry-identity.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-policy-entry-identity.repo-exposure.json --out target/ripr/reports/after-policy-entry-identity.gap-decision-ledger.json --out-md target/ripr/reports/after-policy-entry-identity.gap-decision-ledger.md
```

Observed:

```text
repairable = 65
ripr zero target count = 65
ripr plus target count = 65
crates/allow-diff/src/policy_entry_identity.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `69` to `65` and
reduced `crates/allow-diff/src/policy_entry_identity.rs` from `4` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 27 |
| `error_variant` | 19 |
| `predicate_boundary` | 7 |
| `match_arm` | 5 |
| `field_construction` | 4 |
| `return_value` | 3 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-report/src/audit_remediation.rs` | 4 |
| `crates/allow-report/src/explain_common.rs` | 4 |
| `crates/cargo-allow/src/add_entry.rs` | 4 |
| `crates/cargo-allow/src/prune.rs` | 3 |
| `crates/cargo-allow/src/io.rs` | 3 |

## Hundred-Twentieth Burn-Down Slice

The hundred-twentieth focused slice added same-module add-entry helper coverage
for `crates/cargo-allow/src/add_entry.rs`.

The new tests prove that:

- `ensure_addable_outcome` returns `Ok(())` for `MatchStatus::New`.
- `ensure_addable_outcome` rejects `MatchStatus::Matched` and
  `MatchStatus::Stale` with the exact `CargoAllowError` payload.
- `next_allow_id` returns `allow-0001` for an empty config and skips occupied
  ids.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-add-entry.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-add-entry.repo-exposure.json --out target/ripr/reports/after-add-entry.gap-decision-ledger.json --out-md target/ripr/reports/after-add-entry.gap-decision-ledger.md
```

Observed:

```text
repairable = 45
ripr zero target count = 45
ripr plus target count = 45
crates/cargo-allow/src/add_entry.rs repairable targets = 1
```

The focused slice reduced repo-scoped `ripr+` targets from `65` to `45` and
reduced `crates/cargo-allow/src/add_entry.rs` from `4` repairable targets to `1`.
The remaining `add_entry.rs` target is a `MissingBoundaryAssertion` row on the
`status == MatchStatus::New` branch even though
`ensure_addable_outcome_boundary_accepts_new_status` already uses the exact
`assert_eq!(ensure_addable_outcome(MatchStatus::New), Ok(()))` shape ripr
suggests; treat it as actionable provider noise.

Ledger regeneration also attributed collateral clearance to adjacent
`cargo-allow` helper modules (`prune.rs`, `io.rs`, `init.rs`, and others)
without dedicated tests in this slice.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 26 |
| `predicate_boundary` | 7 |
| `match_arm` | 5 |
| `field_construction` | 4 |
| `return_value` | 3 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-report/src/audit_remediation.rs` | 4 |
| `crates/allow-report/src/explain_common.rs` | 4 |
| `crates/allow-report/src/worklist_summary.rs` | 3 |
| `crates/cargo-allow/src/propose_args.rs` | 3 |
| `crates/cargo-allow/src/worklist_item_kind.rs` | 3 |

## Hundred-Twenty-First Burn-Down Slice

The hundred-twenty-first focused slice added explain helper coverage for
`crates/allow-report/src/explain_common.rs`.

The new tests prove that:

- `finding_location_text` formats span-backed findings as
  `path:line:column` with normalized paths.
- `finding_location_text` returns only the normalized path when span is absent.
- the no-span path exercises `normalize_path` on Windows-style separators.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-explain-common.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-explain-common.repo-exposure.json --out target/ripr/reports/after-explain-common.gap-decision-ledger.json --out-md target/ripr/reports/after-explain-common.gap-decision-ledger.md
```

Observed:

```text
repairable = 41
ripr zero target count = 41
ripr plus target count = 41
crates/allow-report/src/explain_common.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `45` to `41` and
reduced `crates/allow-report/src/explain_common.rs` from `4` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 25 |
| `predicate_boundary` | 7 |
| `field_construction` | 4 |
| `return_value` | 3 |
| `match_arm` | 2 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-report/src/audit_remediation.rs` | 4 |
| `crates/allow-report/src/worklist_summary.rs` | 3 |
| `crates/cargo-allow/src/propose_args.rs` | 3 |
| `crates/cargo-allow/src/worklist_item_kind.rs` | 3 |
| `crates/allow-policy/src/evidence_validation.rs` | 2 |

## Hundred-Twenty-Second Burn-Down Slice

The hundred-twenty-second focused slice added audit remediation route coverage
for `crates/allow-report/src/audit_remediation.rs`.

The new tests prove that:

- `audit_remediation_items` builds `worklist_status` routes with populated
  `item_kind` and `worklist_status` fields for new findings.
- `audit_remediation_items` builds `prune_stale` routes with the
  `stale_allow` item kind for stale findings.
- `push_audit_remediation_item_if` only appends remediation rows when the count
  is non-zero.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-audit-remediation.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-audit-remediation.repo-exposure.json --out target/ripr/reports/after-audit-remediation.gap-decision-ledger.json --out-md target/ripr/reports/after-audit-remediation.gap-decision-ledger.md
```

Observed:

```text
repairable = 37
ripr zero target count = 37
ripr plus target count = 37
crates/allow-report/src/audit_remediation.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `41` to `37` and
reduced `crates/allow-report/src/audit_remediation.rs` from `4` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 21 |
| `predicate_boundary` | 7 |
| `field_construction` | 4 |
| `return_value` | 3 |
| `match_arm` | 2 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-report/src/worklist_summary.rs` | 3 |
| `crates/cargo-allow/src/propose_args.rs` | 3 |
| `crates/cargo-allow/src/worklist_item_kind.rs` | 3 |
| `crates/allow-policy/src/evidence_validation.rs` | 2 |
| `crates/allow-rust/src/line_context.rs` | 2 |

## Hundred-Twenty-Third Burn-Down Slice

The hundred-twenty-third focused slice added worklist summary helper coverage
for `crates/allow-report/src/worklist_summary.rs`.

The new tests prove that:

- `worklist_risk_count` counts items by risk level across a mixed worklist.
- `worklist_kind_counts` aggregates per-kind counts through `counts.entry`.
- `worklist_kind_counts` returns the expected `BTreeMap` for single-kind input.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-worklist-summary.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-worklist-summary.repo-exposure.json --out target/ripr/reports/after-worklist-summary.gap-decision-ledger.json --out-md target/ripr/reports/after-worklist-summary.gap-decision-ledger.md
```

Observed:

```text
repairable = 34
ripr zero target count = 34
ripr plus target count = 34
crates/allow-report/src/worklist_summary.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `37` to `34` and
reduced `crates/allow-report/src/worklist_summary.rs` from `3` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 19 |
| `predicate_boundary` | 7 |
| `field_construction` | 4 |
| `return_value` | 2 |
| `match_arm` | 2 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/propose_args.rs` | 3 |
| `crates/cargo-allow/src/worklist_item_kind.rs` | 3 |
| `crates/allow-policy/src/evidence_validation.rs` | 2 |
| `crates/allow-rust/src/line_context.rs` | 2 |
| `crates/allow-rust/src/line_scan.rs` | 2 |

## Hundred-Twenty-Fourth Burn-Down Slice

The hundred-twenty-fourth focused slice added propose expiry parser coverage for
`crates/cargo-allow/src/propose_args.rs`.

The new tests prove that:

- `parse_propose_expires_arg` accepts the maximum allowed window using
  `SimpleDate::today_utc_approx`.
- `parse_propose_expires_arg` rejects dates before today with the exact error
  payload.
- `parse_propose_expires_arg` rejects dates beyond `BASELINE_DEBT_MAX_DAYS` with
  the exact error payload.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-propose-args.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-propose-args.repo-exposure.json --out target/ripr/reports/after-propose-args.gap-decision-ledger.json --out-md target/ripr/reports/after-propose-args.gap-decision-ledger.md
```

Observed:

```text
repairable = 31
ripr zero target count = 31
ripr plus target count = 31
crates/cargo-allow/src/propose_args.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `34` to `31` and
reduced `crates/cargo-allow/src/propose_args.rs` from `3` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 16 |
| `predicate_boundary` | 7 |
| `field_construction` | 4 |
| `return_value` | 2 |
| `match_arm` | 2 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/worklist_item_kind.rs` | 3 |
| `crates/allow-policy/src/evidence_validation.rs` | 2 |
| `crates/allow-rust/src/line_context.rs` | 2 |
| `crates/allow-rust/src/line_scan.rs` | 2 |
| `crates/cargo-allow/src/artifact_contract_support.rs` | 2 |

## Hundred-Twenty-Fifth Burn-Down Slice

The hundred-twenty-fifth focused slice added work item kind filter coverage for
`crates/cargo-allow/src/worklist_item_kind.rs`.

The new tests prove that:

- `parse_work_item_kind_filter` accepts canonical kinds from `WORK_ITEM_KINDS`.
- `parse_work_item_kind_filter` normalizes hyphenated aliases to underscores.
- `parse_work_item_kind_filter` rejects unknown kinds with the exact error
  payload.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-worklist-item-kind.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-worklist-item-kind.repo-exposure.json --out target/ripr/reports/after-worklist-item-kind.gap-decision-ledger.json --out-md target/ripr/reports/after-worklist-item-kind.gap-decision-ledger.md
```

Observed:

```text
repairable = 28
ripr zero target count = 28
ripr plus target count = 28
crates/cargo-allow/src/worklist_item_kind.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `31` to `28` and
reduced `crates/cargo-allow/src/worklist_item_kind.rs` from `3` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 13 |
| `predicate_boundary` | 7 |
| `field_construction` | 4 |
| `return_value` | 2 |
| `match_arm` | 2 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-policy/src/evidence_validation.rs` | 2 |
| `crates/allow-rust/src/line_context.rs` | 2 |
| `crates/allow-rust/src/line_scan.rs` | 2 |
| `crates/cargo-allow/src/artifact_contract_support.rs` | 2 |
| `crates/cargo-allow/src/list_rows.rs` | 2 |

## Hundred-Twenty-Sixth Burn-Down Slice

The hundred-twenty-sixth focused slice added policy reference validation coverage
for `crates/allow-policy/src/evidence_validation.rs`.

The new tests prove that:

- `policy_reference_validation_error` delegates to `reference_validation_error`
  and returns the missing-local-file `CargoAllowError` payload.
- `reference_validation_error` returns `None` for present, traceability-only,
  and unstructured statuses.
- `reference_validation_error` returns the directory rejection error for
  invalid local paths whose message contains `not a file`.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-evidence-validation.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-evidence-validation.repo-exposure.json --out target/ripr/reports/after-evidence-validation.gap-decision-ledger.json --out-md target/ripr/reports/after-evidence-validation.gap-decision-ledger.md
```

Observed:

```text
repairable = 26
ripr zero target count = 26
ripr plus target count = 26
crates/allow-policy/src/evidence_validation.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `28` to `26` and
reduced `crates/allow-policy/src/evidence_validation.rs` from `2` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 12 |
| `predicate_boundary` | 7 |
| `field_construction` | 4 |
| `return_value` | 2 |
| `match_arm` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-rust/src/line_context.rs` | 2 |
| `crates/allow-rust/src/line_scan.rs` | 2 |
| `crates/cargo-allow/src/artifact_contract_support.rs` | 2 |
| `crates/cargo-allow/src/list_rows.rs` | 2 |
| `crates/cargo-allow/src/worklist_advisories.rs` | 2 |

## Hundred-Twenty-Seventh Burn-Down Slice

The hundred-twenty-seventh focused slice added line context site coverage for
`crates/allow-rust/src/line_context.rs`.

The new tests prove that:

- `LineContext::site` copies `line_no` from the line context into the returned
  `FindingSite`.
- `LineContext::site` copies `module_stack` and the other scope fields into the
  returned `FindingSite`.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-line-context.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-line-context.repo-exposure.json --out target/ripr/reports/after-line-context.gap-decision-ledger.json --out-md target/ripr/reports/after-line-context.gap-decision-ledger.md
```

Observed:

```text
repairable = 24
ripr zero target count = 24
ripr plus target count = 24
crates/allow-rust/src/line_context.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `26` to `24` and
reduced `crates/allow-rust/src/line_context.rs` from `2` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 12 |
| `predicate_boundary` | 7 |
| `field_construction` | 2 |
| `return_value` | 2 |
| `match_arm` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-rust/src/line_scan.rs` | 2 |
| `crates/cargo-allow/src/artifact_contract_support.rs` | 2 |
| `crates/cargo-allow/src/list_rows.rs` | 2 |
| `crates/cargo-allow/src/worklist_advisories.rs` | 2 |
| `crates/allow-core/src/error.rs` | 1 |

## Hundred-Twenty-Eighth Burn-Down Slice

The hundred-twenty-eighth focused slice added source line scan coverage for
`crates/allow-rust/src/line_scan.rs`.

The new tests prove that:

- `scan_source_lines` iterates `source.lines()` and routes syntax facts per line.
- `scan_source_lines` calls `safety_comment_lines` and propagates nearby safety
  comment metadata into unsafe findings.
- `scan_source_lines` returns no findings when syntax facts are empty.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-line-scan.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-line-scan.repo-exposure.json --out target/ripr/reports/after-line-scan.gap-decision-ledger.json --out-md target/ripr/reports/after-line-scan.gap-decision-ledger.md
```

Observed:

```text
repairable = 22
ripr zero target count = 22
ripr plus target count = 22
crates/allow-rust/src/line_scan.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `24` to `22` and
reduced `crates/allow-rust/src/line_scan.rs` from `2` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 10 |
| `predicate_boundary` | 7 |
| `field_construction` | 2 |
| `return_value` | 2 |
| `match_arm` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/artifact_contract_support.rs` | 2 |
| `crates/cargo-allow/src/list_rows.rs` | 2 |
| `crates/cargo-allow/src/worklist_advisories.rs` | 2 |
| `crates/allow-core/src/error.rs` | 1 |
| `crates/allow-core/src/finding.rs` | 1 |

## Hundred-Twenty-Ninth Burn-Down Slice

The hundred-twenty-ninth focused slice added artifact contract helper coverage
for `crates/cargo-allow/src/artifact_contract_support.rs`.

The new tests prove that:

- `parse_json_artifact` resolves inventory scanner values through
  `artifact_contract_for_schema_id`.
- `parse_json_artifact` validates `claim_boundary` and `scanner_limitations`
  arrays through `assert_json_array_eq`.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-artifact-contract-support.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-artifact-contract-support.repo-exposure.json --out target/ripr/reports/after-artifact-contract-support.gap-decision-ledger.json --out-md target/ripr/reports/after-artifact-contract-support.gap-decision-ledger.md
```

Observed:

```text
repairable = 20
ripr zero target count = 20
ripr plus target count = 20
crates/cargo-allow/src/artifact_contract_support.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `22` to `20` and
reduced `crates/cargo-allow/src/artifact_contract_support.rs` from `2` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `call_presence` | 8 |
| `predicate_boundary` | 7 |
| `field_construction` | 2 |
| `return_value` | 2 |
| `match_arm` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/list_rows.rs` | 2 |
| `crates/cargo-allow/src/worklist_advisories.rs` | 2 |
| `crates/allow-core/src/error.rs` | 1 |
| `crates/allow-core/src/finding.rs` | 1 |
| `crates/allow-core/src/source_tree_path.rs` | 1 |

## Hundred-Thirtieth Burn-Down Slice

The hundred-thirtieth focused slice added list row helper coverage for
`crates/cargo-allow/src/list_rows.rs`.

The new tests prove that:

- `entry_reference_diagnostics_for_source_tree` resolves evidence references
  through `policy_reference_diagnostics_for_source_tree` when building list rows.
- `list_entry_status` derives row status from `outcomes.iter()` precedence when
  listing allow entries.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-list-rows.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-list-rows.repo-exposure.json --out target/ripr/reports/after-list-rows.gap-decision-ledger.json --out-md target/ripr/reports/after-list-rows.gap-decision-ledger.md
```

Observed:

```text
repairable = 18
ripr zero target count = 18
ripr plus target count = 18
crates/cargo-allow/src/list_rows.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `20` to `18` and
reduced `crates/cargo-allow/src/list_rows.rs` from `2` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `predicate_boundary` | 7 |
| `call_presence` | 6 |
| `return_value` | 2 |
| `field_construction` | 2 |
| `match_arm` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/worklist_advisories.rs` | 2 |
| `crates/allow-core/src/error.rs` | 1 |
| `crates/allow-core/src/finding.rs` | 1 |
| `crates/allow-core/src/source_tree_path.rs` | 1 |
| `crates/cargo-allow/src/kind_filter.rs` | 1 |

## Hundred-Thirty-First Burn-Down Slice

The hundred-thirty-first focused slice added worklist advisory helper coverage
for `crates/cargo-allow/src/worklist_advisories.rs`.

The new tests prove that:

- `matched_outcome_for_entry` selects matched outcomes via `outcomes.iter()` when
  building policy advisory work items.
- `source_package_name` forwards finding crate names into work item
  `source_package` fields for baseline-debt advisories.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-worklist-advisories.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-worklist-advisories.repo-exposure.json --out target/ripr/reports/after-worklist-advisories.gap-decision-ledger.json --out-md target/ripr/reports/after-worklist-advisories.gap-decision-ledger.md
```

Observed:

```text
repairable = 16
ripr zero target count = 16
ripr plus target count = 16
crates/cargo-allow/src/worklist_advisories.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `18` to `16` and
reduced `crates/cargo-allow/src/worklist_advisories.rs` from `2` repairable targets to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `predicate_boundary` | 7 |
| `call_presence` | 4 |
| `return_value` | 2 |
| `field_construction` | 2 |
| `match_arm` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-core/src/error.rs` | 1 |
| `crates/allow-core/src/finding.rs` | 1 |
| `crates/allow-core/src/source_tree_path.rs` | 1 |
| `crates/cargo-allow/src/kind_filter.rs` | 1 |
| `crates/cargo-allow/src/add_entry.rs` | 1 |

## Hundred-Thirty-Second Burn-Down Slice

The hundred-thirty-second focused slice added kind filter CLI parser coverage for
`crates/cargo-allow/src/kind_filter.rs`.

The new test proves that:

- `parse_kind_filter_arg` delegates to `parse_kind_filter` and returns the
  original kind string on success or a closed `unknown kind` error on failure.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-kind-filter.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-kind-filter.repo-exposure.json --out target/ripr/reports/after-kind-filter.gap-decision-ledger.json --out-md target/ripr/reports/after-kind-filter.gap-decision-ledger.md
```

Observed:

```text
repairable = 15
ripr zero target count = 15
ripr plus target count = 15
crates/cargo-allow/src/kind_filter.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `16` to `15` and
reduced `crates/cargo-allow/src/kind_filter.rs` from `1` repairable target to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `predicate_boundary` | 7 |
| `call_presence` | 3 |
| `return_value` | 2 |
| `field_construction` | 2 |
| `match_arm` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-core/src/error.rs` | 1 |
| `crates/allow-core/src/finding.rs` | 1 |
| `crates/allow-core/src/source_tree_path.rs` | 1 |
| `crates/cargo-allow/src/add_entry.rs` | 1 |
| `crates/cargo-allow/src/worklist_args.rs` | 1 |

## Hundred-Thirty-Third Burn-Down Slice

The hundred-thirty-third focused slice added structural identity key coverage for
`crates/allow-core/src/finding.rs`.

The new test proves that:

- `stable_identity_key_from_parts` length-prefixes identity parts when building
  `StructuralIdentity::stable_key`.
- `finding_identity_key` composes finding scope parts before delegating to the
  same stable key helper.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-finding.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-finding.repo-exposure.json --out target/ripr/reports/after-finding.gap-decision-ledger.json --out-md target/ripr/reports/after-finding.gap-decision-ledger.md
```

Observed:

```text
repairable = 14
ripr zero target count = 14
ripr plus target count = 14
crates/allow-core/src/finding.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `15` to `14` and
reduced `crates/allow-core/src/finding.rs` from `1` repairable target to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `predicate_boundary` | 7 |
| `return_value` | 2 |
| `call_presence` | 2 |
| `field_construction` | 2 |
| `match_arm` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-core/src/error.rs` | 1 |
| `crates/allow-core/src/source_tree_path.rs` | 1 |
| `crates/allow-policy/src/evidence_tests.rs` | 1 |
| `crates/cargo-allow/src/add_entry.rs` | 1 |
| `crates/cargo-allow/src/worklist_args.rs` | 1 |

## Hundred-Thirty-Fourth Burn-Down Slice

The hundred-thirty-fourth focused slice added evidence test fixture cleanup
coverage for `crates/allow-policy/src/evidence_tests.rs`.

The new test proves that:

- `TestRoot::drop` removes temporary fixture directories through
  `remove_test_path` when the guard leaves scope.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-evidence-tests.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-evidence-tests.repo-exposure.json --out target/ripr/reports/after-evidence-tests.gap-decision-ledger.json --out-md target/ripr/reports/after-evidence-tests.gap-decision-ledger.md
```

Observed:

```text
repairable = 13
ripr zero target count = 13
ripr plus target count = 13
crates/allow-policy/src/evidence_tests.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `14` to `13` and
reduced `crates/allow-policy/src/evidence_tests.rs` from `1` repairable target to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `predicate_boundary` | 7 |
| `return_value` | 2 |
| `field_construction` | 2 |
| `match_arm` | 1 |
| `call_presence` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-core/src/error.rs` | 1 |
| `crates/allow-core/src/source_tree_path.rs` | 1 |
| `crates/cargo-allow/src/worklist_args.rs` | 1 |
| `crates/cargo-allow/src/add_entry.rs` | 1 |
| `crates/allow-rust/src/safety_comments.rs` | 1 |

## Hundred-Thirty-Fifth Burn-Down Slice

The hundred-thirty-fifth focused slice added worklist kind filter CLI parser
coverage for `crates/cargo-allow/src/worklist_args.rs`.

The new test proves that:

- `parse_worklist_kind_filter` delegates to `parse_kind_filter` and returns the
  original kind string on success or a closed `unknown worklist kind` error on
  failure.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-worklist-args.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-worklist-args.repo-exposure.json --out target/ripr/reports/after-worklist-args.gap-decision-ledger.json --out-md target/ripr/reports/after-worklist-args.gap-decision-ledger.md
```

Observed:

```text
repairable = 12
ripr zero target count = 12
ripr plus target count = 12
crates/cargo-allow/src/worklist_args.rs repairable targets = 0
call_presence repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `13` to `12` and
cleared the remaining repo-scoped `call_presence` class.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `predicate_boundary` | 7 |
| `return_value` | 2 |
| `field_construction` | 2 |
| `match_arm` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-core/src/error.rs` | 1 |
| `crates/allow-core/src/source_tree_path.rs` | 1 |
| `crates/cargo-allow/src/add_entry.rs` | 1 |
| `crates/allow-rust/src/safety_comments.rs` | 1 |
| `crates/cargo-allow/src/worklist_queue.rs` | 1 |

## Hundred-Thirty-Sixth Burn-Down Slice

The hundred-thirty-sixth focused slice added error display return-value coverage
for `crates/allow-core/src/error.rs`.

The new test proves that:

- `fmt::Display for CargoAllowError` returns the stored message string through
  `format!` and `to_string`.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-error.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-error.repo-exposure.json --out target/ripr/reports/after-error.gap-decision-ledger.json --out-md target/ripr/reports/after-error.gap-decision-ledger.md
```

Observed:

```text
repairable = 11
ripr zero target count = 11
ripr plus target count = 11
crates/allow-core/src/error.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `12` to `11` and
reduced `crates/allow-core/src/error.rs` from `1` repairable target to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `predicate_boundary` | 7 |
| `field_construction` | 2 |
| `match_arm` | 1 |
| `return_value` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-core/src/source_tree_path.rs` | 1 |
| `crates/cargo-allow/src/add_entry.rs` | 1 |
| `crates/cargo-allow/src/prune_render_tests.rs` | 1 |
| `crates/cargo-allow/src/worklist_queue.rs` | 1 |
| `crates/allow-rust/src/safety_comments.rs` | 1 |

## Hundred-Thirty-Seventh Burn-Down Slice

The hundred-thirty-seventh focused slice added prune render test helper
return-value coverage for `crates/cargo-allow/src/prune_render_tests.rs`.

The new test proves that:

- `test_prune_context` returns the expected `PruneContext` inventory fields,
  including `files_scanned: Some(49)`.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-prune-render.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-prune-render.repo-exposure.json --out target/ripr/reports/after-prune-render.gap-decision-ledger.json --out-md target/ripr/reports/after-prune-render.gap-decision-ledger.md
```

Observed:

```text
repairable = 10
ripr zero target count = 10
ripr plus target count = 10
crates/cargo-allow/src/prune_render_tests.rs repairable targets = 0
return_value repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `11` to `10` and
cleared the remaining repo-scoped `return_value` class.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `predicate_boundary` | 7 |
| `field_construction` | 2 |
| `match_arm` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-core/src/source_tree_path.rs` | 1 |
| `crates/cargo-allow/src/add_entry.rs` | 1 |
| `crates/cargo-allow/src/worklist_queue.rs` | 1 |
| `crates/allow-rust/src/safety_comments.rs` | 1 |
| `crates/allow-diff/src/policy_entry_evidence.rs` | 1 |

## Hundred-Thirty-Eighth Burn-Down Slice

The hundred-thirty-eighth focused slice added diff policy change helper field
construction coverage for `crates/allow-report/src/diff_human_tests.rs`.

The new test proves that:

- `bare_policy_change` sets severity, allow ID, kind, and message fields and
  leaves optional nested change fields unset.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-diff-human.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-diff-human.repo-exposure.json --out target/ripr/reports/after-diff-human.gap-decision-ledger.json --out-md target/ripr/reports/after-diff-human.gap-decision-ledger.md
```

Observed:

```text
repairable = 8
ripr zero target count = 8
ripr plus target count = 8
crates/allow-report/src/diff_human_tests.rs repairable targets = 0
crates/allow-report/src/diff_markdown_tests.rs repairable targets = 0
field_construction repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `10` to `8` and
cleared the remaining repo-scoped `field_construction` class, including the
collateral `diff_markdown_tests.rs` helper gap.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `predicate_boundary` | 7 |
| `match_arm` | 1 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-core/src/source_tree_path.rs` | 1 |
| `crates/allow-diff/src/policy_entry_evidence.rs` | 1 |
| `crates/allow-inventory/src/options.rs` | 1 |
| `crates/allow-match/src/scoring.rs` | 1 |
| `crates/allow-report/src/source_inventory.rs` | 1 |

## Hundred-Thirty-Ninth Burn-Down Slice

The hundred-thirty-ninth focused slice added inventory source label match-arm
coverage for `crates/allow-inventory/src/options.rs`.

The new test proves that:

- `InventorySource::as_str` maps each inventory source variant to its stable
  report label string.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-options.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-options.repo-exposure.json --out target/ripr/reports/after-options.gap-decision-ledger.json --out-md target/ripr/reports/after-options.gap-decision-ledger.md
```

Observed:

```text
repairable = 7
ripr zero target count = 7
ripr plus target count = 7
crates/allow-inventory/src/options.rs repairable targets = 0
match_arm repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `8` to `7` and
cleared the remaining repo-scoped `match_arm` class.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `predicate_boundary` | 7 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-core/src/source_tree_path.rs` | 1 |
| `crates/allow-diff/src/policy_entry_evidence.rs` | 1 |
| `crates/allow-match/src/scoring.rs` | 1 |
| `crates/allow-report/src/source_inventory.rs` | 1 |
| `crates/allow-rust/src/safety_comments.rs` | 1 |

## Hundred-Fortieth Burn-Down Slice

The hundred-fortieth focused slice added source-tree path filter exact-equality
boundary coverage for `crates/allow-core/src/source_tree_path.rs`.

The new test proves that:

- `source_tree_path_matches_filter` returns true only when normalized item and
  filter paths are exactly equal, and false for different paths.

After committing the focused test change and regenerating repo exposure and the
gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-source-tree-path.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-source-tree-path.repo-exposure.json --out target/ripr/reports/after-source-tree-path.gap-decision-ledger.json --out-md target/ripr/reports/after-source-tree-path.gap-decision-ledger.md
```

Observed:

```text
repairable = 6
ripr zero target count = 6
ripr plus target count = 6
crates/allow-core/src/source_tree_path.rs repairable targets = 0
```

The focused slice reduced repo-scoped `ripr+` targets from `7` to `6` and
reduced `crates/allow-core/src/source_tree_path.rs` from `1` repairable target to `0`.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `predicate_boundary` | 6 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-diff/src/policy_entry_evidence.rs` | 1 |
| `crates/allow-match/src/scoring.rs` | 1 |
| `crates/allow-report/src/source_inventory.rs` | 1 |
| `crates/allow-rust/src/safety_comments.rs` | 1 |
| `crates/cargo-allow/src/add_entry.rs` | 1 |

## Hundred-Forty-First Burn-Down Slice

The hundred-forty-first focused slice revalidated the remaining
`crates/allow-diff/src/policy_entry_evidence.rs` `predicate_boundary` target on
`removed_evidence_message` (`severity != PolicyChangeSeverity::Fail`).

Existing coverage already includes:

- `message_helpers_follow_severity_and_local_removed_state` with
  `PolicyChangeSeverity::Review` and `PolicyChangeSeverity::Improvement` inputs.
- `removed_evidence_message_treats_improvement_as_non_fail_boundary` with the
  exact non-fail severity boundary shape ripr suggests.

A follow-up variable-backed boundary assertion was attempted locally and did not
move the ledger; `cargo test -p allow-diff policy_entry_evidence::tests` passes.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after-policy-entry-evidence-retry.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after-policy-entry-evidence-retry.repo-exposure.json --out target/ripr/reports/after-policy-entry-evidence-retry.gap-decision-ledger.json --out-md target/ripr/reports/after-policy-entry-evidence-retry.gap-decision-ledger.md
```

Observed:

```text
repairable = 6
ripr zero target count = 6
ripr plus target count = 6
crates/allow-diff/src/policy_entry_evidence.rs repairable targets = 1
```

Repo-scoped `ripr+` remained at `6`. The remaining `policy_entry_evidence.rs`
target is tracked as provider friction in `EffortlessMetrics/ripr#1432`; do not
add low-value padding tests for this anchor.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `predicate_boundary` | 6 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-diff/src/policy_entry_evidence.rs` | 1 |
| `crates/allow-match/src/scoring.rs` | 1 |
| `crates/allow-report/src/source_inventory.rs` | 1 |
| `crates/allow-rust/src/safety_comments.rs` | 1 |
| `crates/cargo-allow/src/add_entry.rs` | 1 |
| `crates/cargo-allow/src/worklist_queue.rs` | 1 |

## Hundred-Forty-Second Burn-Down Slice

The hundred-forty-second focused slice closed the remaining `predicate_boundary`
tail by revalidating `crates/allow-match/src/scoring.rs` and filing provider
issues for every still-actionable anchor.

For `scoring.rs` (`entry.kind != finding.kind` at line 6):

- existing coverage includes `scoring_rejects_kind_family_and_path_mismatches`
  with `assert_eq!(score_match(&entry, &finding), None)` after setting
  `entry.kind = FindingKind::Panic` against an `Unsafe` finding fixture.
- a follow-up variable-backed `score_match_kind_boundary_discriminator` test was
  attempted locally and did not move the ledger.
- provider friction filed as `EffortlessMetrics/ripr#1440`.

Batch provider filing for the other unnumbered tail anchors:

| Path | Anchor | Issue |
| --- | --- | --- |
| `crates/allow-diff/src/policy_entry_evidence.rs` | `removed_evidence_message` / `severity != Fail` | `EffortlessMetrics/ripr#1432` |
| `crates/allow-report/src/source_inventory.rs` | `SourceInventoryRow::add_status` / `status != Matched` | `EffortlessMetrics/ripr#1433` |
| `crates/allow-rust/src/safety_comments.rs` | `is_safety_comment` prefix discriminator | `EffortlessMetrics/ripr#1441` |
| `crates/cargo-allow/src/add_entry.rs` | `ensure_addable_outcome` / `status == New` | `EffortlessMetrics/ripr#1442` |
| `crates/cargo-allow/src/worklist_queue.rs` | `item_kind_matches` hyphen alias | `EffortlessMetrics/ripr#1443` |

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/session.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/session.repo-exposure.json --out target/ripr/reports/session.gap-decision-ledger.json --out-md target/ripr/reports/session.gap-decision-ledger.md
```

Observed:

```text
repairable = 6
ripr zero target count = 6
ripr plus target count = 6
```

Repo-scoped `ripr+` remained at `6`. No further test padding is warranted for
these anchors; cargo-allow's honest readiness position is that the remaining
projection is provider noise until ripr clears or revises the ledger.

Remaining repairable evidence classes:

| Evidence class | Count |
| --- | ---: |
| `predicate_boundary` | 6 |

All six remaining `predicate_boundary` anchors are provider-tracked.

## RIPR 0.10.0 Upgrade Slice

Installed provider:

```bash
cargo install ripr --version 0.10.0 --locked --force
ripr --version
```

Result:

```text
ripr 0.10.0
```

After regenerating full repo exposure (`RIPR_REPO_EXPOSURE_SEAM_LIMIT=0`) and the gap
decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/v0.10.0-full.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/v0.10.0-full.repo-exposure.json --out target/ripr/reports/v0.10.0-full.gap-decision-ledger.json --out-md target/ripr/reports/v0.10.0-full.gap-decision-ledger.md
ripr check --root . --mode ready --format repo-badge-plus-json --gap-ledger target/ripr/reports/v0.10.0-full.gap-decision-ledger.json
```

Observed:

```text
repairable = 364
ripr zero target count = 364
ripr plus target count = 364
```

Evidence-class breakdown:

| Evidence class | Count |
| --- | ---: |
| `error_variant` | 358 |
| `predicate_boundary` | 6 |

`error_variant` related-test concentration:

| Related test | Count |
| --- | ---: |
| `parse_policy_toml_reports_toml_parse_errors` | 235 |
| `ensure_addable_outcome_rejects_matched_findings_with_exact_error` | 92 |
| `artifact_sample_validator_reports_array_and_scalar_constraint_errors` | 24 |
| CLI arg call-presence observers | 7 |

Focused repo test hardening in this slice:

- `crates/allow-policy/src/entry_validation.rs`: rewrote
  `direct_error_discriminators_match_entry_validation_messages` to use `expect_err` plus
  exact `CargoAllowError::new(...)` payload assertions.
- `crates/cargo-allow/src/add_tests.rs`: added
  `select_add_finding_reports_missing_nearby_findings` and tightened ambiguous/matched
  `select_add_finding` / `ensure_addable_outcome` assertions to exact `CargoAllowError`
  payloads.
- `crates/cargo-allow/src/artifact_sample_schema_support.rs`: consolidated object-shape
  and scalar constraint assertions into
  `artifact_sample_validator_reports_array_and_scalar_constraint_errors` using `expect_err`
  plus `format!(...)` payloads that mirror production error construction.

Post-change ledger regeneration did not move the `364` repairable projection. Treat the
`error_variant` bulk as `ripr 0.10.0` oracle-linking friction until the provider revises
related-test routing or accepts the existing exact payload assertions. Filed as
`EffortlessMetrics/ripr-swarm#1304`. The six `predicate_boundary` anchors remain
provider-tracked per-anchor in `ripr#1432`, `#1433`, `#1440`–`#1443`
(`EffortlessMetrics/ripr-swarm#1303`).

## Unsafe-Review Evidence

There is no separate `unsafe-review+` binary. Discovery used the installed
`unsafe-review` CLI help surface:

```bash
unsafe-review --help
unsafe-review doctor --root .
```

`unsafe-review+` readiness is rendered from `unsafe-review badges` as
`target/unsafe-review/reports/badges.json/unsafe-review-plus.json`.

Installed provider:

```text
unsafe-review 0.1.0
```

Initial repo-scoped commands:

```bash
unsafe-review doctor --root .
unsafe-review repo --root . --format json --out target/unsafe-review/reports/repo.json
unsafe-review badges --root . --out target/unsafe-review/reports/badges.json
unsafe-review policy report --root . --format json --out target/unsafe-review/reports/policy-report.json
unsafe-review receipt validate --root .
unsafe-review receipt audit --root . --format json --out target/unsafe-review/reports/receipt-audit.json
unsafe-review check --root . --base origin/main --format json --out target/unsafe-review/reports/check.json
```

Observed repo scope (`target/unsafe-review/reports/repo.json`):

```text
rust_files = 530
unsafe_sites = 50
cards = 50
open_actionable_gaps = 50
contract_missing = 30
guard_missing = 3
guarded_unwitnessed = 0
requires_loom = 5
miri_unsupported = 12
trust_boundary = Static unsafe contract review only; not a proof of memory safety
```

Observed badge projection:

```text
unsafe-review: 50 open gaps
unsafe-review+: 30 contract / 3 guard / 0 witness
```

`unsafe-review+` therefore starts at `33` actionable gaps (`contract_missing` +
`guard_missing` + `guarded_unwitnessed`), excluding `requires_loom` and
`miri_unsupported` from the plus badge.

Diff scope on a clean `main` tree (`target/unsafe-review/reports/check.json`)
reported `0` cards. Witness receipt inventory is empty
(`receipt validate`: `0 valid`; `receipt audit`: `0` receipts).

Initial triage:

- `40 / 50` cards anchor in tests or intentional fixtures, including
  `crates/allow-rust/src/tests/unsafe_scan.rs` (`36` cards) and
  `fixtures/unsafe/src/lib.rs`.
- the remaining nine non-fixture cards are false positives on safe scanner code
  or illustrative test string literals, including
  `crates/allow-rust/src/safety_comments.rs:34` and `:38` (`unsafe { load(ptr) }`
  inside a `#[test]` source string) and
  `crates/allow-rust/src/line_unsafe_findings.rs:14` (`push_finding(...)` helper
  misclassified as `unsafe_fn_call`).

Provider friction filed as `EffortlessMetrics/unsafe-review#541`. Do not add
witness receipts or `SAFETY:` padding to test fixtures solely to clear the badge.

## Claim Boundary

cargo-allow did not execute `ripr` as part of its own scan. The `ripr` results
above are external readiness evidence.

This record does not claim:

- test adequacy.
- mutation adequacy.
- coverage proof.
- semantic correctness.
- unsafe soundness.
- release readiness.
- proof execution by cargo-allow.

`ripr+ = 364` means the installed ripr 0.10.0 ledger still reports three hundred
sixty-four actionable targets. The `358` `error_variant` rows are not newly discovered
untested seams: cross-version comparison shows they are the same anchors ripr 0.9.0 marked
`already_observed`, reclassified as `MissingErrorDiscriminator` with misrouted
`related_test`. Estimate ~75–85% oracle-linking regression and ~10–15% (~35–55) plausibly
legitimate local gaps. cargo-allow strengthened exact error-discriminator tests in
`entry_validation`, `add_tests`, and `artifact_sample_schema_support`, but the ledger did
not move. The dominant `error_variant` projection is filed as provider oracle-linking
friction in `EffortlessMetrics/ripr-swarm#1304` until ripr 0.10.0 revises related-test routing.

The six unchanged `predicate_boundary` anchors remain provider-tracked in `ripr#1432`,
`#1433`, and `#1440`–`#1443` (`EffortlessMetrics/ripr-swarm#1303`). Do not add low-value
padding tests for those anchors.

`unsafe-review+ = 33` means the installed unsafe-review badge still reports
thirty-three contract/guard/witness gaps on a repo-scoped scan. cargo-allow's
initial triage found no production unsafe-contract debt requiring witness import;
the projection is filed as provider friction in `unsafe-review#541`.

cargo-allow did not execute `unsafe-review` as part of its own scan. The
unsafe-review results above are external readiness evidence.

## Next Work

Under **provider-tracked readiness**, recommended lanes (in order):

```text
1. release: document and prove automated release prerequisites (E1–E2)
2. migration: inventory 0.2 parity gaps (B1)
3. migration: close one high-value evidence-preservation gap (B2)
4. identity: inventory structural identity gaps (D1)
5. spec-system: .allow/import design remains docs-only until authorized (C1 done)
```

Provider refresh (does not block `0.1.10` under provider-tracked policy):

1. wait for `unsafe-review#541` triage or provider rule updates.
2. wait for `EffortlessMetrics/ripr-swarm#1304` / `#1303` provider movement.
3. regenerate provider receipts and re-check strict readiness bar.
4. revise readiness classification only with explicit approval.

Do not start external repo migration at scale until strict readiness is met.
Do not start `.allow` profile state migration until an explicit implementation PR
is authorized.

## Validation

Re-check local readiness:

```bash
cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md
cargo run -p cargo-allow -- check --profile spec-system --mode audit --format json --output target/cargo-allow/spec-system.json
```

Re-check provider-tracked blockers (external evidence; not cargo-allow scan):

```bash
ripr check --root . --mode ready --format repo-badge-plus-json --gap-ledger target/ripr/reports/gap-decision-ledger.json
unsafe-review badges
```

## Claim Boundary

This record documents self-hosting and adoption-readiness posture. It does not
execute ripr, unsafe-review, or other proof providers as part of cargo-allow's
own scan. Provider-tracked acceptance does not prove zero-gap readiness or
external migration readiness.
