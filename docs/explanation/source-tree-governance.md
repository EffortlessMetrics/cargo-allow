# Explanation: Source-Tree Governance

cargo-allow treats exceptions as governance records rather than compiler or
linter suppressions. Its core question is not "is this program correct?" but
"is every retained source-tree exception visible, owned, scoped, evidenced, and
reviewable?"

## Why The Boundary Is Source-Tree First

A source-tree scan is available before a successful build. That makes it useful
for repositories with incomplete toolchains, generated files, partial platform
support, or policy-only review. cargo-allow reads repository files and visible
source syntax; it does not need Cargo metadata, rustc, Clippy, build scripts, or
external proof tools to produce its own inventory.

This boundary keeps the claim narrow. A passing report can say that no new
unreceipted findings were found in the scanned source-tree inventory. It cannot
say that no unsafe, panic, lint suppression, or operational risk exists outside
that scanned surface.

## Why Receipts Instead Of Suppressions

A suppression hides a finding from future attention. A cargo-allow receipt keeps
attention attached to the finding. The policy entry records:

- who owns the exception;
- why the exception is allowed;
- which source surface it covers;
- which evidence or review trail supports the rationale; and
- when the exception must be reviewed or removed.

That receipt can then be diffed. Pull requests that remove rationale, broaden
scope, weaken selectors, or extend expiry become visible policy changes rather
than invisible cleanup noise.

## Why Evidence Is Traceability, Not Proof

Evidence references connect a policy entry to tests, docs, review artifacts,
issues, or tool receipts. cargo-allow validates local evidence paths for known
local prefixes, but it does not execute tests, run external analyzers, fetch
network resources, or interpret third-party report formats as proof.

This separation lets repositories use evidence from many tools while preserving
a single cargo-allow claim boundary: the ledger records traceability and review
state for source-tree exceptions.

## Why Baseline Debt Is Allowed

Real repositories often start with existing exceptions. Requiring a perfect
ledger before adoption would delay visibility. `baseline_debt` entries allow a
repository to adopt no-new enforcement while making existing debt explicit.

Baseline debt should be temporary. It should carry expiry pressure and should be
burned down by removing the source exception, adding a reviewed receipt, or
narrowing the selector to the intended source surface.
