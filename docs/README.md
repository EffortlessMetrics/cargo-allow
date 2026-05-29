# cargo-allow docs

The docs are organized with the Diataxis model so readers can choose the right
kind of help for their current task: learn, solve, look up, or understand.

## Tutorials

Learning-oriented walkthroughs for first-time users.

- [Quickstart tutorial](quickstart.md): create a policy, inspect findings,
  generate a proposed baseline, and run a no-new check.

## How-to guides

Task-oriented recipes for maintainers.

- [Adopt cargo-allow with no-new mode](how-to/adopt-no-new.md): introduce CI
  protection while managing historical baseline debt.
- [Triage exception cleanup with worklists](how-to/triage-worklist.md): route
  cleanup work by intent, owner, path, package, or allow ID.
- [CI](ci.md): GitHub Actions examples for PR posture diffs and mainline
  checks.
- [Migration from xtask](migration-from-xtask.md): move bespoke allowlist tasks
  into cargo-allow.
- [0.1.0 release runbook](release/0.1.0.md): publish order, dry-run sequencing,
  and rollback limits.
- [Agent worklist prompt](agents/cargo-allow-worklist.md): bounded agent use of
  `cargo-allow worklist`.

## Reference

Information-oriented material for lookup.

- [CLI reference](reference/cli.md): command purposes, common formats, and
  artifact paths.
- [Source exception ledger](source-exception-ledger.md): policy concepts and
  entry model.
- [Structural identity v1](identity.md): source-syntax identity contract used by
  matching and diff posture.
- [JSON schemas](schemas/README.md): schemas for doctor, report, receipt,
  explain, list, prune, propose, add, migrate, and worklist artifacts.

## Explanation

Understanding-oriented material about design decisions and boundaries.

- [Source-tree boundary](explanation/source-tree-boundary.md): what
  `cargo-allow` can and cannot claim, and why evidence references are not proof.
- [Design](design.md): cargo-allow's source-exception governance model.
- [Claim boundaries](claim-boundaries.md): what current and future reports may
  claim.
- [Crate namespace policy](crate-namespace.md): why implementation crates use
  the `allow-*` namespace.
- [Roadmap](roadmap.md): the PR-sized path from MVP to useful product.
