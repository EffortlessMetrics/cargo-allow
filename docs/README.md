# cargo-allow docs

These documents are organized with the Diátaxis documentation model: tutorials
for learning, how-to guides for tasks, reference for lookup, and explanation for
conceptual background.

## Start Here

- New to cargo-allow? Follow the [first ledger tutorial](tutorials/first-ledger.md).
- Adopting CI with existing findings? Use the [no-new adoption guide](how-to/adopt-no-new.md).
- Looking up commands or artifacts? Open the [command reference](reference/commands.md).
- Checking the product boundary? Read [source-tree governance](explanation/source-tree-governance.md)
  and [claim boundaries](claim-boundaries.md).

## Tutorials

Tutorials teach by walking through a concrete path from start to finish.

- [Create your first source exception ledger](tutorials/first-ledger.md): create
  policy, inventory findings, promote one reviewed receipt, and run no-new mode.

## How-To Guides

How-to guides solve focused operational tasks.

- [Adopt cargo-allow in no-new mode](how-to/adopt-no-new.md): introduce CI
  without hiding existing baseline debt.
- [Migration from xtask](migration-from-xtask.md): move bespoke allowlist tasks
  into cargo-allow.
- [CI](ci.md): wire PR posture diffs, mainline checks, and artifact uploads.
- [Agent worklist prompt](agents/cargo-allow-worklist.md): bounded agent use of
  `cargo-allow worklist`.
- [0.1.0 release runbook](release/0.1.0.md): publish order, dry-run sequencing,
  and rollback limits.

## Reference

Reference material is for accurate lookup after you know what you need.

- [Command reference](reference/commands.md): command purposes, output patterns,
  gate modes, and durable artifacts.
- [Source exception ledger](source-exception-ledger.md): policy concepts, entry
  fields, selectors, lifecycle, baseline debt, and command semantics.
- [Structural identity v1](identity.md): source-syntax identity contract used by
  matching and diff posture.
- [Crate namespace policy](crate-namespace.md): first-party crate naming rules.
- [Schemas](schemas/README.md): JSON schemas for doctor, report, receipt,
  explain, list, prune, propose, add, migrate, and worklist artifacts.

## Explanation

Explanation docs clarify why cargo-allow is shaped this way.

- [Source-tree governance](explanation/source-tree-governance.md): why
  cargo-allow uses receipts, traceability, and a source-tree-first claim.
- [Design](design.md): cargo-allow's source-exception governance model.
- [Claim boundaries](claim-boundaries.md): what current and future reports may
  claim.
- [Roadmap](roadmap.md): the PR-sized path from MVP to useful product.

## Examples

- [Examples](../examples/README.md): copyable source-tree exception governance
  patterns for adoption, receipts, PR summaries, and worklists.
