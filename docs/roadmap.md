# Roadmap

The roadmap is intentionally PR-sized. Each PR should include purpose,
non-goals, validation, claim boundary, and rollback path.

## Phase 1: Stabilize The MVP

Goal: make the imported MVP boring, tested, documented, and safe to evolve.

Completed:

- Import the repo-ready MVP source tree.
- Add CI gates and the generated no-new baseline.
- Harden path normalization, inventory traversal, indexing heuristics, and
  snippet-hash matching regressions.
- Define the product lane and claim boundaries in docs.
- Stabilize current JSON reports and receipt schemas.
- Replace handwritten TOML parsing with typed serde/toml policy loading.
- Strengthen lifecycle and required-field validation.
- Replace manual CLI parsing with clap.
- Document the source-tree-only product boundary.
- Remove remaining Cargo-project discovery assumptions from root and inventory
  discovery.
- Harden non-Rust classification.
- Support generated-code and ignored-surface policy.
- Improve human and Markdown non-Rust audit output.

Next:

- Dogfood non-Rust governance against an existing bespoke file-policy xtask.

## Phase 2: Replace Temporary Foundations

Goal: make the product surface stable enough for real users.

Planned PRs:

- Thread source-tree root and inventory facts into scanners and reports without
  requiring Cargo project metadata or a successful build.

## Phase 3: Make Non-Rust Governance Useful

Goal: ship the first low-parser-risk lane that real repositories can adopt.

Completed:

- Add `--compat --kind non-rust` for shiplog-style
  `policy/non-rust-allowlist.toml` and prove it side-by-side against shiplog's
  blocking file-policy xtask.
- Add `--compat --kind generated` for shiplog-style
  `policy/generated-allowlist.toml` and prove it side-by-side against shiplog's
  generated-file xtask.
- Add `--compat --kind executable` for shiplog-style
  `policy/executable-allowlist.toml` and prove it side-by-side against
  shiplog's executable-bit xtask.
- Add `--compat --kind workflow` for shiplog-style
  `policy/workflow-allowlist.toml` and prove it side-by-side against shiplog's
  workflow xtask.
- Add `--compat --kind dependency-surface` for shiplog-style
  `policy/dependency-surface-allowlist.toml` and prove it side-by-side against
  shiplog's dependency-surface xtask.
- Add `--compat --kind process` for shiplog-style
  `policy/process-allowlist.toml` and prove it side-by-side against shiplog's
  process-policy xtask.
- Add `--compat --kind network` for shiplog-style
  `policy/network-allowlist.toml` and prove it side-by-side against shiplog's
  network-policy xtask.
- Add `cargo-allow migrate --repo-policy policy/ --out policy/allow.toml` for
  combining supported shiplog-style legacy files into one canonical policy.
- Let canonical checks collect migrated generated, executable, workflow,
  dependency-surface, process, and network companion findings without requiring
  `--compat`.
- Dogfood the migrated canonical policy against shiplog's current non-Rust,
  generated, executable, workflow, dependency-surface, process, and network
  policy surfaces.

Next:

- Prepare the first shiplog replacement PR for a file-policy lane while keeping
  panic, unsafe, and lint source lanes out of scope.

## Phase 4: Build Structural Identity

Goal: move from line-oriented scanning toward durable source identity.

Planned PRs:

- Define `StructuralIdentity` v1 as a stable contract.
- Integrate a lossless Rust syntax parser foundation.
- Implement container identity.
- Replace ad hoc matching with a scored structural matcher.
- Add selector precision scoring.

## Phase 5: Panic-Family Lane

Goal: replace bespoke no-panic allowlist xtasks with structural, reviewable
receipts.

Completed:

- Add count-limited migration for generated shiplog-style
  `policy/no-panic-baseline.toml` into temporary `baseline_debt` entries.

Planned work:

- Syntax scanner for method calls and macros.
- Indexing and slicing scanner.
- no-new and strict behavior for panic-family findings.
- Legacy no-panic allowlist adapter.
- Side-by-side dogfood against a strict repo.

## Phase 6: Unsafe Lane

Goal: make every retained unsafe site carry reason, evidence, ownership, scope,
and lifecycle.

Planned work:

- Syntax scanner for unsafe forms.
- Safety-comment and evidence metadata.
- unsafe-review evidence references.
- Legacy unsafe allowlist adapter.
- Side-by-side dogfood against a repo with existing unsafe policy.

## Phase 7: Lint Suppression Lane

Goal: make source suppressions link back to the ledger.

Planned work:

- Scan allow and expect attributes.
- Enforce suppression policy.
- Verify policy ID linkage.
- Add a legacy clippy exceptions adapter.

## Phase 8: PR Diff As Flagship

Goal: make PR review the primary source-tree exception review experience.

Completed:

- Detect policy weakening in `cargo-allow diff` for current `policy/allow.toml`
  versus `--base`, including scope broadening, selector precision loss, expiry
  extension, evidence removal, metadata removal, occurrence-limit loosening, and
  added baseline debt.
- Compare base and head source findings in `cargo-allow diff` using durable
  finding keys, so reviewers can see new and removed syntax-visible exception
  posture independent of line movement.
- Emit Markdown PR summaries with net posture, reviewer action, current
  no-new failures, source finding changes, and policy weakening counts.
- Add GitHub Actions examples for PR posture diff and mainline no-new check
  workflows.

## Phase 9: Human UX Commands

Goal: make the tool pleasant and self-explanatory.

Completed:

- Improve `explain` so it reports the current live match status, matched
  finding count, occurrence-limit overruns, stale entries, lifecycle/evidence
  outcomes, and the command claim boundary.
- Improve `list` with current status, match counts, owner/kind/lifecycle
  filters, and baseline-debt filtering.
- Implement dry-run-first stale pruning.
- Make baseline proposal output safer by refusing accidental overwrites and
  emitting a proposal summary.
- Add allow entries from current findings with structural selectors and
  fail-closed nearest-finding selection.

Planned work:


## Phase 10: Migration And Dogfood

Goal: replace bespoke xtask lanes.

Planned work:

- Canonical `allow.toml` writer.
- Multi-file legacy config compatibility.
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
- Refine worklist risk and difficulty heuristics for source-tree policy
  exception families without using Cargo package metadata.
- Keep worklist proof commands as executable cargo-allow commands and avoid
  vague external validation placeholders.

Planned work:

- Add crate-local validation suggestions only when cargo-allow has explicit
  source-tree package context that does not require Cargo metadata.

## Phase 13: Audit Reports

Goal: make output useful beyond developers.

Completed:

- Add a Markdown audit summary and review queue for non-matched source-tree
  outcomes.
- Include review-due and invalid-selector statuses in human and Markdown report
  count tables.

Planned work:

- HTML audit report when useful.
- SARIF output.
- Exception trend receipt.

## Phase 14: Public Product Polish

Goal: make cargo-allow installable, understandable, and publishable.

Planned work:

- Public README.
- Examples.
- crates.io metadata.
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
