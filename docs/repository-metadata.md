# Repository Metadata Candidate

This file records the reviewed source candidate for the public repository
metadata. It is an operator handoff, not a live GitHub-settings receipt. A
future settings change must capture the current public values, apply only this
reviewed candidate, and capture the resulting values separately.

## Candidate

| Field | Proposed value | Boundary |
| --- | --- | --- |
| Description | `Source-tree exception ledger and policy scanner for Rust repositories.` | Describes the primary cargo-allow product; does not claim complete analyzer coverage or release readiness. |
| Homepage | `https://github.com/EffortlessMetrics/cargo-allow` | Stable repository and getting-started surface; not a temporary campaign or unreleased version. |
| Topics | `rust`, `cargo`, `static-analysis`, `policy-as-code`, `developer-tools` | Concrete discovery terms only; no unsupported runtime, proof, safety, or support claims. |
| Delete branches on merge | `true` for ordinary merged branches | Repository setting; does not establish branch protection or authorize deletion of protected, release, incident, or unmerged branches. |

The README tagline and workspace repository metadata are the source-controlled
identity surfaces for this candidate. The repository description and topics are
public GitHub state and therefore require a separate read-before-write and
read-after-write receipt.

## Authority split

- Source files establish the reviewed candidate and its claim boundary.
- GitHub repository settings establish the live public metadata and merge
  hygiene state.
- A source candidate must not be reported as live state without a fresh public
  API observation.
- Live application must not change repository visibility, default branch,
  rulesets, required checks, secrets, environments, tags, releases, merge
  methods, or support tiers.

## Reconciliation

The live operator should retain, at minimum:

1. the exact before-values for description, homepage, topics, and branch
   deletion;
2. the exact candidate revision used for the write;
3. the exact after-values returned by GitHub;
4. any permission or provider failure as an unsuccessful application;
5. a semantic diff showing whether only the selected fields changed.

This document does not close the live-settings or source/live reconciliation
work in the parent issue.
