---
id: CARGO-ALLOW-SPEC-0013
kind: spec
status: draft
owner: repo-infra
created: 2026-08-24
linked_proposal:
standalone_reason: Diff posture is an existing reviewer-facing core contract described by current documentation and implementation; this draft records its semantics before future changes extend them.
linked_adrs: []
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
  - .allow/artifacts/doc-artifacts.toml
---

# Spec: Diff Posture

## Summary

`cargo-allow diff` compares source-tree/source-syntax findings and exception
policy across revisions and reports the reviewer-facing posture change. It
keeps source presence, policy quality, and review routing distinct: a removed
finding is not the same thing as improved evidence, and a policy edit may be
review-required even when source findings are unchanged.

This specification formalizes the current contract described in [PR
Posture](../pr-posture.md) and [Policy Weakening](../policy-weakening.md).

## Behavior Contract

The system must:

- compare a declared base and head using the same source-tree and selected
  policy path, with revision provenance visible in the result;
- classify net posture as `worse`, `review-required`, `unchanged`, or
  `improved` according to the highest-severity observed signal;
- distinguish source finding movement from policy posture changes;
- report weakening signals such as scope broadening, selector precision loss,
  occurrence-limit loosening, evidence/owner/reason removal, baseline-debt
  introduction, requirement loosening, and added ignored inventory scopes;
- report review-required changes such as equal-precision selector changes,
  owner/reason/classification changes, lifecycle extensions, and retargeted
  scopes;
- report ordinary improvements such as removed findings, narrowed scopes,
  increased selector precision, tightened limits, added typed evidence, and
  reduced generated debt;
- retain stable entry identity, movement details, change kind, and enough
  provenance for a human or machine consumer to reproduce the routing decision.

The system must not:

- collapse policy quality into source finding counts;
- treat a status label or advisory result as a no-new approval;
- infer macro expansion, type information, reachability, proof adequacy, or
  runtime safety from a clean diff;
- classify an unrecognized or incomplete revision comparison as unchanged;
- hide a policy weakening signal merely because the source tree improved.

## Inputs and Outputs

| Input | Required | Notes |
| --- | --- | --- |
| Base revision | yes | Git revision or supported equivalent. |
| Head/current revision | yes | The comparison target. |
| Source-exception policy | yes when applicable | One selected policy path is used for both revisions; missing-side state is explicit. |
| Selected source inventory | yes | Must preserve partial/error posture. |

| Output | Required | Notes |
| --- | --- | --- |
| Per-row movement/posture | yes | Includes stable ID and change classification. |
| Net posture | yes | Highest-severity summary for reviewer routing. |
| Human/JSON projection | command-dependent | Same semantic result in each supported format. |

## Accepted States

- A complete comparison identifies both revisions and reports any scanner or
  policy-resolution limitation instead of silently omitting it.
- The current comparison resolver selects one policy path for the comparison
  (preferring the head when supported paths differ), reads that path at both
  revisions, and represents a missing side explicitly rather than claiming
  independent per-revision policy discovery.
- A removed finding is represented as movement, while evidence or selector
  changes remain policy posture signals.
- A weakening signal yields at least `review-required` and uses `worse` where
  the contract defines the change as directly weakening approval.
- Equivalent unchanged policy and source produce `unchanged`, not an inferred
  improvement.
- A finding or policy improvement produces `improved` only when no worse or
  review-required signal outranks it.

## Rejected States

- Missing base/head state presented as a clean comparison.
- A broadening, precision loss, owner/evidence removal, or ignored-scope
  addition reported as neutral.
- Policy-only changes omitted because source findings did not move.
- A partial scan or failed revision read represented as zero findings.

## Artifact Links

- Reviewer contract: [PR Posture](../pr-posture.md).
- Signal definitions: [Policy Weakening](../policy-weakening.md).
- Entry model: [Source Exception Ledger](../source-exception-ledger.md).
- Registry: [doc-artifacts.toml](../../.allow/artifacts/doc-artifacts.toml).

## Required Evidence

- Paired fixtures for each weakening, review-required, improvement, and
  unchanged signal.
- Cross-format parity tests for human, Markdown, JSON, receipt, and worklist
  projections where those surfaces exist.
- Revision-error and partial-inventory fixtures proving incomplete comparisons
  fail visibly rather than becoming clean results.

## Non-Goals

- Deciding whether a reviewer should approve a pull request.
- Running tests, proof tools, Cargo, rustc, network services, or repository code.
- Replacing source-exception matching or inventing a second ledger identity.

## Claim Boundary

This spec defines reviewer-facing classification of observed source and policy
changes. A clean result supports only the claim that no covered posture signal
was observed in the selected comparison. It does not prove the repository is
safe, correct, tested, covered, or release-ready.

## Rollback Or Compatibility

The draft records the existing posture vocabulary and output semantics. Future
movement/posture extensions must preserve current fields or provide an explicit
versioned projection so existing CI and review consumers do not reinterpret a
historical result.
