---
id: CARGO-ALLOW-PROP-0004
kind: proposal
status: accepted
owner: repo-infra
created: 2026-06-17
linked_specs:
  - CARGO-ALLOW-SPEC-0004
support_tier_impact: advisory
policy_impact: none
---

# Proposal: `.allow` Namespace and Import Roots

## Summary

cargo-allow-owned governance profile state should live under `.allow/` while
legacy `policy/` paths remain supported as compatibility fallbacks. External
tool directories are import/read roots by default, not rewrite targets.

## Problem

The current `.codex/goals/` namespace is convenient for dogfooding but is not
portable across repositories and tool ecosystems. Spec-system portability needs
a clear ownership model before implementation moves profile state.

## Proposed Shape

```text
.allow/           cargo-allow-owned profile/config/artifact/goal state
policy/allow.toml still supported source-exception policy
policy/spec-system.toml, policy/doc-artifacts.toml compatibility fallback
.kiro/, .specify/, .spec/, .rails/, .<repo>-spec/, xtask/ import roots (read-only default)
```

## Non-Goals

- No full import implementation in `0.1.10`.
- No rewrite of `.kiro`, `.specify`, `.spec`, `.rails`, or foreign systems by
  default.
- No migration of cargo-allow profile state to `.allow/` until readiness
  policy is settled and an explicit implementation PR is authorized.

## Claim Boundary

This proposal defines namespace ownership and import posture. It does not
implement resolution, discovery, or migration.
