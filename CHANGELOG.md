# Changelog

All notable changes to cargo-allow are documented here.

cargo-allow is a direct source-tree exception ledger for Rust repositories.
Release notes preserve the claim boundary: cargo-allow scans source-tree
inventory without executing repository code.

## [Unreleased]

### Fixed

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
