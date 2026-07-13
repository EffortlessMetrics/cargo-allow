---
id: CARGO-ALLOW-CLOSEOUT-0025
kind: closeout
status: done
owner: repo-infra
created: 2026-07-13
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/1807
merged_commit: fdb457af
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: S2 Match-Layer Occurrence Accounting

## Landed

- `allow_match::evaluate_detailed` now returns deterministic accounting for
  every policy entry with an `occurrence_limit`.
- Each `OccurrenceAccounting` record exposes the allow ID, observed count,
  configured limit, remaining headroom, and exceeded count.
- Observed counts include findings that arrive after the configured limit is
  exhausted, so overflow is visible without parsing human-readable messages.
- The existing `allow_match::evaluate` API remains unchanged and returns the
  existing `Vec<MatchOutcome>` behavior.

## Acceptance proof

- `crates/allow-match/src/tests/evaluation.rs` covers exhausted limits,
  remaining headroom, and zero-use limited entries.
- Current-main `cargo test -p allow-match --locked`: 70 passed.
- Current-main `cargo test --workspace --locked`: 2,037 passed across 45 suites.
- Current-main Clippy: no issues found.
- Current-main no-new guard: passed.
- PR #2126 CI: both test checks passed. UB Review stopped at the repository's
  missing `MINIMAX_API_KEY` guard before code review; no UB finding was emitted.

## Claim boundary and remaining work

This closes the allow-match visibility gap tracked by #1807. It does not yet
replace every report/check consumer's derived occurrence summary or complete
the broader shared read model and typed artifact work; those remain separate
roadmap lanes.
