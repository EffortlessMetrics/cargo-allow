# cargo-allow Docs

These documents define the default source-tree exception ledger and the opt-in
governance profile model around it.

## Start Here

- [Onboarding](onboarding.md): choose the right first path: source exceptions,
  no-new governance, spec-system, CI, cross-repo adoption, or friction filing.
- [Getting started](getting-started.md): first-run tutorial for doctor, audit,
  init/propose, and no-new checks.
- [Glossary](glossary.md): definitions for recurring policy, selector, identity,
  lifecycle, and migration terms.
- [Changelog](../CHANGELOG.md): curated user-facing release ledger.
- [Contributing](../CONTRIBUTING.md): local development, product boundaries,
  and pull request expectations.

## Product documentation

- [cargo-allow getting started](products/cargo-allow/getting-started.md)
- [cargo-allow command reference](products/cargo-allow/command-reference.md)
- [cargo-allow schemas and artifacts](products/cargo-allow/schemas.md)
- [cargo-allow limitations](products/cargo-allow/limitations.md)
- [cargo-allow compatibility](products/cargo-allow/compatibility.md)
- [cargo-allow support and security](products/cargo-allow/support-and-security.md)
- [cargo-allow release notes](products/cargo-allow/release-notes.md)

## Understand The Model

- [Design](design.md): cargo-allow's source-exception governance model.
- [Claim boundaries](claim-boundaries.md): what current and future reports may
  claim.
- [Source exception ledger](source-exception-ledger.md): the policy concepts and
  entry model.
- [Structural identity v1](identity.md): the source-syntax identity contract
  used by matching and diff posture.
- [PR posture](pr-posture.md): reviewer-facing diff posture and net posture
  semantics.
- [Policy weakening](policy-weakening.md): policy edits that broaden, weaken,
  or improve retained source exceptions.
- [Roadmap](roadmap.md): the PR-sized path from source-tree ledger to mature
  product.

## Adopt cargo-allow

- [Adopt no-new-debt](how-to/adopt-no-new-debt.md): move an existing repo toward
  a no-new source-exception posture.
- [Manage an exception](how-to/manage-an-exception.md): follow one finding
  through decision, repair, weakening notes, pruning, and final proof.
- [Run in CI](how-to/run-in-ci.md): add default source-exception checks to CI.
- [Review PR posture](how-to/review-pr-posture.md): use `cargo-allow diff` for
  reviewer-facing posture.
- [Explain an allow entry](how-to/explain-an-allow.md): inspect one retained
  exception receipt.
- [Explain why a finding is unreceipted](how-to/explain-why-a-finding.md):
  inverse of `explain` for a path/line finding.
- [Fix broken evidence](how-to/fix-broken-evidence.md): repair local evidence
  references.
- [Prune stale allows](how-to/prune-stale-allows.md): remove expired or unused
  receipts.
- [Feed agent worklists](how-to/feed-agent-worklists.md): route bounded repair
  work to humans and agents.

## Use Opt-In Profiles

- [Opt-in governance profiles](profiles.md): reusable profile architecture for
  source-tree config, structural validation, artifacts, worklists, doctor/init,
  and advisory/shadow/blocking rollout.
- [Source-of-truth stack](source-of-truth/README.md): opt-in governance graph
  for proposals, specs, ADRs, plans, goals, support tiers, policy ledgers, and
  closeouts.
- [Doc artifact ledger](source-of-truth/doc-artifact-ledger.md): advisory
  registry for governed proposal/spec artifacts in the source-of-truth graph.
- [Source-of-truth templates](templates/proposal.md): starter templates for
  proposals, specs, ADRs, implementation plans, plan items, closeouts, and PR
  bodies.
- [Spec-system profile proposal](proposals/CARGO-ALLOW-PROP-0001-spec-system-profile.md):
  accepted product proposal for the opt-in source-of-truth profile.
- [Spec-system profile spec](specs/CARGO-ALLOW-SPEC-0001-spec-system-profile.md):
  accepted behavior contract for the opt-in source-of-truth profile.
- [Support tiers](status/SUPPORT_TIERS.md): claim-to-proof map for current
  cargo-allow surfaces and the opt-in spec-system profile.
- [Active goal manifest](../.allow/goals/README.md): Codex execution-state
  model for the source-of-truth profile.
- [Spec-system implementation plan](../plans/spec-system/implementation-plan.md):
  PR sequence for implementing the opt-in source-of-truth profile.
- [Adopt the spec-system profile](how-to/adopt-spec-system-profile.md):
  first-hour setup for the opt-in source-of-truth graph profile.
- [Run the spec-system profile in CI](how-to/run-spec-system-in-ci.md):
  advisory/shadow CI artifact guidance for this opt-in profile.

## Move Repos And Migrate Legacy Policy

- [Adopt cargo-allow across repos](how-to/adopt-cargo-allow-across-repos.md):
  migration playbook for default source-exception checks, opt-in profiles, CI
  artifacts, and adoption-friction issues.
- [ripr spec-system adoption handoff](../plans/external-dogfood/ripr-spec-system-adoption.md):
  first external-dogfood plan for adopting the spec-system preview without
  making it a hard gate immediately.
- [Migration from xtask](migration-from-xtask.md): how bespoke allowlist tasks
  should move into cargo-allow.
- [Migrate from xtask](how-to/migrate-from-xtask.md): task guide for legacy
  policy migration.
- [Migration evidence cookbook](how-to/migration-evidence-cookbook.md): examples
  for preserving migration evidence.
- [Close unsafe migration evidence](how-to/close-unsafe-migration-evidence.md):
  close retained unsafe migration evidence.

## Release And Publish

- [Self-hosting readiness](readiness/self-hosting.md): current proof-stack
  record for docs, cargo-allow no-new, spec-system, ripr+, and unsafe-review+.
- [0.1.0 release runbook](release/0.1.0.md): publish order, dry-run sequencing,
  and rollback limits.
- [0.1.1 release record](release/0.1.1.md): completed patch release and
  publication evidence.
- [0.1.2 release record](release/0.1.2.md): completed patch release for the
  post-0.1.1 receipt inventory contract.
- [0.1.3 release record](release/0.1.3.md): completed patch release for the
  post-0.1.2 evidence, diff posture, and source-snapshot hardening.
- [0.1.4 release record](release/0.1.4.md): completed patch release for the
  post-0.1.3 setup diagnostics, evidence routing, README logo, and scanner
  identity hardening.
- [0.1.5 release record](release/0.1.5.md): completed patch release for
  evidence-health repair queue routing metadata and unsafe-scoped migration
  evidence repair routes.
- [0.1.6 release record](release/0.1.6.md): completed patch release for
  source-syntax identity hardening and legacy evidence preservation.
- [0.1.7 release record](release/0.1.7.md): completed patch release for the
  opt-in spec-system preview profile.
- [0.1.7 GitHub Release body](release/github/v0.1.7.md): public release notes
  for the `v0.1.7` GitHub Release.
- [0.1.8 release record](release/0.1.8.md): completed patch release for
  spec-system first-hour adoption cleanup.
- [0.1.8 GitHub Release body](release/github/v0.1.8.md): public release notes
  for the `v0.1.8` GitHub Release.
- [0.1.9 release record](release/0.1.9.md): completed maintenance release.
- [0.1.9 GitHub Release body](release/github/v0.1.9.md): public notes for
  `v0.1.9`.
- [0.1.10 readiness assessment](release/0.1.10-readiness.md): deferred
  adoption-trust release checklist.
- [0.1.10 release record](release/0.1.10.md): completed adoption-trust and ledger-coherence patch release.
- [0.1.10 GitHub Release body](release/github/v0.1.10.md): public notes for
  `v0.1.10`.
- [0.1.11 readiness snapshot](release/0.1.11-readiness.md): current
  qualification and publication posture.
- [0.1.11 release record](release/0.1.11.md): supported-core usability, safety-bound, and installed-candidate patch release.
- [0.1.11 GitHub Release body](release/github/v0.1.11.md): public notes for `v0.1.11`.
- [0.2.0 release record](release/0.2.0.md): pre-publication release contract and
  candidate posture.
- [0.2.0 GitHub Release body](release/github/v0.2.0.md): public notes for the
  `v0.2.0` candidate.

## Reference

- [How-to guides](how-to/README.md): task guide index.
- [Install shell completions](how-to/install-shell-completions.md): generate
  version-matched completion scripts from the installed binary.
- [Error codes and exit codes](error-codes.md): stable `E000*` registry and
  process exit mapping (`0` / `1` / `2`).
- [CI](ci.md): GitHub Actions examples for PR posture diffs, mainline checks,
  and opt-in profile artifacts.
- [Crates](crates.md): workspace crate responsibilities and library namespace
  policy.
- [Crate namespace](crate-namespace.md): first-party crate naming policy.
- [Agent worklist prompt](agents/cargo-allow-worklist.md): bounded agent use of
  `cargo-allow worklist`.
- [Schemas](schemas/README.md): the current JSON artifact-contract catalog,
  including source reports, receipts, `refresh` and other mutation summaries,
  the `spec-system` graph report, and worklists. The catalog also documents
  federation and movement/posture fields carried by existing v1 artifacts.
