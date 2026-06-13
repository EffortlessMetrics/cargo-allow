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
| workspace fmt/clippy/tests | passed | `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace` reported `1234 passed`. |
| default cargo-allow no-new | passed | installed `cargo-allow 0.1.8`; `cargo-allow check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md` reported `625` scanned files, `118` matched findings, `0` new findings, and `0` stale receipts. |
| spec-system profile | passed | installed `cargo-allow 0.1.8`; `cargo-allow check --profile spec-system --mode audit --format json --output target/cargo-allow/spec-system.json` reported `6` artifacts, `17` links, `4` support-tier rows, `0` findings, and `0` work items. |
| spec-system worklist | passed | installed `cargo-allow 0.1.8`; `cargo-allow worklist --profile spec-system --format json --output target/cargo-allow/spec-system-worklist.json` reported `0` findings and `0` work items. |
| ripr doctor | passed | installed `ripr 0.9.0`; `ripr doctor` passed and selected `ripr first-pr --root . --base origin/main --head HEAD` as the safe next action. |
| ripr+ repo readiness | blocked | `ripr` explicit gap-ledger projection reported `2162` `ripr` targets and `2162` `ripr+` targets after the tenth burn-down slice. |
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

`ripr+ = 2162` means the current repo does not yet meet the requested
self-hosting readiness bar. Do not move `ripr` or other external repositories
onto cargo-allow/spec-system as a readiness claim until this is resolved or the
readiness bar is explicitly revised.

## Next Work

Recommended next lane:

```text
evidence: reduce or scope cargo-allow ripr+ readiness
```

Start with one high-volume, low-judgment class:

1. inspect `crates/allow-diff/src/policy_compare.rs`.
2. choose one `MissingBoundaryAssertion`, `MissingValueAssertion`, or
   `MissingSideEffectObserver` group.
3. add or tighten a focused test.
4. regenerate `target/ripr/reports/repo-exposure.json`.
5. regenerate `target/ripr/reports/gap-decision-ledger.json`.
6. verify the `ripr+` target count moves down.

If provider behavior is noisy or non-portable, file a ripr issue with:

- `ripr --version`.
- the exact command.
- the relevant excerpt from `target/ripr/reports/gap-decision-ledger.json`.
- why the finding blocks cargo-allow self-hosting adoption.
- the claim boundary above.
