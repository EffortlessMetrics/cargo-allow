# Self-Hosting Readiness

This record tracks whether cargo-allow is ready to be the source-of-truth
governance example for external repository adoption.

The target state is:

```text
docs gate: passed
default cargo-allow no-new: passed
spec-system profile: passed
ripr+: 0 actionable gaps
unsafe-review+: 0 actionable gaps
```

Current state: **not ready for external migration**.

## Current Result

Recorded: 2026-06-13

| Surface | Status | Evidence |
| --- | --- | --- |
| docs gate | passed | `cargo test --doc --workspace`; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`; CI run `27455099250` passed both steps on `main`. |
| workspace fmt/clippy/tests | passed | `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace` reported `1370 passed`. |
| default cargo-allow no-new | passed | installed `cargo-allow 0.1.8`; `cargo-allow check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md` reported `625` scanned files, `118` matched findings, `0` new findings, and `0` stale receipts. |
| spec-system profile | passed | installed `cargo-allow 0.1.8`; `cargo-allow check --profile spec-system --mode audit --format json --output target/cargo-allow/spec-system.json` reported `6` artifacts, `17` links, `4` support-tier rows, `0` findings, and `0` work items. |
| spec-system worklist | passed | installed `cargo-allow 0.1.8`; `cargo-allow worklist --profile spec-system --format json --output target/cargo-allow/spec-system-worklist.json` reported `0` findings and `0` work items. |
| ripr doctor | passed | installed `ripr 0.9.0`; `ripr doctor` passed and selected `ripr first-pr --root . --base origin/main --head HEAD` as the safe next action. |
| ripr+ repo readiness | blocked | `ripr` explicit gap-ledger projection reported `1340` `ripr` targets and `1340` `ripr+` targets after the thirty-sixth burn-down slice. |
| unsafe-review+ readiness | not run | Deferred until the `ripr+` readiness blocker is resolved. |

## RIPR Evidence

The installed provider was:

```bash
ripr --version
```

Result:

```text
ripr 0.9.0
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

`ripr+ = 1340` means the current repo does not yet meet the requested
self-hosting readiness bar. Do not move `ripr` or other external repositories
onto cargo-allow/spec-system as a readiness claim until this is resolved or the
readiness bar is explicitly revised.

## Next Work

Recommended next lane:

```text
evidence: reduce or scope cargo-allow ripr+ readiness
```

Start with one high-volume, low-judgment class:

1. inspect a pure parser, path, or rendering target such as
   `crates/allow-policy-legacy/src/parser_dependency_entries.rs` or
   `crates/allow-policy-legacy/src/fields.rs`.
2. choose one `MissingBoundaryAssertion`, `MissingValueAssertion`, or
   `MissingSideEffectObserver` group with direct behavior assertions.
3. add or tighten a focused test.
4. regenerate `target/ripr/reports/repo-exposure.json`.
5. regenerate `target/ripr/reports/gap-decision-ledger.json`.
6. verify the `ripr+` target count moves down.

The largest remaining files,
`crates/allow-policy/src/spec_system/validate.rs` and
`crates/allow-policy/src/entry_validation.rs`, are concentrated in
`MissingErrorDiscriminator` findings and have shown provider-sensitive
behavior in earlier slices. Prefer lower-risk pure helper coverage unless a
slice intentionally targets that provider behavior.

If provider behavior is noisy or non-portable, file a ripr issue with:

- `ripr --version`.
- the exact command.
- the relevant excerpt from `target/ripr/reports/gap-decision-ledger.json`.
- why the finding blocks cargo-allow self-hosting adoption.
- the claim boundary above.
