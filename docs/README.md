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
- [Structural identity v1](identity.md): the source-syntax identity contract
  used by matching and diff posture.
- [Migration from xtask](migration-from-xtask.md): how bespoke allowlist tasks
  should move into cargo-allow.
- [CI](ci.md): GitHub Actions examples for PR posture diffs and mainline
  checks.
- [0.1.0 release runbook](release/0.1.0.md): publish order, dry-run sequencing,
  and rollback limits.
- [0.1.1 release prep](release/0.1.1.md): patch-release handoff for the current
  post-0.1.0 product hardening.
- [Agent worklist prompt](agents/cargo-allow-worklist.md): bounded agent use of
  `cargo-allow worklist`.
- [Schemas](schemas/README.md): JSON schemas for doctor, report, receipt,
  explain, list, prune, propose, add, migrate, and worklist artifacts.
