# cargo-allow docs

These documents define the product lane for the current source-tree exception
ledger and the planned growth around it.

- [Changelog](../CHANGELOG.md): curated user-facing release ledger.
- [Design](design.md): cargo-allow's source-exception governance model.
- [Claim boundaries](claim-boundaries.md): what current and future reports may
  claim.
- [Contributing](../CONTRIBUTING.md): local development, product boundaries,
  and pull request expectations.
- [Roadmap](roadmap.md): the PR-sized path from source-tree ledger to mature
  product.
- [Source exception ledger](source-exception-ledger.md): the policy concepts and
  entry model.
- [Source-of-truth stack](source-of-truth/README.md): planned opt-in
  governance graph for proposals, specs, ADRs, plans, goals, support tiers,
  policy ledgers, and closeouts.
- [Doc artifact ledger](source-of-truth/doc-artifact-ledger.md): advisory
  registry for governed proposal/spec artifacts in the planned source-of-truth
  graph.
- [Source-of-truth templates](templates/proposal.md): starter templates for
  proposals, specs, ADRs, implementation plans, plan items, closeouts, and PR
  bodies.
- [Spec-system profile proposal](proposals/CARGO-ALLOW-PROP-0001-spec-system-profile.md):
  accepted product proposal for the planned opt-in source-of-truth profile.
- [Spec-system profile spec](specs/CARGO-ALLOW-SPEC-0001-spec-system-profile.md):
  accepted behavior contract for the planned opt-in source-of-truth profile.
- [Support tiers](status/SUPPORT_TIERS.md): claim-to-proof map for current
  cargo-allow surfaces and the planned advisory spec-system profile.
- [Active goal manifest](../.codex/goals/README.md): current Codex execution
  state for the planned source-of-truth profile.
- [Spec-system implementation plan](../plans/spec-system/implementation-plan.md):
  PR sequence for implementing the opt-in source-of-truth profile.
- [Getting started](getting-started.md): first-run tutorial for doctor, audit,
  init/propose, and no-new checks.
- [Structural identity v1](identity.md): the source-syntax identity contract
  used by matching and diff posture.
- [PR posture](pr-posture.md): reviewer-facing diff posture and net posture
  semantics.
- [Policy weakening](policy-weakening.md): policy edits that broaden, weaken,
  or improve retained source exceptions.
- [Migration from xtask](migration-from-xtask.md): how bespoke allowlist tasks
  should move into cargo-allow.
- [CI](ci.md): GitHub Actions examples for PR posture diffs and mainline
  checks.
- [How-to guides](how-to/README.md): task guides for CI, explain, evidence
  repair, PR posture review, stale pruning, migration, migration evidence
  closeout, and agent worklists.
- [Crates](crates.md): workspace crate responsibilities and library namespace
  policy.
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
- [Agent worklist prompt](agents/cargo-allow-worklist.md): bounded agent use of
  `cargo-allow worklist`.
- [Schemas](schemas/README.md): JSON schemas for doctor, report, receipt,
  explain, list, prune, propose, add, migrate, and worklist artifacts.
