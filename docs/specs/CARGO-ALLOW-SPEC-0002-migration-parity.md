---
id: CARGO-ALLOW-SPEC-0002
kind: spec
status: accepted
owner: repo-infra
created: 2026-06-16
linked_proposal: CARGO-ALLOW-PROP-0002
linked_adrs: []
support_tier_impact: advisory
policy_impact:
  - policy/doc-artifacts.toml
  - policy/allow.toml
---

# Spec: Migration and Evidence Parity

## Summary

The migration parity lane governs how cargo-allow moves repositories from bespoke
xtask allowlists to durable `policy/allow.toml` receipts with documented,
reviewable deltas. Success means a maintainer can run side-by-side proof, classify
remaining gaps honestly, and retire an xtask only when documented deltas are
acceptable.

## Behavior Contract

The lane must:

- keep compat modes as explicit bridges, not silent canonical policy shape;
- preserve existing IDs, owners, reasons, and evidence when practical;
- document every known delta between xtask and cargo-allow findings;
- emit migration summaries and closeout queues that agents can execute without
  chat memory;
- fail closed on undocumented broadening or suppressed findings.

The lane must not:

- claim macro-expanded, type-aware, MIR-level, or build-aware parity;
- auto-approve `baseline_debt` or launder temporary debt into durable approval;
- execute repository code, Cargo, rustc, ripr, unsafe-review, or network checks
  as part of cargo-allow's own scan.

## Inputs

| Input | Required | Notes |
| --- | --- | --- |
| Legacy policy file | yes | Per compat kind documented in migration guides |
| Side-by-side cargo-allow command | yes | Closest `check --compat` or migrate flow |
| Migration summary artifact | when saved | `cargo-allow.migrate.v1` when migration writer runs |
| Active goal work item | yes | Bounded repair scope for agents |

## Outputs

| Output | Required | Notes |
| --- | --- | --- |
| Documented delta classification | yes | same, stricter, weaker, stale |
| Receipt or summary evidence | yes | Human- or agent-reviewable |
| Closeout queue items | when debt remains | Owner, reason, review, expiry for retained debt |

## Accepted States

- Side-by-side proof shows only documented, acceptable deltas for a lane.
- Migration summary closeout queues are empty or owned with review dates.
- Active goal work items map to proof commands and claim boundaries.

## Rejected States

- Undocumented finding suppression to force parity.
- Compat output treated as final policy without deliberate migration.
- Release or support-tier claims ahead of parity evidence.

## Proof Commands

| Command | Establishes | Does not establish |
| --- | --- | --- |
| `cargo-allow check --compat --kind <kind>` | Side-by-side findings for one compat lane | Full xtask retirement for all lanes |
| `cargo-allow migrate ...` | Saved migration summary and closeout queues | Semantic correctness of migrated policy |
| `cargo-allow check --mode no-new` | No new unreceipted source-tree findings | Migration parity itself |

## Linked Artifacts

- Proposal:
  [CARGO-ALLOW-PROP-0002](../proposals/CARGO-ALLOW-PROP-0002-migration-parity.md)
- Implementation plan:
  [plans/migration-parity/implementation-plan.md](../../plans/migration-parity/implementation-plan.md)
- Active goal:
  [CARGO-ALLOW-GOAL-0002](../../.allow/goals/active.toml)
- Migration guide:
  [docs/migration-from-xtask.md](../migration-from-xtask.md)
