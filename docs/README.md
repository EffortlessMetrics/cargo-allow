# cargo-allow docs

These documents define the product lane before the implementation expands
beyond the MVP scaffold.

- [Design](design.md): cargo-allow's source-exception governance model.
- [Claim boundaries](claim-boundaries.md): what current and future reports may
  claim.
- [Roadmap](roadmap.md): the PR-sized path from MVP to useful product.
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
- [Agent worklist prompt](agents/cargo-allow-worklist.md): bounded agent use of
  `cargo-allow worklist`.
- [Schemas](schemas/README.md): JSON schemas for report, receipt, explain,
  list, prune, and worklist artifacts.
