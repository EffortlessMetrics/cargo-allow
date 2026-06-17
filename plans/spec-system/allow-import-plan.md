---
id: CARGO-ALLOW-PLAN-0004
kind: implementation_plan
status: draft
owner: repo-infra
created: 2026-06-17
linked_proposal: CARGO-ALLOW-PROP-0004
linked_spec: CARGO-ALLOW-SPEC-0004
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
support_tier_impact: advisory
policy_impact: none
---

# Implementation Plan: `.allow` Namespace and Imports

## Purpose

Sequence spec-system portability work after readiness policy is settled. Design
is accepted; implementation waits for explicit PR authorization.

## Non-Goals

- No full import support in `0.1.10`.
- No rewrite of foreign spec systems.
- No profile state migration until C4 is explicitly authorized.

## PR Sequence (C1–C12)

| PR | Title | Scope | 0.1.10 candidate? |
| --- | --- | --- | --- |
| C1 | Define `.allow` namespace and import policy | docs/spec only | yes (landed in design spec) |
| C2 | `.allow` profile resolution with policy fallback | `crates/` resolver | maybe if low risk |
| C3 | `init` writes spec-system state to `.allow/` | CLI init | no |
| C4 | Dogfood migrate profile state to `.allow/` | this repo | no |
| C5 | Import-root config | `policy/spec-system.toml` | no |
| C6 | Normalize imported graph nodes/edges | spec-system scanner | no |
| C7 | Worklist route import diagnostics | worklist | no |
| C8 | Discover Kiro imports | import adapter | no |
| C9 | Discover Spec Kit imports | import adapter | no |
| C10 | Discover generic `.spec`/`.rails`/repo-spec | import adapter | no |
| C11 | xtask command registry support | import adapter | no |
| C12 | Import adoption guide | docs/how-to | no |

## Claim Boundary

This plan sequences portability work. C1 is documentation only and does not
change runtime behavior.
