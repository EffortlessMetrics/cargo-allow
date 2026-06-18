---
id: CARGO-ALLOW-CLOSEOUT-0008
kind: closeout
status: done
owner: repo-infra
created: 2026-06-18
linked_plan: CARGO-ALLOW-PLAN-0004
linked_proposal: CARGO-ALLOW-PROP-0004
linked_spec: CARGO-ALLOW-SPEC-0004
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0003
support_tier_impact: advisory
policy_impact:
  - .allow/profiles/spec-system.toml
  - .allow/artifacts/doc-artifacts.toml
  - .allow/goals/active.toml
  - policy/allow.toml
---

# Closeout: Dogfood Migrate Profile State to `.allow/` (C4)

## Summary

Execution closeout for CARGO-ALLOW-PLAN-0004 C4. This repository's dogfood
profile state now lives under `.allow/`:

- `.allow/profiles/spec-system.toml`
- `.allow/artifacts/doc-artifacts.toml`
- `.allow/goals/active.toml` and `.allow/goals/archive/`
- `.allow/imports/` stub

Legacy `policy/spec-system.toml`, `policy/doc-artifacts.toml`, and canonical
`.codex/goals/` profile paths are removed. `.codex/goals/README.md` remains as a
non-canonical migration pointer. `policy/allow.toml` stays the source-exception
ledger. C2 resolution fallback and legacy-path fixtures in tests remain for
compatibility.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `parses_current_repository_active_goal_manifest` | pass | #1752 merge `651d9c90` |
| `cargo-allow check --profile spec-system --mode audit` | pass | #1752 merge `651d9c90` |
| `cargo-allow check --mode no-new` | pass | `target/cargo-allow/check.receipt.json` |
| `cargo test -p cargo-allow init` | pass | #1752 merge `651d9c90` |
| `cargo test -p cargo-allow doctor` | pass | #1752 merge `651d9c90` |

## Non-Goals

- Import adapters (C8–C11) or import-root config (C5).
- P2 multi-ledger federation (#1473).
- Full import mode (#1466) or external `ripr` migration.
- Version bump or `0.1.10` release authorization.

## Remaining Work

- **Active goal:** CARGO-ALLOW-GOAL-0003 portable governance substrate.
- **Done:** `portable-governance-c4` (dogfood migrate profile state to `.allow/`; #1752 merge `651d9c90`).
- **Design-ready (blocked):** `portable-governance-f0-federation` (#1473; design-first).
- **Blocked:** external ripr adoption, full import mode (#1466).

## Claim Boundary

C4 dogfood migration only. Does not implement import adapters, federation, or
authorize release cut.
