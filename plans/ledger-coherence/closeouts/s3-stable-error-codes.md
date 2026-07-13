---
id: CARGO-ALLOW-CLOSEOUT-0026
kind: closeout
status: done
owner: repo-infra
created: 2026-07-13
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2020
merged_commit: c15aa9f1
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: S3 Stable Error Codes

## Landed

- `CargoAllowErrorKind::code()` assigns stable `E0001_*` through `E0009_*`
  identifiers to every current error kind.
- `CargoAllowError::code()` exposes the kind code without requiring callers to
  inspect the error's private representation.
- `docs/error-codes.md` is the append-only downstream registry; existing kind
  strings, messages, and cause rendering remain unchanged.
- `CargoAllowErrorKind::ALL` and tests enforce uniqueness for the current
  registry while preserving the enum's non-exhaustive compatibility boundary.

## Acceptance proof

- `cargo test -p allow-core --locked`: 93 passed.
- Workspace Clippy: no issues found.
- `cargo fmt --all --check`: passed.
- Current-main no-new guard: passed with `allow-4722` documenting the registry.
- PR #2128 CI test: passed.
- UB Review stopped at the repository's missing `MINIMAX_API_KEY` guard before
  code review; no UB finding was emitted.

## Claim boundary and remaining work

This closes the stable broad error-kind code registry tracked by #2020. It does
not claim per-validation-rule codes, structured TOML source spans, aggregated
diagnostic collections, or typed error receipts; those remain separate lanes.
