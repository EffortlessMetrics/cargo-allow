---
id: CARGO-ALLOW-PROP-0008
kind: proposal
status: accepted
owner: repo-infra
created: 2026-06-19
linked_specs:
  - CARGO-ALLOW-SPEC-0008
support_tier_impact: advisory
policy_impact:
  - .allow/goals/active.toml
  - .allow/artifacts/doc-artifacts.toml
---

# Proposal: Core Exception Ledger Coherence and Change Control

## Summary

cargo-allow's central product is the source-exception ledger. Public workflows
(`doctor`, `audit`, `check`, `diff`, `list`, `explain`, `worklist`, and
mutation commands) should consume one coherent domain model for identity,
scope, accountability, evidence, lifecycle, capacity, current state, movement,
posture change, and repair routing.

This proposal registers **CARGO-ALLOW-GOAL-0004** after portable governance
substrate execution closed under GOAL-0003. It sequences ledger-coherence work
without release cut, external ripr migration, or full import mode.

## Problem

Recent ratcheting and federation lanes landed useful pieces — advisory-class
registry, `occurrence_headroom`, per-lane posture, multi-ledger federation,
and structural identity — but the product still reimplements vocabulary in
CLI parsing, receipts, Markdown renderers, schemas, and worklists.

Operators also lack a durable contract for **why** policy entries changed.
Issue #1475 identifies that `reason` explains why an exception exists, not why
its selector, scope, evidence, ownership, or lifecycle changed.

Issue #1471 correctly asks for a common movement vocabulary across diff and
PR posture surfaces, but collapsing movement and posture into one four-value
enum would lose cargo-allow's improvement and review-required semantics.

## Proposed Shape

### Orthogonal movement and posture

```text
movement (presence):
  new
  resolved
  inherited

posture_delta (quality):
  improved
  worsened
  review_required
  unchanged
```

Sibling-compatible projections may still render compact `new` / `worsened` /
`resolved` / `inherited` summaries, but the canonical model keeps both fields.

### Coherent exception record

Every retained exception should expose one model across read and mutation
surfaces:

```text
Identity        stable ID, kind, family, structural selector
Scope           path/glob, ledger, lane, effective posture
Accountability  owner, classification, reason
Evidence        typed evidence, links, evidence health
Lifecycle       created, review_after, expires, last_seen
Capacity        occurrence_limit, actual count, headroom
Current state   matched, stale, expired, review_due, drifted, debt, invalid
PR movement     movement + posture_delta (orthogonal)
Change control  revision notes for governed edits
Repair route    exact next action and proof command
```

### Crate boundaries

| Crate | Owns |
| --- | --- |
| `allow-core` | Canonical IDs, movement, posture, lifecycle, and health enums |
| `allow-policy` | Ledger parsing, validation, revision/change-note contract |
| `allow-match` | Finding-to-entry evaluation, counts, drift, headroom |
| `allow-diff` | Before/after movement and policy-change classification |
| `allow-report` | Shared artifact views and rendering |
| `cargo-allow` | CLI orchestration only |

Business vocabulary must not be separately reimplemented across CLI, receipts,
Markdown, schemas, and worklists. The advisory-class registry and
`occurrence_headroom` implementation are the reference pattern.

### Policy revision contract

Use `.allow/revisions/` for durable change records that explain governed edits.
Design the contract before enforcement: which changes require a note, how one
note covers multiple entries, how notes match diffs, and whether notes are
append-only or expire after merge.

## Non-Goals

- External ripr migration or R0 preflight execution.
- Full import mode product behavior (#1466).
- Version bump, `0.1.10` release cut, or OIDC publish lanes.
- Kiro/Spec Kit expansion, CI/LLM gate redesign (#1477), or broad interop
  documentation (#1476).
- New scanner families.

## Success Criteria

- One canonical ledger-state model drives diff, receipts, worklist, and PR
  summaries without user-visible semantic drift in PR 1–2.
- Governed policy edits require matching revision notes before merge (PR 4).
- Mutation commands share a provenance envelope (PR 5).
- Read surfaces agree on status, posture, movement, and repair routing (PR 6).
- A lifecycle scenario corpus guards semantic consistency (PR 7).
- Dogfood change-control loop proves the contract on this repository (PR 8).

## Claim Boundary

This proposal registers governance artifacts and sequences implementation.
It does not implement domain types, diff behavior, revision enforcement, or
release readiness.
