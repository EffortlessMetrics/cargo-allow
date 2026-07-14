---
id: CARGO-ALLOW-CLOSEOUT-0028
kind: closeout
status: done
owner: repo-infra
created: 2026-07-13
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2021
merged_commit: 70cbc8a8
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: Auxiliary TOML Error Locations

## Landed

- Federation, import-root, and spec-system configuration parsers now expose
  source paths with structured TOML locations.
- Active-goal and doc-artifact parsing preserves the originating file path.
- Legacy compatibility loaders pass their source path through the same located
  error contract.
- Regression tests cover each auxiliary parser family without changing the
  existing human-readable error messages.

## Acceptance proof

- `cargo test -p allow-policy --locked`: 368 passed.
- `cargo test -p allow-policy-legacy --locked`: 230 passed.
- `cargo test --workspace --locked`: 2,045 passed across 45 suites.
- Workspace Clippy, formatting, `cargo check --workspace --locked`, and
  `git diff --check` passed.
- Current-branch no-new guard passed.
- PR #2132 merged as `70cbc8a8`; CI, CodeRabbit, and GitGuardian passed.
- UB Review stopped at the repository's missing `MINIMAX_API_KEY` guard before
  code review; no UB finding was emitted.

## Claim boundary and remaining work

This closes the auxiliary parser-location slice of #2021. It does not close
the issue: aggregated validation, federation/import/spec-system semantic
validation, and any remaining non-TOML parser paths still need explicit
structured diagnostics and remain open follow-up work.
