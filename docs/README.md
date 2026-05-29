# cargo-allow docs

These documents define the product lane before the implementation expands
beyond the MVP scaffold. They are organized with the Diátaxis documentation
model so readers can choose the right kind of material for their task.

## Tutorials

Learning-oriented walkthroughs for first-time users.

- [Your first source-exception ledger](tutorials/first-ledger.md): create a
  strict policy, inventory current findings, generate a temporary baseline, and
  run the no-new gate.

## How-To Guides

Task-oriented recipes for people adopting, operating, or maintaining a ledger.

- [CI](ci.md): GitHub Actions examples for PR posture diffs and mainline
  checks.
- [Migration from xtask](migration-from-xtask.md): how bespoke allowlist tasks
  should move into cargo-allow.
- [Agent worklist prompt](agents/cargo-allow-worklist.md): bounded agent use of
  `cargo-allow worklist`.
- [0.1.0 release runbook](release/0.1.0.md): publish order, dry-run sequencing,
  and rollback limits.

## Reference

Information-oriented lookup material with stable contracts and command details.

- [Command reference](reference/commands.md): command purposes, common filters,
  and artifact conventions.
- [Schemas](schemas/README.md): JSON schemas for doctor, report, receipt,
  explain, list, prune, propose, add, migrate, and worklist artifacts.
- [Structural identity v1](identity.md): the source-syntax identity contract
  used by matching and diff posture.
- [Crate namespace policy](crate-namespace.md): naming rules for first-party
  crates and future integrations.

## Explanation

Understanding-oriented background about cargo-allow's model and boundaries.

- [Design](design.md): cargo-allow's source-exception governance model.
- [Claim boundaries](claim-boundaries.md): what current and future reports may
  claim.
- [Source exception ledger](source-exception-ledger.md): the policy concepts and
  entry model.
- [Roadmap](roadmap.md): the PR-sized path from MVP to useful product.

## Choosing A Document

- Start with the tutorial when you are learning the workflow.
- Use how-to guides when you already know the goal and need a repeatable recipe.
- Use reference pages when you need exact command, schema, or contract details.
- Use explanation pages when you need to understand why cargo-allow behaves the
  way it does.
