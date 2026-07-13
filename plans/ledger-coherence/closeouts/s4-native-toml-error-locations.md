---
id: CARGO-ALLOW-CLOSEOUT-0027
kind: closeout
status: done
owner: repo-infra
created: 2026-07-13
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2021
merged_commit: 18a47739
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: S4 Native TOML Error Locations

## Landed

- `CargoAllowErrorLocation` carries an optional source path and one-based line
  and character column.
- `CargoAllowError::with_toml_span` converts TOML byte spans without relying on
  parser display text; `location()` exposes the structured value.
- Native `policy/allow.toml` parse failures now use `InvalidPolicy` and retain
  their existing human-readable message while exposing the structured location.
- The located-error contract is documented in `docs/error-codes.md`.

## Acceptance proof

- Failing-first tests covered core span conversion and policy parse locations.
- `cargo test --workspace --locked`: 2,041 passed across 45 suites.
- Workspace Clippy and formatting passed.
- Current-main no-new guard passed.
- PR #2130 CI passed; CodeRabbit completed without a blocking result.
- UB Review stopped at the repository's missing `MINIMAX_API_KEY` guard before
  code review; no UB finding was emitted.

## Claim boundary and remaining work

This closes the native policy parser slice of #2021. Federation, import-root,
spec-system, doc-artifact, legacy, and aggregated validation diagnostics still
need the same structured location contract and remain open follow-up work.
