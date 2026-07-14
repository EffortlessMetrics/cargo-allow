---
id: CARGO-ALLOW-CLOSEOUT-0030
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/1803
merged_commit: 700bba24
support_tier_impact: advisory
policy_impact: []
---

# Closeout: Structured Match Candidates

## Landed

- `allow_core::MatchOutcome` now carries deterministic `candidate_ids` in
  policy order. Genuine ambiguity reports all tied candidates instead of
  requiring consumers to parse the human-readable message.
- Candidate IDs propagate through cargo-allow worklist items and JSON output.
- Match outcome JSON used by report and explain artifacts emits the same field.
- `common.v1`, report, explain, and worklist schemas describe the shared
  candidate-ID shape. The worklist field remains optional for compatibility
  with older artifacts while current renderers emit it.
- Existing fixture literals and JSON contract tests were updated to preserve
  the public struct and artifact contracts.

## Acceptance proof

- Genuine equal-strength matches assert `Ambiguous`, `allow_id = None`, and
  the complete deterministic candidate list.
- `cargo test -p allow-match --locked`: 70 passed.
- `cargo test -p allow-report --locked`: 219 passed.
- Worklist tests: 110 passed, 589 filtered.
- Shared schema-fragment tests: 3 passed, 696 filtered.
- Workspace Clippy with `-D warnings` passed.
- `cargo check --workspace --locked` passed.
- The current-main no-new guard and `git diff --check` passed.
- PR #2136 merged as `700bba24`; its required CI test passed. UB Review
  stopped at the repository-wide missing `MINIMAX_API_KEY` preflight and
  emitted no code finding.

## Validation boundary and remaining work

This closes the structured candidate-ID portion of #1803. It does not add
candidate strengths or mismatch reasons, occurrence-limit reporting, property
tests over generated finding/policy corpora, or the remaining shared read-model
work. Inventory completeness, typed mutation artifacts, migration, portability,
cross-platform release proof, and scale work remain open under the completion
roadmap.
