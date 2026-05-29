# cargo-allow docs

These documents define and operate cargo-allow's source exception ledger. They
are organized with the Diataxis model so readers can choose the right page for
their situation.

- [Diataxis documentation map](diataxis.md): tutorial, how-to, reference, and
  explanation entry points.

## Tutorials

- [Getting started](tutorials/getting-started.md): create a policy, run the
  first audit, add or propose entries, gate future changes, and create review
  artifacts.

## How-to guides

- [Adopt no-new in CI](how-to/adopt-no-new.md): add the normal PR gate and save
  Markdown/receipt artifacts.
- [Triage worklist items](how-to/triage-worklist.md): route baseline debt, stale
  entries, broken evidence links, and broad-scope cleanup.
- [Migration from xtask](migration-from-xtask.md): move bespoke allowlist tasks
  into cargo-allow.
- [CI](ci.md): GitHub Actions examples for PR posture diffs and mainline checks.
- [Agent worklist prompt](agents/cargo-allow-worklist.md): bounded agent use of
  `cargo-allow worklist`.

## Reference

- [CLI reference](reference/cli.md): command map, shared options, and common
  invocations.
- [Source exception ledger](source-exception-ledger.md): policy concepts, entry
  model, selector precision, lifecycle, command behavior, and non-Rust files.
- [Schemas](schemas/README.md): JSON schemas for doctor, report, receipt,
  explain, list, prune, propose, add, migrate, and worklist artifacts.
- [Crate namespace policy](crate-namespace.md): rules for first-party public
  crates.
- [0.1.0 release runbook](release/0.1.0.md): publish order, dry-run sequencing,
  and rollback limits.

## Explanation

- [Design](design.md): cargo-allow's product lane, governance model, matching
  direction, evidence model, and non-goals.
- [Claim boundaries](claim-boundaries.md): what current and future reports may
  claim.
- [Structural identity v1](identity.md): the source-syntax identity contract used
  by matching and diff posture.
- [Roadmap](roadmap.md): the PR-sized path from MVP to useful product.
- [Shiplog file-policy dogfood](dogfood/shiplog-non-rust.md): current dogfood
  evidence and replacement boundaries.
