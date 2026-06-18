# Changelog

All notable changes to cargo-allow are documented here.

cargo-allow is a direct source-tree exception ledger for Rust repositories.
Release notes preserve the claim boundary: cargo-allow scans source-tree
inventory without executing repository code.

## [Unreleased]

### Added

- Add import-parity owner/reason/evidence acceptance fixture matrix (#1717):
  table-driven `import_parity_metadata_acceptance_tests.rs` characterizes
  semantic-selector governance round-trip (`owner`, `reason`, `evidence`,
  legacy `covered_by`) across no-panic, lint, and unsafe lanes; surfaces weak
  or missing evidence as visible debt without laundering into reviewed approval;
  and verifies multi-family batch import preserves governance metadata. Fixtures:
  `tests/fixtures/migration/no-panic-allowlist-semantic-selectors-covered-by.toml`,
  `lint-exception-semantic-selectors-covered-by.toml`.
- Add multi-family legacy ledger import model (`LegacyImportBatch`,
  `LegacyImportFamily`) for policy-directory batch migration: absorbs panic-family,
  lint-attribute, and other compat lanes in deterministic lane-descriptor order
  while preserving per-lane entry families, finding kinds, and owner/reason
  metadata without collapsing families. Fixture:
  `tests/fixtures/migration/no-panic-allowlist.toml`,
  `panic-baseline.toml`, `lint-exception.toml`.
- Add `cargo-allow refresh --allow-id <id>` to record operator-approved advisory
  drift refresh for entries with `location_drift` outcomes. Writes
  `cargo-allow.refresh.v1` receipts that update `last_seen` (and selector
  `line_hint`) without changing lifecycle dates. Fixture:
  `tests/fixtures/refresh/advisory-drift/`.
- Import legacy semantic selector fields (`receiver` / `receiver_fingerprint`,
  `target` / `target_fingerprint`, `symbol`, `normalized_snippet_hash`) from
  nested `[allow.selector]` tables into canonical policy entries for
  no-panic-allowlist and clippy-exceptions migrations. Fixture:
  `tests/fixtures/migration/no-panic-allowlist-semantic-selectors.toml`.
- Extend adoption-substrate lane closeout (CARGO-ALLOW-CLOSEOUT-0003) after
  structural identity D7 diff posture identity characterization lands in #1732;
  close structural identity execution lane D1–D7 (D8 docs deferred).
- Add fixture-backed `allow-diff` posture characterization tests asserting
  policy selector precision weakening when identity fields loosen, improvement
  when fields tighten, equal-precision selector identity retarget review, and
  finding identity loss between structural-identity refactor sides. Covers D2–D5
  fixture policy entries `allow-0215`..`0234` and `allow-0243`..`0246`; existing
  diff logic passes without changes.
- Add fixture-backed `allow-match` selector precision characterization tests
  asserting policy selectors uniquely match intended structural-identity
  findings via `container`, `receiver_fingerprint`, `target_fingerprint`,
  `symbol`, and `normalized_snippet_hash`. Covers D2–D5 fixture policy entries
  `allow-0215`..`0234` and `allow-0243`..`0246`; existing matcher passes
  without changes.
- Harden lint `#[allow(...)]` / `#[expect(...)]` attribute target identity so
  the same lint on different items yields distinct stable keys via `container`
  (and `module` for inner module attributes) while preserving
  `target_fingerprint` policy references. Scope collection now records
  `inner_attribute_item` lines alongside outer `attribute_item` targets.
  Characterization covers same-line, `cfg_attr`, multiline, shared-policy, and
  inner module/impl attributes. Fixture: `lint_same_different_items`.
- Map simple Rust parameter identifiers to structural receiver fingerprints
  (`param:N`) for panic method calls and index receivers, preserving identity
  across rename-only refactors while still distinguishing different parameter
  slots. Non-identifier receivers keep normalized expression text. Index
  `target_fingerprint` now records the bracket selector instead of mirroring
  the receiver. Fixtures: `rename_local`, `callee_same_receiver_diff`,
  `index_same_form_diff_targets`.
- Extend adoption-substrate lane closeout (CARGO-ALLOW-CLOSEOUT-0003) after
  structural identity D4 receiver/target fingerprints land in #1726.
- Extend adoption-substrate lane closeout (CARGO-ALLOW-CLOSEOUT-0003) after
  structural identity D5 lint attribute target identity lands in #1728.
- Register adoption-substrate lane closeout (CARGO-ALLOW-CLOSEOUT-0003) after the
  10-PR cleanup queue lands on main: modularization, advisory ratcheting,
  governance split, two in-repository dogfood receipts, and structural identity
  D3 container module-qualification.
- Qualify unqualified Rust `container` identity with the module path for findings
  inside nested modules, disambiguating sibling modules that share a free-function
  name (`inner::access` vs `access`). Impl/trait/extern containers that already
  include `::` are unchanged. Fixture: `container_same_name_sibling_modules`.
- Add second in-repository migration parity dogfood receipt for the characterized
  `unsafe-allowlist` lane: compat check, migrate, canonical check, worklist,
  and closeout artifacts under `docs/dogfood/`.
- Add optional `[lanes.<kind>]` policy posture with `mode = "advisory" | "shadow" | "blocking"`.
  Check honors per-lane posture in read/report paths: shadow and advisory lanes keep
  findings visible without failing `no-new`/`strict` unless `--deny` promotes receipt advisory
  counts; blocking lanes retain existing gate behavior. Receipts emit `lane_posture` with
  effective mode per configured or scanned kind (#1473).
- Add `check --deny <status>` to promote individual receipt `advisory` count classes to
  blocking failures without changing check mode. Repeatable; supported classes mirror receipt
  advisory fields. `occurrence_headroom` remains unavailable (#1472).
- Add receipt-visible `advisory` counters to `cargo-allow.receipt.v1` check artifacts so
  CI and ratcheting workflows can read review-oriented status totals (`review_items`,
  `review_due`, `stale`, `baseline_debt`, optional policy/evidence-health counts) without
  parsing human reports. Markdown check reports include a matching `## Advisory counts`
  section. Exit status is unchanged without `--deny`; per-class `--deny` escalation is available
  for receipt `advisory` fields except deferred `occurrence_headroom` (#1472).
- Normalize migrate `closeout.next_queues` construction across legacy compat lanes
  using shared lane-descriptor debt classification and `CloseoutQueueHints`
  (`migration_closeout`, `migrate_closeout_queues`). Panic-baseline migrations
  now label baseline-debt queues consistently; follow-up and closeout queues
  share one builder without migration behavior change beyond routing labels.
- Add shared migration metadata helpers in `allow-policy-legacy`
  (`preserve_metadata`, `preserve_evidence`, `preserve_evidence_with_fallback`,
  `extend_evidence_with_markers`, `map_lifecycle`, `map_baseline_debt_lifecycle`,
  `map_occurrence_limit`, `classify_baseline_debt`) and route clippy, unsafe,
  dependency-surface, panic-baseline, and non-rust file converters through them
  without migration behavior change.
- Add shared migration lane descriptor table in `allow-policy-legacy` covering all
  11 supported legacy compat kinds (`CompatKind`, `LegacyLaneDescriptor`,
  `LegacyInputKind`, `EvidencePolicy`, `LifecyclePolicy`, `DebtPolicy`,
  `ExpectedCanonicalShape`, optional `CloseoutQueueHints`). Refactor fixture
  matrix and legacy filename lookups to table-drive from descriptors without
  migration behavior change.
- Add post-publish install-smoke job to the release workflow for tag pushes:
  installs `cargo-allow` from crates.io and verifies `--version`, `doctor`,
  `check --help`, and `doctor --profile spec-system --help` (skipped on
  workflow_dispatch dry-run).
- Add `scripts/release-install-smoke.sh` and
  `scripts/test-release-install-smoke.sh` for release install verification.
- Add `scripts/verify-crate-registry-version.sh` and wire the release publish job
  to wait for exact crates.io index visibility after each upload.
- Fix workflow_dispatch dry-run semantics: validate all workspace crates via
  preflight `cargo package --workspace --locked`, then run a single
  `cargo publish --dry-run` for leaf crate `allow-core` instead of failing on
  unpublished internal dependency versions.
- Add `scripts/release-version-preflight.sh` and wire it into the release
  publish job to guard tag/workspace version alignment, internal dependency
  version consistency, CHANGELOG sections, and release-record artifacts before
  crates.io upload (release-record checks skip on workflow_dispatch dry-run).

### Testing

- Add structural identity refactor-pair fixture matrix under
  `tests/fixtures/structural-identity/` with characterization tests in
  `allow-rust` covering line/function/module movement, receiver renames, lint
  target disambiguation, macro path keys, and indexing targets.

### Added

- Policy discovery prefers `policy/cargo-allow.toml` when present, recognizes the
  `policy = "cargo-allow"` dialect marker in `policy/allow.toml`, and skips
  foreign-dialect candidates with named diagnostics instead of hard-failing on
  the first path hit. Explicit `--config` precedence is unchanged.

### Documentation

- Register import parity execution lane in `.codex/goals/active.toml` after
  adoption-substrate and structural identity D1–D7 closeout: queue #1713
  (semantic selector fields) as the first import slice; #1714–#1718 remain
  sequenced siblings. Release/OIDC publish lanes stay dormant; `.allow` namespace
  import remains design-only (CARGO-ALLOW-SPEC-0004).
- Split umbrella #1466 into six owned child issues for import parity governance:
  #1713 (semantic selector fields), #1714 (advisory drift / last_seen),
  #1715 (recorded re-bless receipts), #1716 (multi-family legacy ledger model),
  #1717 (owner/reason/evidence acceptance fixture), #1718 (ripr-style adoption
  receipt). Umbrella #1466 remains open; update `gap-inventory.md` and
  `adoption-substrate-pr-005` in `.codex/goals/active.toml`.
- Reconcile migration parity and adoption-substrate execution state after B3–B6
  groundwork: mark B1–B6 done in `.codex/goals/active.toml`, close #1470 in
  gap-inventory, record #1466 umbrella split as adoption-substrate-pr-005, pivot
  active lane to modularization PRs 2–6, and defer `0.1.10` release cut pending
  adoption/cleanup (release automation #1703–#1705 dormant on `main`).
- Add first in-repository migration parity dogfood receipt for the characterized
  `no-panic-baseline` lane: compat check, migrate, canonical check, worklist,
  and closeout artifacts under `docs/dogfood/`.
- Reconcile migration parity execution state: mark goal registration and B2
  no-panic-baseline slice complete, populate `plans/migration-parity/gap-inventory.md`
  from `allow-policy-legacy` characterization and open issues #1466/#1470, and set
  B3 fixture matrix as the next active goal work item.
- Record provider-tracked self-hosting readiness policy: strict vs
  provider-tracked definitions, `0.1.10` path acceptance, and honest external
  migration blockers (`ripr+`, `unsafe-review+` remain filed upstream).
- Register improvement-lane specs and plans for readiness, migration parity
  queue, `.allow`/import design, structural identity quality, and `0.1.10`
  adoption-trust release sequencing.
- Document automated release prerequisites in `docs/release/README.md`: Trusted
  Publishing checklist for all ten crates, token fallback, workflow_dispatch
  dry-run steps, publish-order verification, recovery/yank guidance, and links
  to the `0.1.10` release plan (E1/E2).

### Migration

- Add `closeout` routing to `cargo-allow.migrate.v1` summaries: preserved
  counts, baseline-debt and evidence-debt signals, phased `next_queues`, and
  legacy-file retirement readiness for imported compat sources.
- Preserve optional owner, reason, lifecycle, and legacy `evidence`/`covered_by`
  fields when migrating `no-panic-baseline` entries; entries without evidence
  still emit visible `baseline_debt` traceability markers and keep
  `occurrence_limit` from legacy `count`.

### Testing

- Add `tests/fixtures/migration/` characterization matrix across supported legacy
  compat lanes with table-driven `allow-policy-legacy` tests for parse preservation,
  evidence/covered_by, lifecycle, occurrence limits, visible `baseline_debt`,
  deterministic reruns, policy-dir batch import, and compat loader smoke checks.
- Harden `add` load, validate, and write error paths (#1685).
- Add exact diff missing-policy config discriminator coverage (#1681).
- Cover explicit `diff --config` missing behavior in revisions (#1683).

### CI

- Add tag-triggered release workflow (#1684).

## [0.1.9] - 2026-06-16

### Receipt adoption

- `check` receipts now populate `counts.review_due` from matcher outcomes and
  fail `strict`/`release` on review-due entries while keeping `audit`/`no-new`
  advisory.
- `--receipt` writes an `error` receipt (or overwrites stale JSON) on exit-2
  validation failures instead of leaving the previous run's evidence in place.
- Receipts record effective `mode`, `enforcement`, `policy_config`, and
  `tool_version` provenance for gate consumers.
- Policy parsing accepts integer `schema_version` values and names the ledger
  file path in TOML parse errors.

### Maintenance

- Test hardening across receipt integration, matcher lifecycle, and legacy
  compat characterization.
- Module decomposition and adoption-path cleanup carried in the 0.1.9 lane stack.
- ripr `0.10.0` readiness work remains validation and fixture alignment only;
  this release does not add new scanner features.

## [0.1.8] - 2026-06-12

### Spec-system preview cleanup

- Made `cargo-allow init --profile spec-system` easier to adopt in new
  repositories by starting bootstrap active-goal validation as optional until a
  real proposal/spec/plan graph is registered, avoiding an immediate
  self-invalidating first-hour `doctor --profile spec-system` result.
- Simplified spec-system Markdown finding summaries so advisory reports use one
  neutral `Findings` section instead of repeating `Advisory Findings`.

## [0.1.7] - 2026-06-12

### Spec-system preview

- Added `spec-system` as an opt-in governance profile for static source-tree
  graph validation across proposals, specs, support tiers, active goals,
  implementation plans, closeouts, policy ledgers, and related proof-command
  fields.
- Added `cargo-allow check --profile spec-system`, `audit --profile
  spec-system`, `worklist --profile spec-system --format json`, `doctor
  --profile spec-system`, `init --profile spec-system`, and `explain
  <artifact-id> --profile spec-system` preview surfaces.
- Added the `cargo-allow.spec-system.v1` JSON report shape with artifacts,
  links, findings, work items, setup readiness, single-artifact explanation,
  scanner limitations, and the structural source-tree claim boundary.
- Dogfooded the profile in this repository with advisory CI artifacts, shadow
  mode, clean shadow burn-in evidence, blocking-eligible structural finding
  classification, repo-local blocking posture for selected structural findings,
  and reviewer/agent-oriented report and worklist posture.
- Added first-hour adoption and CI guidance for treating `spec-system` as one
  opt-in governance profile, not default cargo-allow behavior.
- Added opt-in profile architecture and cross-repo adoption guidance so
  spec-system portability issues can feed back into cargo-allow instead of
  becoming per-repo workarounds.

### Known limitations

- The profile is a preview and remains opt-in.
- The cargo-allow repo runs the profile in blocking mode for selected
  structural findings, while lifecycle and judgment-heavy checks remain
  advisory.
- The profile validates structural graph relationships only; it does not
  execute proof commands, call GitHub APIs, run Cargo, rustc, Clippy, build
  scripts, proc macros, ripr, unsafe-review, coverage, or network checks.
- The profile does not claim semantic correctness, proof execution, release
  readiness, unsafe soundness, test adequacy, or coverage proof.

## [0.1.6] - 2026-06-03

### Migration

- Preserved recognized and unstructured evidence when migrating Clippy,
  no-panic, non-Rust, generated-file, executable-bit, workflow, dependency,
  process, and network legacy policy lanes.
- Honored root-relative evidence references while converting legacy `from`
  sources, keeping migrated entries traceable to their original evidence.

### Scanner identity

- Recorded more precise source-syntax identity for unsafe impls, unsafe extern
  blocks, unsafe item containers, unsafe attribute targets, trait-method
  containers, and extern signatures.
- Split lint-attribute target scope more clearly so retained `#[allow(...)]`
  and `#[expect(...)]` findings are easier to review and narrow.
- Strengthened panic-family, nested panic receiver, index expression, and string
  slicing findings without claiming type-aware or control-flow analysis.

### Policy and reports

- Kept the panic fixture policy aligned with the current source-syntax scanner
  shape without weakening the no-new source-tree claim.
- Preserved the retained-exception posture model: findings stay owned,
  evidenced, reviewable, and difficult to silently broaden.

### Documentation

- Updated repository agent guidance to match the current PR, swarm, and release
  operating model.
- Recorded the completed 0.1.6 release evidence, publication order, registry
  verification, install smoke, no-new receipt, and rollback limits in
  `docs/release/0.1.6.md`.

### Known limitations

- Source-syntax only.
- No macro expansion.
- No type analysis.
- No MIR, control-flow, or data-flow analysis.
- No repository code execution.
- No proof that retained unsafe code is correct.
