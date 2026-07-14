---
id: CARGO-ALLOW-CLOSEOUT-0029
kind: closeout
status: done
owner: repo-infra
created: 2026-07-13
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2017
merged_commit: 1b1c4519
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: Structured Policy Validation Diagnostics

## Landed

- `allow-core` now exposes `CargoAllowDiagnostic` and
  `CargoAllowDiagnosticSeverity` as shared machine-readable diagnostic types.
- `CargoAllowError::diagnostics()` preserves independent validation failures
  through single-error and aggregate policy-validation paths.
- Entry diagnostics carry stable error code, category, severity, allow-entry ID,
  and validation field while retaining the existing human-readable summary.
- Core policy validation carries the same diagnostic payload for schema, policy,
  status, and workspace-mode failures.
- The public contract is documented in `docs/error-codes.md` and the unreleased
  changelog.

## Acceptance proof

- `cargo test -p allow-core --locked`: 95 passed.
- `cargo test -p allow-policy --locked`: 370 passed.
- Workspace Clippy with `-D warnings` passed.
- `cargo check --workspace --locked` passed.
- `git diff --check` and the post-merge current-main no-new guard passed.
- PR #2134 merged as `1b1c4519`; CI, GitGuardian, Graphite review, and
  mergeability passed.

## Validation boundary and remaining work

This is the first aggregation slice of #2017. It does not yet convert the
existing federation, import-root, or evidence diagnostic structs to the shared
type, nor does it add semantic-validation source spans or one renderer for all
diagnostic families. The full workspace test was attempted but reached the
10-minute Windows timeout in slow subprocess fixtures without emitting a test
failure. UB Review stopped at the missing `MINIMAX_API_KEY` preflight, and
CodeRabbit was quota-pending; neither produced an actionable code finding.
