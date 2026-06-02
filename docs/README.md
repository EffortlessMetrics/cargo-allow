# cargo-allow docs

These documents define the product lane for the current source-tree exception
ledger and the planned growth around it.

- [Design](design.md): cargo-allow's source-exception governance model.
- [Claim boundaries](claim-boundaries.md): what current and future reports may
  claim.
- [Contributing](../CONTRIBUTING.md): local development, product boundaries,
  and pull request expectations.
- [Roadmap](roadmap.md): the PR-sized path from source-tree ledger to mature
  product.
- [Source exception ledger](source-exception-ledger.md): the policy concepts and
  entry model.
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
  repair, stale pruning, migration, and agent worklists.
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
- [Agent worklist prompt](agents/cargo-allow-worklist.md): bounded agent use of
  `cargo-allow worklist`.
- [Schemas](schemas/README.md): JSON schemas for doctor, report, receipt,
  explain, list, prune, propose, add, migrate, and worklist artifacts.
