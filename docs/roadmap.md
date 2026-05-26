# Roadmap

The roadmap is intentionally PR-sized. Each PR should include purpose,
non-goals, validation, claim boundary, and rollback path.

## Phase 1: Stabilize The MVP

Goal: make the imported MVP boring, tested, documented, and safe to evolve.

Completed:

- Import the repo-ready MVP workspace.
- Add CI gates and the generated no-new baseline.
- Harden path normalization, inventory traversal, indexing heuristics, and
  snippet-hash matching regressions.
- Define the product lane and claim boundaries in docs.
- Stabilize current JSON reports and receipt schemas.
- Replace handwritten TOML parsing with typed serde/toml policy loading.
- Strengthen lifecycle and required-field validation.
- Replace manual CLI parsing with clap.
- Use cargo_metadata for workspace discovery.
- Harden non-Rust classification.
- Support generated-code and ignored-surface policy.
- Improve human and Markdown non-Rust audit output.

Next:

- Dogfood non-Rust governance against an existing bespoke file-policy xtask.

## Phase 2: Replace Temporary Foundations

Goal: make the product surface stable enough for real users.

Planned PRs:

- Thread Cargo metadata package/source-root facts into scanners and reports.

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

Next:

- Harden documented replacement gaps before removing any existing xtask:
  network policy.

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

Goal: make PR review the primary cargo-allow experience.

Planned work:

- Scan base and head.
- Detect policy weakening.
- Emit Markdown PR summaries.
- Add GitHub Actions examples.

## Phase 9: Human UX Commands

Goal: make the tool pleasant and self-explanatory.

Planned work:

- Improve `explain`.
- Improve `list`.
- Implement dry-run-first stale pruning.
- Add allow entries from findings.
- Make baseline proposal production-quality.

## Phase 10: Migration And Dogfood

Goal: replace bespoke xtask lanes.

Planned work:

- Canonical `allow.toml` writer.
- Multi-file legacy config compatibility.
- Dogfood all compat lanes in one repo.
- Replace non-Rust, panic, lint, and unsafe xtasks incrementally.

## Phase 11: Evidence And Integrations

Goal: connect source exceptions to proof artifacts.

Planned work:

- Parse evidence references.
- Validate local evidence files.
- Explain broken evidence.
- Add examples for ripr, unsafe-review, and coverage evidence.

## Phase 12: Agent-Native Worklists

Goal: make cargo-allow a safe work router for humans and agents.

Planned work:

- Emit `cargo allow worklist --format json`.
- Add risk and difficulty heuristics.
- Add suggested proof commands.
- Document agent prompt patterns.

## Phase 13: Audit Reports

Goal: make output useful beyond developers.

Planned work:

- Markdown audit report.
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
cargo-allow is the stable source exception ledger for Rust workspaces.
```
