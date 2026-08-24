# Changelog

All notable changes to cargo-allow are documented here.

cargo-allow is a direct source-tree exception ledger for Rust repositories.
Release notes preserve the claim boundary: cargo-allow scans source-tree
inventory without executing repository code.

## [Unreleased]

## [0.2.0-rc.1] - 2026-08-24

## [0.2.0] - 2026-07-19

### Added

- Incremental scan cache: `allow-rust::ScanCache` caches parsed findings
  keyed by file mtime+size. On a repeat scan within the same process, files
  whose mtime+size hasn't changed are served from cache instead of
  re-parsing. This is the foundation for faster edit→check loops (#2514).
  The cache is in-memory only (thread-local), conservatively correct (any
  cache miss falls through to a full re-parse), and applied transparently
  in `load_world`/`load_world_with_evidence_mode`.
- Typed manifest validation with `Complete`/`Incomplete` result gate:
  `validate_release_manifest` checks schema, auth_source, crate order, and
  checksums before attestation (#2495/#2497). The `PUBLISH_ORDER` constant
  is the single source of truth for the ten-crate publish order.
- Scanner completeness tracking: `RustScanResult` carries `files_skipped`
  from the scanner. `check --mode no-new` fails closed when tracked `.rs`
  files were unreadable/non-UTF8/oversized, preventing false-clean receipts
  (#2486/#2493).
- Atomicity containment: all seven live-ledger mutation commands now call
  `assert_path_within_root` before acquiring the lock (#2490). The
  `MutationLock` key is lexically canonicalized so alias-convergent paths
  acquire the same lock (#2487/#2489).
- Release workflow generates a `ReleaseManifestV1` manifest, attests it with
  keyless OIDC-backed build provenance (`actions/attest-build-provenance`), and
  attaches the manifest + SHA-256 to the GitHub Release. The manifest binds
  repository, tag, commit, tree, version, crate checksums, auth source,
  workflow run ID, MSRV, proven platforms, and claim boundary. The attestation
  binds the manifest to the exact workflow identity that produced it (#2280).
- `scripts/generate-release-manifest.sh` generates a
  `cargo-allow.release-manifest.v1` JSON manifest from release workflow context
  (version, git identity, auth source, MSRV, proven platforms, crate checksums)
  and emits a SHA-256 sidecar. Runs after publish + install-smoke succeed in the
  release workflow (#2279).
- `ReleaseManifestV1` (`cargo-allow.release-manifest.v1`) typed schema in
  `allow-report`: binds repository, tag, commit, tree, version, source candidate,
  crates with checksums, auth source, workflow run ID, MSRV, proven platforms,
  schema/tool generations, limitations, and claim boundary into one deterministic
  JSON artifact. This is the foundation for #2279/#2280 — the generator script
  and signing/attachment workflow build on this schema.

### Changed

- Raised minimum supported Rust version from 1.85 to 1.95 for the 0.2.0 train
  (#2371). The CI MSRV lane now proves Rust 1.95.0. The 0.1.x package line
  remains on 1.85.
- SHA-pinned `actions/checkout`, `actions/upload-artifact`,
  `rust-lang/crates-io-auth-action`, and `softprops/action-gh-release` by commit
  SHA across `release.yml` and `ci.yml` (#1896). `dtolnay/rust-toolchain` and
  `Swatinem/rust-cache` remain tag-pinned (follow-up).
- Made crates.io OIDC Trusted Publishing fail-closed in `release.yml` by
  removing `continue-on-error: true` from the OIDC step (#2281). A release
  that cannot authenticate via OIDC now fails instead of silently falling back
  to the `CARGO_REGISTRY_TOKEN` secret.

### Added

- Performance budget smoke (`scripts/perf-budget-smoke.sh`) measures wall-clock
  elapsed time for the critical operator-loop commands (audit, check, why,
  diff) and writes a structured receipt. Initial baseline documented in
  `docs/performance-budgets.md`. The `why` single-file fast path measures
  ~240ms vs ~22s for full `audit` — a 73x improvement.

- `allow-core` defines a versioned actionable-diagnostic kernel
  (`CargoAllowDiagnosticV1` / `CargoAllowActionV1` / `CargoAllowDiagnosticBatchV1`)
  so every output surface can share one semantic finding/repair object. It keeps
  four judgment dimensions independent — severity, rule posture, confidence, and
  result class (finding vs. stale vs. not-proven vs. unsupported vs. instrument
  failure) — carries closed missing-obligation and typed-action vocabularies,
  explicit source ranges with encoding/base/provenance contracts (line-only
  locations stay explicitly degraded), and a deterministic identity fingerprint
  that survives message/format changes but changes when the rule, subject,
  location, obligation, result class, or snapshot basis changes (encoding and
  position base bind too, so a UTF-8/one-based location never collides with a
  UTF-16/zero-based one). Only deterministic, non-inventive actions may be
  marked automatic — never a policy exemption or a previewable edit. This slice
  is the typed
  kernel and fixtures; renderer projection parity, JSON schema wiring, and
  safe-edit preview/apply are follow-ups. (#2188)
- `allow-diff` exposes one exact, typed repository-snapshot identity
  (`repository_snapshot`, `resolve_revision_identity`, `resolve_dirty_state`)
  so spec plans, captured evidence, and proof receipts share common Git/source
  freshness inputs instead of each reimplementing revision interpretation. The
  `RepositorySnapshotIdentity` binds a checkout-independent repository root
  identity (derived from root-commit ids, never an absolute path), object
  format, resolved head/base commit and tree, merge base, a distinct
  worktree/index dirty state (clean vs. tracked-modified / staged / untracked /
  unavailable / non-repository), and a deterministic selected-source closure
  hash over caller-supplied load-bearing paths. Git failures and missing bases
  fail visibly rather than degrading to a clean snapshot. This slice implements
  the committed-head and committed-range kinds; staged/working-tree/external
  kinds and consumer wiring are follow-up. (#2225)
- `cargo-allow init` now writes the starter policy via the durable `write_file`
  atomic-replace path instead of raw `fs::write`, matching every other mutation
  command. An interrupted `init` can no longer leave a partial policy file.

### Changed

- Corrected the product description in `docs/design.md`: cargo-allow is a
  source-syntax policy linter and durable exception ledger, not "not a linter."
  The adjacent-tool boundary (does not replace rustc lints, Clippy, etc.) is
  preserved.

- `cargo-allow add --from-plan <PATH>` consumes a `cargo-allow.add-finding-plan.v1`
  artifact (from `why --plan`) as a fail-closed live-ledger transaction. It
  acquires the mutation lock, strictly parses the v1 plan, recomputes the
  repository / inventory / policy / source / finding / selector bindings against
  a fresh scan, requires the exact finding to remain uniquely `New`, validates
  the complete policy, and atomically replaces the ledger. The allow entry is
  built canonically from the freshly re-selected finding plus operator judgment
  fields — never by deserializing approval metadata from the plan — and the
  route requires `--update` while conflicting with manual target selectors,
  `--write`, and `--force`. Every stale or malformed case (unsupported
  schema/tool generation, different repository or policy path, policy / source /
  inventory drift, missing or changed finding, selector drift, ambiguous or
  non-`New` posture, replay after success) fails without changing policy. On
  success it emits a `cargo-allow.add-plan-application.v1` receipt binding the
  plan digest, finding digest, before/after policy digests, added allow ID, and
  target ledger, with an honest `targeted_recheck = not_executed` plus the
  full-check argv the operator must run next.

- `cargo-allow why` now scans only the file at `--path` instead of the entire
  source tree, so explaining a single finding no longer tree-sitter-parses every
  `.rs` file in the repository. The fast path loads the full policy but skips the
  `git ls-files` inventory walk. Safe for `why` (advisory, read-only); `add`
  keeps the full pipeline since it mutates the ledger.

### Changed

- `Selector.line_hint` is no longer propagated from TOML into the runtime
  `Selector`. The field is still accepted in policy TOML for backward
  compatibility but is dropped during deserialization, making it inert
  everywhere downstream (fingerprint, render, validation). It was never read by
  the matching engine; numeric line-distance scoring was retired in favor of
  explicit match-strength tiers. Existing policies with `line_hint = N` continue
  to parse without error; new policies written by `add`/`migrate`/`refresh` no
  longer emit the field.

- `cargo-allow why --plan <PATH>` emits a versioned
  `cargo-allow.add-finding-plan.v1` artifact for an exact currently-new
  finding. The non-mutating, no-overwrite plan binds repository inventory,
  policy bytes, source bytes, structural identity, policy-derived human
  requirements, near-miss reasons, and structured add/check argv; non-new
  findings fail closed without writing a plan.
- `cargo-allow why` now scans only the file at `--path` instead of the entire
  source tree, so explaining a single finding no longer tree-sitter-parses every
  `.rs` file in the repository. The fast path loads the full policy but skips the
  `git ls-files` inventory walk. Safe for `why` (advisory, read-only); `add`
  keeps the full pipeline since it mutates the ledger.

- `cargo-allow list` now includes a "Next steps" block with per-entry commands
  for rows with actionable statuses (stale, expired, review_due,
  location_drift) or broken evidence references, so the operator can go from
  "this entry is stale" to the fix command without looking up syntax.
- `cargo-allow doctor` now suggests a repair path when the policy config is
  invalid, not just when it's missing.

## [0.1.11] - 2026-07-17

### Added

- `cargo-allow diff` now includes copy-paste `why` / `add --update` receipt
  commands for each introduced (unreceipted) finding, so a PR reviewer can
  investigate and receipt a new finding without looking up the command syntax.
- `cargo-allow why` now passes the `--kind` filter to the scanner, matching
  `add`'s behavior, so fewer irrelevant findings are evaluated before the
  requested finding is selected.

- `cargo-allow add --update` writes the new entry directly into the live
  `policy/allow.toml` via load → validate → atomic replace, instead of
  rendering a candidate file. This is the normal receipt path: it preserves
  unrelated entries, validates the complete result, and emits a mutation
  receipt. The existing `--write <PATH>` candidate-file behavior is unchanged;
  `--update` and `--write` are mutually exclusive.
- `cargo-allow migrate --update` writes the migrated policy via atomic replace
  instead of failing when the output path already exists. The existing `--out`
  /`--force` candidate-file behavior is unchanged; `--update` and `--force`
  are mutually exclusive.
- `cargo-allow check --mode no-new` now includes a remediation roadmap with
  copy-paste commands for every non-matched status, not just evidence-repair
  queues. The same roadmap that `audit` produces is now surfaced on failed
  `check` output so the operator knows what to run next.
- `scripts/source-candidate-smoke.sh` emits
  `cargo-allow.source-candidate-smoke-receipt.v1` after a path-installed (or
  `CARGO_ALLOW_BIN`) binary completes the brownfield first-hour journey plus
  refresh (location_drift), `diff --base`, prune preview→write, and git policy
  rollback after prune in a temporary consumer repo, with omitted-step /
  preview-apply / malformed-receipt / post-install source-hidden ordinary-scan /
  package-rebuild omit (`MissingAsset`) / wrong-version (`StaleCandidate`) /
  ordinary-scan offline (`NetworkIsolated`) / unexpected-network
  (`NetworkRequired`) / failed-policy-rollback (`RecoveryFailed`) /
  optional-profile-without-assets (`NotProven`) negatives
  (#2403 / #2402 / #2400 / #2398 / #2396 / #2387 / #2373 / #2278; path-install
  still uses the source checkout).
- `scripts/exact-candidate-package-set.sh` emits
  `cargo-allow.exact-candidate-package-set.v1` after packaging the shared
  ten-crate set, assembling a classic Cargo local-registry (`.crate` + index)
  for the lockfile graph with candidate crates injected, offline-installing
  with crates-io source replacement while renaming workspace `crates/` away
  (`source_checkout_denied` / `CheckoutIsolated`), and running
  omit/path/checksum/version/local-registry-omit / candidate-mismatch
  (`CandidateStale`) / missing-metadata (`ManifestMalformed`) /
  source-checkout-denied negatives (#2408 / #2406 / #2384 / #2380 / #2277).
- Hosted `shallow-diff-smoke` CI job and `scripts/shallow-diff-base-smoke.sh`:
  prove `diff --base` fails closed without base history, then succeeds after
  history is available (#2366).
- Copy-paste CI/ops path: expanded `docs/how-to/run-in-ci.md`, troubleshooting
  and rollback guides, and offline workflow-contract tests for the committed
  GitHub Actions examples (#2355).
- Offline published-release first-run command registry
  (`docs/dogfood/fixtures/getting-started/published-command-registry.toml`) with
  `PublishedQuickStartV1` docs contract tests so source-candidate-only commands
  cannot appear as ordinary published quick-start instructions (#2353).

### Changed

- Removed the unused `maybe_line_distance_score` helper and its tests. The
  function was exported but never consulted by the matching engine; numeric
  line-distance scoring was retired in favor of explicit match-strength tiers.

- Branched, executable first-hour journey in `docs/getting-started.md` with
  published-vs-candidate channel selection, install prerequisites, clean vs
  brownfield forks, `init`/`propose` alternatives, policy command map, fixture
  markers, and a checked step inventory shared with
  `first_hour_adoption` tests (#2354).
- `cargo-allow why` explains unreceipted findings at a path/line, with human and
  `cargo-allow.why.v1` JSON output for first-hour diagnosis before `add`.
- `list --location-drift` status shortcut (parity with `--stale` / `--expired` /
  `--review-due`) for reviewing drifted `last_seen` ledger rows.
- CI MSRV job on Rust 1.85: workspace `cargo check` plus fast `cargo-allow`
  binary package proof, so the declared `rust-version = "1.85"` is executed
  rather than documentation-only.
- Pre-publication `scripts/package-candidate-smoke.sh` (SourceCandidateSmoke):
  workspace package verification, no-path-deps check on packed crates, isolated
  install, and first-hour CLI smoke with a receipt (#2256 Stage A). The smoke
  runs in hosted CI on Linux as the `package-smoke` job and uploads a
  `package-candidate-smoke-receipt` artifact for every PR and push to `main`.

### Fixed

- Glob matching budgets recursive steps (`GLOB_MATCH_MAX_STEPS = 10_000`) so
  pathological `*` / `**` patterns fail closed quickly instead of
  exponential-backtracking (#1924).
- Revision path reads reject Windows drive, UNC/device, and rooted host paths
  before separator rewriting, and prove literal pathspec selection against
  colliding metacharacter names (#2321).
- Operator `manage-an-exception` guide now includes `why`, preview/`--write`
  add examples, adoption-route links, and a docs command-parity test (#2251).
- `cargo-allow why` proof guidance is built as structured argv (`proof_plans`)
  and rendered with platform shell quoting (or an explicit non-copyable argv
  listing). Ambiguous outcomes emit `explain` plans for every candidate ID
  (#2335).
- Structured `CargoAllowErrorKind::Usage` errors now exit `2` (same class as
  Clap parse failures); policy/runtime failures remain exit `1` (#2340).
- Filesystem inventory walks now cap recursion depth (64) and collected file
  entries (250_000). Cap hits record skip diagnostics and mark completeness
  partial instead of unbounded stack/memory growth (#1917).
- Source-tree text reads are capped at 8 MiB (`SOURCE_FILE_READ_MAX_BYTES`)
  across scanners, policy loaders, federation, and legacy migrate paths so a
  single oversized file cannot unbounded-memory the scan.
- `list` status selectors (`--status`, `--expired`, `--review-due`, `--stale`,
  `--location-drift`) are mutually exclusive instead of silently ANDing to an
  empty result.
- `list` / `worklist --status` accept every `MatchStatus` value, including
  `location_drift`.
- Inventory reports now disclose whether source coverage is complete, scoped,
  obtained through fallback, or partial across shared report artifacts and
  schemas.

- Include-untracked filesystem fallback now preserves the Git inventory error
  in the returned inventory metadata for diagnostics and receipts.
- Filesystem fallback inventory now prunes only the repository-root `target/`
  directory, preserving legitimate nested source directories named `target`.
- Policy validation failures now retain structured diagnostic details on
  `CargoAllowError`, including stable code, category, severity, entry ID, and
  validation field, while preserving the existing human-readable aggregate.
- Match outcomes now carry deterministic structured candidate allow IDs, and
  worklist JSON exposes those IDs for ambiguous findings without requiring
  consumers to parse human-readable messages.
- `CargoAllowError` and `CargoAllowErrorKind` now expose stable `E000x_*`
  machine-readable codes, with a checked-in registry at
  `docs/error-codes.md`; human-readable messages remain unchanged.
- `allow_match::evaluate_detailed` now exposes per-entry occurrence accounting
  (observed count, configured limit, remaining headroom, and exceeded count) so
  library consumers do not have to reconstruct occurrence-limit state from
  human-readable match messages. The existing `evaluate` API remains unchanged
  and returns the same match outcomes.
- Follow-up to the mutation-receipt envelope (GOAL-0004 PR 5A): `add --glob
  --summary-format json` now carries the same shared `mutation_receipt`
  envelope as the `--path`/`--line` JSON path (`allow_report::
  render_mutation_receipt_json` is now `pub`, so it renders identically from
  both call sites instead of the broad-baseline path omitting it). Fixed
  `allow_core::allow_entry_content_fingerprint` to slash-normalize `glob` and
  `selector.glob` the same way `path` already was, so semantically identical
  scopes authored on Windows and Unix (`docs\**` vs `docs/**`) fingerprint
  identically instead of spuriously differing by platform.

## [0.1.10] - 2026-07-08

### Added

- Shared mutation-receipt envelope for `add` (GOAL-0004 PR 5A, first slice of
  #1475's provenance work): `allow_report::MutationReceipt` defines the
  provenance envelope from CARGO-ALLOW-SPEC-0008 ("Mutation Receipt Envelope")
  once — `operation`, `tool_version`, `repo_root`, `config_source`,
  `ledger_ids`, `changed_allow_ids`, `before_fingerprints`,
  `after_fingerprints`, `result`, `next_commands`, `claim_boundary` — and wires
  it into `cargo-allow add --summary-format json` as a new `mutation_receipt`
  object. `after_fingerprints` uses a new
  `allow_core::allow_entry_content_fingerprint` SHA-256 digest over an
  explicitly versioned, length-prefixed canonical entry serialization; it is
  provenance evidence rather than an identity or matching key.
  `propose`, `refresh`, `prune`, and `migrate` adopt the same envelope in later
  slices instead of reinventing per-command provenance shapes. Provenance
  metadata only; does not change `add`'s write behavior.
- Runtime enforcement for required change notes on weakening edits (GOAL-0004
  PR 4, #1475, #2075): `cargo-allow diff --require-change-note` fails the diff
  when a policy edit with severity `Fail` or `Review` lacks a matching revision
  note in `.allow/revisions/` (`--revisions-dir` overrides the default).
  Matching is structural on `(allow_id, change_kind)` using the canonical
  `policy_change_kind` vocabulary; improvements are exempt. Does not yet pin
  repeatable-weakening kinds to a specific transition (tracked as a follow-up)
  or publish a `.allow/revisions/` JSON schema.
- Movement classification in diff (GOAL-0004 PR 2, #1471): every diff row carries
  orthogonal `movement`, `posture_delta`, and `changed_in_diff` plus optional
  `subject`, `allow_id`, `ledger_id`, and `lane`; JSON diff adds dual summary blocks
  (`movement.introduced/retained/removed`, `posture_delta.improved/worsened/
  review_required/unchanged`) while retaining legacy `change`, `summary`, and
  net-posture fields. Projects through human diff, Markdown PR summary, JSON,
  receipts, and worklist routing vocabulary. Does not enforce revision notes (PR 4).
- Canonical ledger state model in `allow-core` (GOAL-0004 PR 1): add
  `PresenceMovement`, `PostureDelta`, `LedgerPosture`, and `NetPosture` with
  round-trip parsing, receipt/artifact field names, and PR-summary projection
  helpers; centralize diff string mappings in `allow-report::ledger_posture`.
  Characterization tests lock existing finding-change and net-posture strings.
  No user-visible semantic change; PR 2 movement classification unblocked.
- Register GOAL-0004 core exception ledger coherence and change control
  (CARGO-ALLOW-CLOSEOUT-0019): add CARGO-ALLOW-PROP-0008, CARGO-ALLOW-SPEC-0008,
  CARGO-ALLOW-PLAN-0009, and active goal with PR 1–9 work items; reconcile
  #1473 (per-lane posture + F0–F3 federation landed); confirm #1472/#1474
  already closed. Queue `ledger-coherence-pr1-canonical-state-model` ready; defer
  ripr and full import mode from GOAL-0003. No behavior change or release
  authorization.
- Close GOAL-0003 portable governance substrate (extend CARGO-ALLOW-CLOSEOUT-0017):
  archive full execution history to
  `.allow/goals/archive/CARGO-ALLOW-GOAL-0003-portable-governance-substrate.toml`;
  slim `.allow/goals/active.toml` to done stub with blocked ripr and full import
  follow-ups only; no ready work items. No release authorization.
- Import graph dogfood receipt: `docs/dogfood/cargo-allow-import-graph.md` with
  committed spec-system audit JSON for main-repo `import_graph` and I2 Kiro,
  Spec Kit, and xtask characterization fixtures. Marks
  `portable-governance-import-dogfood` done. Does not claim external ripr
  migration, full import mode, or release readiness.
- Register GOAL-0003 partial progress closeout (CARGO-ALLOW-CLOSEOUT-0017):
  record C2–C4, F0–F3, and I1–I2 done after #1765; queue
  `portable-governance-import-dogfood` ready; keep ripr and full import mode
  blocked. No release authorization.
- Register I2 xtask command registry adapter closeout (CARGO-ALLOW-CLOSEOUT-0016):
  mark `portable-governance-i2-xtask-adapter` done after #1765; close I2 import
  adapter lane; queue `portable-governance-ripr-preflight-r0` blocked. No release
  authorization.
- xtask command registry import adapter I2 slice (#1466): read-only discovery for `xtask/`
  registry TOML files (`commands.toml`, `command-registry.toml`, `registry.toml`) via
  `allow-policy::import_roots::adapters::xtask`. Normalizes `[[commands]]` entries into
  nodes, edges, provenance, confidence, and diagnostics on the I1 `import_graph` in
  spec-system doctor/audit/worklist without Rust dispatch parsing. Fixture-backed tests under
  `tests/fixtures/import/xtask`. Does not implement full import mode or claim release readiness.
- Kiro and Spec Kit import adapters I2 slice (#1466): read-only discovery for `.kiro/`
  (`requirements.md`|`bugfix.md`, `design.md`, `tasks.md`) and `.specify/` (constitution,
  `spec.md`, `plan.md`, `tasks.md`, templates) via `allow-policy::import_roots::adapters::kiro`
  and `::spec_kit`. Normalizes nodes, edges, provenance, confidence, and diagnostics on the
  I1 `import_graph` in spec-system doctor/audit/worklist. Fixture-backed tests under
  `tests/fixtures/import/kiro` and `tests/fixtures/import/spec-kit`. Does not implement
  xtask registry adapter, full import mode, or claim release readiness.
- Generic import adapters I2 slice (#1466): read-only discovery for `.spec/`, `.rails/`,
  and auto-detected `.<repo>-spec/` roots via `allow-policy::import_roots::adapters::generic`.
  Recursive markdown scan with front-matter `id` and `linked_*` normalization extends the
  I1 `import_graph` in spec-system doctor/audit/worklist. Fixture-backed tests under
  `tests/fixtures/import/`. Does not implement Kiro or Spec Kit adapters (follow-up PR),
  xtask registry adapter, or claim full import mode.
- Register I1 generic import-root model closeout (CARGO-ALLOW-CLOSEOUT-0013):
  mark `portable-governance-i1-import` done after #1761; queue
  `portable-governance-i2-import-adapters` ready. No release authorization.
- Generic import-root model I1 (#1466): parse optional `[import_roots]` config on the
  spec-system profile with owned/imported/legacy/generated node roles; read-only
  discovery stub normalizes graph nodes, edges, provenance, confidence, and
  diagnostics in `allow-policy::import_roots`. Spec-system doctor, audit, and
  worklist emit `import_graph` summaries and route `broken_import` work items for
  broken edges and config collisions. Does not implement Kiro/Spec Kit/.rails
  adapters (I2+) or claim full import mode.
- Multi-ledger federation F3 mirror divergence (#1473): compare canonical and mirror
  policy ledgers during active `[[drain_windows]]` in `.allow/config.toml`; emit
  visible `mirror_divergence`, `mirror_stale`, and blocking `drain_expired`
  records in doctor, check receipts (`federation.divergence_summary`), and
  worklist (`mirror_divergence` item kind). Optional `check --deny mirror_divergence`
  escalates advisory mirror drift to failure. Does not claim import adapters,
  release readiness, or external ripr parity.
- Register F2 federation check evaluation closeout (CARGO-ALLOW-CLOSEOUT-0011):
  mark `portable-governance-f2-federation` done after #1758; queue
  `portable-governance-f3-federation` ready; reconcile
  `plans/migration-parity/gap-inventory.md`. No release authorization.
- Multi-ledger federation F2 evaluation (#1473): evaluate canonical ledgers from
  `.allow/config.toml` with deterministic precedence on the source-exception
  `check` path; annotate findings, work items, and receipts with
  `ledger_id`, `ledger_path`, `lane`, `mode`, and `role` provenance plus receipt
  `federation.ledger_contributors` and `precedence_applied`. Spec-system work
  items inherit doc-artifacts ledger provenance when federation config is present.
  Does not claim mirror divergence enforcement (F3) or release readiness.
- Register F1 federation config parse closeout (CARGO-ALLOW-CLOSEOUT-0010):
  mark `portable-governance-f1-federation` done after #1756; queue
  `portable-governance-f2-federation` ready; reconcile
  `plans/migration-parity/gap-inventory.md`. No release authorization.
- Multi-ledger federation F1 config (#1473): parse `[[ledgers]]` entries from
  `.allow/config.toml` with `id`, `path`, `dialect`, `role`, optional `lanes`,
  `mode`, `priority`, and mirror `mirrors` targets. Validate duplicate ledger
  IDs/paths, canonical lane collisions, mirror targets, and foreign dialect
  posture (`dialect_conflict` vs informational `dialect_skipped`). Default
  `doctor` and spec-system doctor report configured ledgers and validation
  diagnostics; multi-ledger check evaluation remains deferred to F2.
- Multi-ledger federation F0 design (#1473): register CARGO-ALLOW-PROP-0007,
  CARGO-ALLOW-SPEC-0007, CARGO-ALLOW-ADR-0001, and CARGO-ALLOW-PLAN-0007.
  Define canonical/mirror/imported ledger roles, lane ownership, deterministic
  precedence, duplicate and dialect handling, drain windows, divergence
  reporting, and receipt provenance with no silent merging. Mark
  `portable-governance-f0-federation` done (design-only); queue F1 runtime
  implementation blocked pending F0 merge. No release authorization.
- Dogfood migrate profile state to `.allow/` (CARGO-ALLOW-PLAN-0004 C4): move
  spec-system profile config, artifact ledger, active goal, archive, and imports
  stub from legacy `policy/` profile paths and `.codex/goals/` to owned
  `.allow/` layout. Register CARGO-ALLOW-CLOSEOUT-0008; mark
  `portable-governance-c4` done; queue federation (#1473) blocked. Legacy C2
  resolution fallback and fixture tests remain. `policy/allow.toml` stays the
  source-exception ledger. No release authorization.
- Register C3 init writes closeout (CARGO-ALLOW-CLOSEOUT-0007): mark
  `portable-governance-c3` done after #1750; queue `portable-governance-c4`
  (dogfood migrate profile state to `.allow/`) ready; reconcile
  `gap-inventory.md` and governance manifests. No release authorization.
- Register C2 profile resolution closeout (CARGO-ALLOW-CLOSEOUT-0006): mark
  `portable-governance-c2` done after #1748; queue `portable-governance-c3`
  (`init` writes spec-system state to `.allow/`) ready; reconcile
  `gap-inventory.md` and governance manifests. No release authorization.
- Resolve spec-system profile config with `.allow/` precedence and legacy
  `policy/<profile>.toml` fallback (CARGO-ALLOW-PLAN-0004 C2): explicit
  `--config`, then `.allow/profiles/<profile>.toml`, `.allow/config.toml`,
  legacy `policy/spec-system.toml`, then built-in defaults. Doctor and
  spec-system receipts report `config_provenance`; owned plus legacy configs
  emit an advisory conflict diagnostic instead of silent merge (#1748).
- `cargo-allow init --profile spec-system` bootstraps owned profile state under
  `.allow/` (CARGO-ALLOW-PLAN-0004 C3): `.allow/profiles/spec-system.toml`,
  `.allow/artifacts/doc-artifacts.toml`, `.allow/goals/active.toml`,
  `.allow/goals/archive/`, and `.allow/imports/README.md`. Legacy `policy/`
  profile paths remain supported when `.allow/` is absent; `policy/allow.toml`
  stays the source-exception ledger. Spec-system doctor reports
  `allow_profiles` provenance and an `allow_imports` readiness check for the
  owned layout.
- Register portable governance transition closeout (CARGO-ALLOW-CLOSEOUT-0005):
  archive CARGO-ALLOW-GOAL-0002 migration/adoption-substrate/import-parity
  execution; close #1474 advisory counters + `--deny` escalation after #1472
  `occurrence_headroom`; register CARGO-ALLOW-GOAL-0003 with
  `portable-governance-c2` (`.allow` profile resolution) `ready` and federation
  (#1473), external ripr, and full import mode (#1466) blocked. Reconcile
  `gap-inventory.md` and `0.1.10-readiness.md`; no release authorization.
- Consolidate receipt advisory field names and `check --deny` parsing behind
  canonical `AdvisoryClass` registry in `allow-report` (#1746).
- Emit `occurrence_headroom` advisory counts when a matched allow entry has
  `occurrence_limit` above its current matched count; route worklist items with
  limit-reduction guidance; include receipt/report trend and repair-queue
  routing; and support `check --deny occurrence_headroom` (#1472).
- Document structural identity scanner limitations and claim boundary (D8):
  extend [docs/identity.md](docs/identity.md) with source-syntax claim boundary,
  per-field stable/hint/ambiguous/missing table from D2–D7 characterization,
  and fixture-backed examples referencing
  `tests/fixtures/structural-identity/`; link from
  [gap-inventory.md](plans/structural-identity/gap-inventory.md) and
  [implementation-plan.md](plans/structural-identity/implementation-plan.md);
  cross-link [docs/claim-boundaries.md](docs/claim-boundaries.md). Mark
  `post-import-d8` done in `.codex/goals/active.toml`; extend
  CARGO-ALLOW-CLOSEOUT-0003. Closes structural identity execution lane D1–D8;
  no scanner, matcher, or diff behavior changes.
- Register post-import next execution lane in `.codex/goals/active.toml`: D8
  scanner limitation docs and #1472 `occurrence_headroom` outcomes/worklist
  marked `ready`; #1473 P2 multi-ledger federation and #1466 full import mode
  marked `blocked`. Refresh `docs/release/0.1.10-readiness.md` with import-parity
  closeout, advisory ratcheting progress, and D1–D7 identity characterization;
  release cut remains deferred.
- Register import-parity execution lane closeout (CARGO-ALLOW-CLOSEOUT-0004)
  after #1713–#1718 characterization slices and ripr-style in-repo dogfood
  receipt (#1741). Umbrella #1466 remains open for full import mode and external
  adoption; gap-inventory #1466 row stays `partial`.
- Add import-parity ripr-style multi-family adoption dogfood receipt (#1718):
  `docs/dogfood/cargo-allow-ripr-style-adoption.md` records per-lane compat
  checks, `--repo-policy` batch migrate, per-lane canonical checks, worklist
  routing, and closeout for a panic+unsafe+lint legacy batch resembling ripr
  adoption concerns. Committed fixtures under `docs/dogfood/fixtures/ripr-style/`
  and receipts under `docs/dogfood/receipts/cargo-allow-ripr-style-adoption.*`.
  Does not migrate the external `ripr` repository.
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
  advisory fields including `occurrence_headroom`.
- Add receipt-visible `advisory` counters to `cargo-allow.receipt.v1` check artifacts so
  CI and ratcheting workflows can read review-oriented status totals (`review_items`,
  `review_due`, `stale`, `baseline_debt`, optional policy/evidence-health counts) without
  parsing human reports. Markdown check reports include a matching `## Advisory counts`
  section. Exit status is unchanged without `--deny`; per-class `--deny` escalation is available
  for receipt `advisory` fields.
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
