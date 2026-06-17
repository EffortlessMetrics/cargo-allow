---
id: CARGO-ALLOW-SPEC-0004
kind: spec
status: accepted
owner: repo-infra
created: 2026-06-17
linked_proposal: CARGO-ALLOW-PROP-0004
linked_adrs: []
support_tier_impact: advisory
policy_impact: none
---

# Spec: `.allow` Namespace and Import Roots (Design)

## Summary

This spec defines the ownership model for cargo-allow governance profile state
and read-only import roots for external spec ecosystems. It is documentation
only for `0.1.10`; implementation is sequenced in
[plans/spec-system/allow-import-plan.md](../../plans/spec-system/allow-import-plan.md).

## Ownership Model

| Root | Role | Default posture |
| --- | --- | --- |
| `.allow/` | cargo-allow-owned profile config, artifact registry, active goal | write target for new repos |
| `policy/allow.toml` | source-exception ledger | supported; canonical for exceptions |
| `policy/spec-system.toml` | spec-system profile config | compatibility fallback |
| `policy/doc-artifacts.toml` | governed artifact registry | compatibility fallback |
| `.codex/goals/` | current dogfood active goal | migrate to `.allow/` later |
| `.kiro/` | Kiro specs | import/read-only |
| `.specify/` | GitHub Spec Kit | import/read-only |
| `.spec/`, `.rails/`, `.<repo>-spec/` | generic spec roots | import/read-only |
| `xtask/` | command registry surface | import/read-only |

## Import Priorities

| Ecosystem | Expected artifacts | First slice |
| --- | --- | --- |
| Kiro | `requirements.md`, `design.md`, `tasks.md` | path + front matter discovery |
| Spec Kit | constitution, spec, plan, tasks, templates | `.specify/` roots |
| Generic | front matter + path conventions | `.spec/`, `.rails/`, repo-spec dirs |
| xtask | command registry | registry first, not Rust dispatch parsing |

## Behavior Contract (Future Implementation)

The system must:

- resolve `.allow/` first, then fall back to `policy/` paths;
- treat import roots as advisory graph inputs unless explicitly promoted;
- emit worklist items for broken imports without rewriting foreign files.

The system must not:

- rewrite imported spec systems by default;
- execute proof commands from imported nodes;
- claim semantic equivalence between foreign and cargo-allow artifact graphs.

## Non-Goals for 0.1.10

- No `.allow/` profile resolution implementation.
- No Kiro/Spec Kit/generic import adapters.
- No dogfood migration of `.codex/goals/` to `.allow/`.

## Linked Artifacts

- Proposal:
  [CARGO-ALLOW-PROP-0004](../proposals/CARGO-ALLOW-PROP-0004-allow-import-profile.md)
- Implementation plan:
  [plans/spec-system/allow-import-plan.md](../../plans/spec-system/allow-import-plan.md)
- Parent profile spec:
  [CARGO-ALLOW-SPEC-0001](CARGO-ALLOW-SPEC-0001-spec-system-profile.md)

## Claim Boundary

This spec defines namespace and import design only. It does not prove import
discovery, graph normalization, or portable profile behavior.
