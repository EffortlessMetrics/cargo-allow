# Roadmap

The roadmap is intentionally PR-sized. Each PR should include purpose,
non-goals, validation, claim boundary, and rollback path.

External dogfood is validation evidence, not the default next local task. Do
not switch into another repository from this roadmap unless that dogfood run is
explicitly selected for the current PR. Keep cargo-allow repo work focused on
in-repo product, scanner, policy, report, and documentation slices.

## Phase 1: Stabilize The MVP

Goal: make the imported MVP boring, tested, documented, and safe to evolve.

Completed:

- Import the repo-ready MVP source tree.
- Add CI gates and the generated no-new baseline.
- Harden path normalization, inventory traversal, indexing heuristics, and
  snippet-hash matching regressions.
- Define the product lane and claim boundaries in docs.
- Stabilize current JSON reports and receipt schemas.
- Enumerate supported scanner limitation values in report and receipt schemas.
- Share claim-boundary and scanner-limitation flags between report, receipt,
  and worklist JSON producers to prevent contract drift.
- Publish report, receipt, and worklist schema IDs/versions from `allow-report`
  instead of duplicating worklist literals in the CLI.
- Assert every shared claim-boundary and scanner-limitation flag is documented
  across report, receipt, and worklist schemas.
- Enumerate supported claim-boundary values in report, receipt, and worklist
  schemas.
- Thread source-tree root and inventory file-count facts into reports and
  receipts without requiring Cargo project metadata or a successful build.
- Replace handwritten TOML parsing with typed serde/toml policy loading.
- Strengthen lifecycle and required-field validation.
- Replace manual CLI parsing with clap.
- Document the source-tree-only product boundary.
- Remove remaining Cargo-project discovery assumptions from root and inventory
  discovery.
- Harden non-Rust classification.
- Support generated-code and ignored-surface policy.
- Improve human and Markdown non-Rust audit output.

External dogfood:

- Non-Rust governance against an existing bespoke file-policy xtask.

## Phase 2: Replace Temporary Foundations

Goal: make the product surface stable enough for real users.

Planned PRs:

## Phase 3: Make Non-Rust Governance Useful

Goal: ship the first low-parser-risk lane that real repositories can adopt.

Completed:

- Add `--compat --kind non-rust` for the shiplog-style legacy
  `policy/non-rust-allowlist.toml` shape and validate it side-by-side against a
  blocking file-policy xtask fixture.
- Add `--compat --kind generated` for the shiplog-style legacy
  `policy/generated-allowlist.toml` shape and validate it side-by-side against a
  generated-file xtask fixture.
- Add `--compat --kind executable` for the shiplog-style legacy
  `policy/executable-allowlist.toml` shape and validate it side-by-side against
  an executable-bit xtask fixture.
- Add `--compat --kind workflow` for the shiplog-style legacy
  `policy/workflow-allowlist.toml` shape and validate it side-by-side against a
  workflow xtask fixture.
- Add `--compat --kind dependency-surface` for the shiplog-style legacy
  `policy/dependency-surface-allowlist.toml` shape and validate it side-by-side
  against a dependency-surface xtask fixture.
- Add `--compat --kind process` for the shiplog-style legacy
  `policy/process-allowlist.toml` shape and validate it side-by-side against a
  process-policy xtask fixture.
- Add `--compat --kind network` for the shiplog-style legacy
  `policy/network-allowlist.toml` shape and validate it side-by-side against a
  network-policy xtask fixture.
- Add `cargo-allow migrate --repo-policy policy/ --out policy/allow.toml` for
  combining supported shiplog-style legacy files into one canonical policy.
- Let canonical checks collect migrated generated, executable, workflow,
  dependency-surface, process, and network companion findings without requiring
  `--compat`.
- Dogfood the migrated canonical policy against real legacy non-Rust,
  generated, executable, workflow, dependency-surface, process, and network
  policy fixtures.

External dogfood:

- Prepare a target-repo replacement PR for a file-policy lane while keeping
  panic, unsafe, and lint source lanes out of scope. This belongs in the target
  repository, not as an implicit cargo-allow repo task.

## Phase 4: Build Structural Identity

Goal: move from line-oriented scanning toward durable source identity.

Completed:

- Define `StructuralIdentity` v1 as a stable contract.
- Integrate a source-syntax Rust parser foundation that parses `.rs` files
  directly without requiring Cargo metadata, compilation, build scripts, or proc
  macro expansion.
- Implement source-syntax container identity for modules, free functions,
  inherent impl methods, and trait impl methods.
- Populate optional source-derived package context from tracked `Cargo.toml`
  `[package].name` fields without invoking Cargo metadata.
- Replace ad hoc matching with a scored structural matcher that requires
  kind/path compatibility plus selector field agreement and fails closed on
  ambiguity.
- Add selector precision scoring for diff-time policy weakening detection.

Planned PRs:

## Phase 5: Panic-Family Lane

Goal: replace bespoke no-panic allowlist xtasks with structural, reviewable
receipts.

Completed:

- Add count-limited migration for generated shiplog-style legacy
  `policy/no-panic-baseline.toml` into temporary `baseline_debt` entries.
- Scan panic-family method calls and macros from source syntax.
- Scan indexing and slicing expressions from source syntax.
- Apply shared no-new and strict matching behavior to panic-family findings.
- Add `--compat --kind panic` for generated shiplog-style legacy
  `policy/no-panic-baseline.toml`, preserving legacy `count` as
  `occurrence_limit`.
- Add `--compat --kind no-panic-allowlist` for legacy
  `policy/no-panic-allowlist.toml`, mapping `explanation` to `reason`,
  `selector.kind` to `selector.ast_kind`, and `last_seen` to hints only.

External dogfood:

- Side-by-side dogfood against a strict repo.

## Phase 6: Unsafe Lane

Goal: make every retained unsafe site carry reason, evidence, ownership, scope,
and lifecycle.

Completed:

- Scan unsafe functions, impls, traits, extern blocks, unsafe blocks, and unsafe
  attributes from source syntax, including multiple unsafe constructs on the
  same line.
- Detect nearby visible `SAFETY:` comments as source-text metadata for unsafe
  findings and enforce `requirements.unsafe.safety_comment_required` in the
  matcher.
- Add `--compat --kind unsafe` for legacy `policy/unsafe-allowlist.toml`
  files, mapping retained unsafe entries to source-syntax `unsafe` receipts and
  keeping missing legacy evidence as temporary baseline debt.
- Validate `unsafe-review:` evidence references as local source-tree files
  without executing unsafe-review.

External dogfood:

- Side-by-side dogfood against a repo with existing unsafe policy.

## Phase 7: Lint Suppression Lane

Goal: make source suppressions link back to the ledger.

Completed:

- Scan outer and inner `allow` and `expect` attributes from source syntax,
  including visible lint names, source text, and attribute column hints.
- Extract visible `policy:<allow-id>` references from lint attribute source text
  and fail closed when a matched lint suppression references a different allow
  entry.
- Enforce `allow_bare_allow_attributes = false` by failing matched bare
  `#[allow]` suppressions instead of treating the receipt as approval.
- Require visible `policy:<allow-id>` references for matched lint suppressions
  when `lint_policy_id_required = true`.
- Add `--compat --kind lint-exception` for legacy `policy/clippy-exceptions.toml`
  files, mapping retained suppression entries to source-syntax
  `lint_exception` receipts.

External dogfood:

- Dogfood lint-exception compat against a repo with an existing Clippy
  exceptions policy.

## Phase 8: PR Diff As Flagship

Goal: make PR review the primary source-tree exception review experience.

Completed:

- Detect policy weakening in `cargo-allow diff` for current `policy/allow.toml`
  versus `--base`, including scope broadening, selector precision loss, expiry
  extension, evidence removal, metadata removal, occurrence-limit loosening, and
  added baseline debt.
- Report selector precision increases as policy improvements so narrowed or
  structurally strengthened receipts are visible in PR posture.
- Report evidence additions as policy improvements so proof-link strengthening
  is visible in PR posture.
- Report occurrence-limit additions and reductions as policy improvements so
  counted-baseline tightening is visible in PR posture.
- Report lifecycle additions and earlier review/expiry dates as policy
  improvements so lifecycle tightening is visible in PR posture.
- Report owner, reason, and classification additions as policy improvements so
  required-metadata restoration is visible in PR posture.
- Report scope narrowing as a policy improvement so glob-to-path and broad-glob
  cleanup is visible in PR posture.
- Compare base and head source findings in `cargo-allow diff` using durable
  finding keys, so reviewers can see new and removed syntax-visible exception
  posture independent of line movement.
- Emit Markdown PR summaries with net posture, reviewer action, current
  no-new failures, source finding changes, and policy weakening counts.
- Emit structured JSON diff posture data with net posture, finding changes, and
  policy changes for automated PR consumers.
- Report removed allow entries as policy improvements so stale-ledger cleanup
  is visible in PR summaries and structured JSON.
- Add GitHub Actions examples for PR posture diff and mainline no-new check
  workflows.

## Phase 9: Human UX Commands

Goal: make the tool pleasant and self-explanatory.

Completed:

- Improve `explain` so it reports the current live match status, matched
  finding count, occurrence-limit overruns, stale entries, lifecycle/evidence
  outcomes, and the command claim boundary.
- Include suggested actions and proof commands in `explain` output for entries
  that need attention.
- Include suggested actions and proof commands in `explain` output for matched
  `baseline_debt` entries so generated debt is not hidden by a matched status.
- Show scanner-provided `source_package` context in `explain` current findings
  without treating it as Cargo metadata.
- Improve `list` with current status, match counts, owner/kind/lifecycle
  filters, and baseline-debt filtering.
- Show scanner-provided source package context in `list` rows without treating
  it as Cargo metadata.
- Filter `list` output by policy classification for reviewed-exception and
  baseline-debt ledger slices.
- Implement dry-run-first stale pruning.
- Support explicit `prune --stale --write` removal of stale entries while
  keeping dry-run as the default.
- Make baseline proposal output safer by refusing accidental overwrites and
  emitting a proposal summary.
- Add allow entries from current findings with structural selectors and
  fail-closed nearest-finding selection.

Planned work:


## Phase 10: Migration And External Dogfood

Goal: replace bespoke xtask lanes.

Completed:

- Canonical `allow.toml` writer.
- Multi-file legacy config compatibility.

External dogfood:

- Dogfood all compat lanes in one repo.
- Replace non-Rust, panic, lint, and unsafe xtasks incrementally.

## Phase 11: Evidence And Integrations

Goal: connect source exceptions to proof artifacts.

Completed:

- Parse evidence references and validate local file evidence prefixes when a
  policy is loaded from a source tree.
- Explain evidence reference status from `cargo-allow explain`, including
  present, missing, invalid, traceability, and unstructured references.
- Add examples for ripr, unsafe-review, and coverage evidence references while
  keeping the no-execution claim boundary explicit.

Planned work:

## Phase 12: Agent-Native Worklists

Goal: make cargo-allow a safe work router for humans and agents.

Completed:

- Emit an initial `cargo-allow worklist --format json` that converts non-matched
  no-new outcomes into risk/difficulty-scored work items with suggested actions
  and proof commands.
- Emit `broken_evidence_link` work items for missing or invalid local evidence
  references.
- Document bounded agent prompt patterns for `cargo-allow worklist`.
- Align worklist JSON claim-boundary flags with the report and receipt
  source-tree scanner limitations.
- Emit explicit worklist JSON `scanner_limitations` alongside
  `claim_boundary`, matching report and receipt artifacts.
- Enumerate supported worklist scanner limitation values in the JSON schema.
- Refine worklist risk and difficulty heuristics for source-tree policy
  exception families without using Cargo package metadata.
- Keep worklist proof commands as executable cargo-allow commands and avoid
  vague external validation placeholders.
- Surface explicit source-tree package context in work items when a scanner
  provides it, without inferring Cargo metadata or build facts.
- Surface explicit governed exception kind and family in work items so agents
  can route work without parsing messages.
- Summarize worklist difficulty counts and include source-tree inventory
  context, including `files_scanned`, in worklist JSON and human output.
- Add a versioned JSON schema for `cargo-allow.worklist.v1`.
- Emit advisory `broad_scope` work items for matched allow entries that still
  use wildcard source-tree scopes.
- Emit advisory `baseline_debt` work items for matched generated baseline
  entries so temporary debt stays visible after no-new passes.
- Show suggested actions in human worklist output so maintainers can triage
  without switching to JSON.
- Show multiple proof commands in human worklist output for local follow-up and
  refreshed worklist routing.
- Report human worklist truncation and point to JSON for the full queue.
- Filter worklist output by risk and difficulty so humans and agents can choose
  a bounded queue without parsing JSON.
- Record applied worklist filters in human and JSON output so filtered artifacts
  cannot be confused with the full ledger queue.
- Sort worklist output by risk and difficulty so the default queue surfaces
  high-priority work without requiring filters.
- Assign worklist item IDs after filtering and sorting so artifact-local queue
  handles match the presented order.
- Include owner, classification, and reason on policy-backed worklist items so
  humans and agents can route work before running `explain`.
- Include lifecycle dates and evidence counts on policy-backed worklist items so
  expiry pressure and weak evidence are visible in the queue.
- Filter worklist output by policy owner and classification so humans and
  agents can take bounded ownership or debt-class slices.
- Filter worklist output by queue item kind so agents can take targeted stale,
  broad-scope, baseline-debt, or broken-evidence slices.

Planned work:

## Phase 13: Audit Reports

Goal: make output useful beyond developers.

Completed:

- Add a Markdown audit summary and review queue for non-matched source-tree
  outcomes.
- Include review-due and invalid-selector statuses in human and Markdown report
  count tables.
- Add optional JSON report trend metrics for review items, lifecycle pressure,
  evidence gaps, and baseline debt.
- Count policy-level `baseline_debt` entries in audit/report trend summaries so
  matched generated debt stays visible after no-new passes.
- Expose optional source-derived package context as `source_package` in JSON
  report findings and SARIF result properties, without treating it as Cargo
  metadata.
- Emit SARIF output for non-matched source-tree outcomes in audit/check style
  reports.
- Emit static HTML reports for source-tree audit/check output.

Planned work:

## Phase 14: Public Product Polish

Goal: make cargo-allow installable, understandable, and publishable.

Completed:

- Public README.
- Examples.
- crates.io metadata.
- crates.io-compatible version requirements for internal workspace
  dependencies.
- 0.1.0 release runbook.
- 0.1.0 dry-run.
- 0.1.0 publish.

## Milestone Claims

`0.1.0` should claim:

```text
cargo-allow inventories syntax-visible source exceptions and checks them against
a policy ledger.
```

`0.2.0` should claim:

```text
cargo-allow can replace bespoke AST/TOML allowlist xtasks.
```

`0.3.0` should claim:

```text
cargo-allow shows how a PR changes source exception posture.
```

`0.4.0` should claim:

```text
cargo-allow connects source exceptions to proof artifacts.
```

`1.0` should claim:

```text
cargo-allow is the stable source exception ledger for source trees.
```
