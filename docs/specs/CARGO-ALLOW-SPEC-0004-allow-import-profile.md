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
and read-only import roots for external spec ecosystems. It was documentation
only for `0.1.10`; the implementation was sequenced in
[plans/spec-system/allow-import-plan.md](../../plans/spec-system/allow-import-plan.md).

The implementation has since landed in the 0.2.x line. The historical
`0.1.10` non-goals below are therefore superseded by implementation; the
design, ownership model, and claim boundary remain the governing reference.

## Ownership Model

| Root | Role | Default posture |
| --- | --- | --- |
| `.allow/` | cargo-allow-owned profile config, artifact registry, active goal | write target for new repos |
| `policy/allow.toml` | source-exception ledger | supported; canonical for exceptions |
| `policy/spec-system.toml` | spec-system profile config | compatibility fallback |
| `policy/doc-artifacts.toml` | governed artifact registry | compatibility fallback |
| `.codex/goals/` | historical dogfood active-goal root | compatibility/read-only source where supported |
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

## Behavior Contract

The implemented profile/import surface must:

- resolve `.allow/` first, then fall back to `policy/` paths;
- treat import roots as advisory graph inputs unless explicitly promoted;
- emit worklist items for broken imports without rewriting foreign files;
- preserve the source ecosystem and implementation generation in its
  diagnostics and artifacts.

It must not:

- rewrite imported spec systems by default;
- execute proof commands from imported nodes;
- claim semantic equivalence between foreign and cargo-allow artifact graphs.

## Historical Non-Goals for 0.1.10 — Superseded

The following constraints described the planned scope of the 0.1.10
documentation/design slice. They are retained as historical context, not as
current statements of product capability:

- No `.allow/` profile resolution implementation.
- No Kiro/Spec Kit/generic import adapters.
- No dogfood migration of `.codex/goals/` to `.allow/`.

The corresponding implementation closeouts are recorded in the
[0.2.0 changelog](../../CHANGELOG.md#020---2026-07-19), including `.allow/`
resolution, `.codex/goals/` migration, and the generic, Kiro, Spec Kit, and
xtask import adapters. Future changes must update this section's status rather
than restoring these items as current non-goals.

## Linked Artifacts

- Proposal:
  [CARGO-ALLOW-PROP-0004](../proposals/CARGO-ALLOW-PROP-0004-allow-import-profile.md)
- Implementation plan:
  [plans/spec-system/allow-import-plan.md](../../plans/spec-system/allow-import-plan.md)
- Parent profile spec:
  [CARGO-ALLOW-SPEC-0001](CARGO-ALLOW-SPEC-0001-spec-system-profile.md)
- Documentation/governance umbrella:
  [issue #1785](https://github.com/EffortlessMetrics/cargo-allow/issues/1785)
- Supersession correction:
  [issue #1972](https://github.com/EffortlessMetrics/cargo-allow/issues/1972)

## Claim Boundary

This spec defines namespace and import design plus the current documented
implementation status. It does not, by itself, prove import discovery, graph
normalization, or portable profile behavior.
