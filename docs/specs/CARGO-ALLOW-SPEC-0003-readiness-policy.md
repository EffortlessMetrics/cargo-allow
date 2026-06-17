---
id: CARGO-ALLOW-SPEC-0003
kind: spec
status: accepted
owner: repo-infra
created: 2026-06-17
linked_proposal: CARGO-ALLOW-PROP-0003
linked_adrs: []
support_tier_impact: advisory
policy_impact:
  - policy/doc-artifacts.toml
---

# Spec: Self-Hosting Readiness Policy

## Summary

This spec defines **strict readiness** and **provider-tracked readiness** for
cargo-allow self-hosting and external adoption claims. The repository accepts
**provider-tracked readiness** for continuing `0.1.10` adoption-trust work and
`0.2.0` migration parity while provider gaps remain filed upstream.

## Behavior Contract

The readiness record must:

- name which readiness definition applies to each claim;
- separate local cargo-allow proof from external provider proof;
- list provider-tracked blockers with upstream filing references;
- state what blocks `0.1.10` versus external migration at scale;
- preserve the source-tree/no-execution claim boundary.

The readiness record must not:

- call provider-tracked posture zero-gap readiness;
- claim external migration readiness while `ripr+` or `unsafe-review+` provider
  blockers remain under strict definitions;
- execute ripr, unsafe-review, or other proof providers as part of
  cargo-allow's own scan.

## Readiness Definitions

### Strict readiness

All of the following must pass:

```text
docs gate: passed
workspace fmt/clippy/tests: passed
default cargo-allow no-new: passed
spec-system profile: passed
ripr+: 0 actionable gaps
unsafe-review+: 0 actionable gaps
```

Strict readiness is required for **external repository migration at scale** and
for claiming zero-gap self-hosting readiness.

### Provider-tracked readiness

All local cargo-allow surfaces pass and remaining external gaps are filed
upstream with durable references:

```text
local cargo-allow actionable gaps = 0
ripr+ blockers: provider-tracked, filed upstream
unsafe-review+ blockers: provider-tracked, filed upstream
```

Provider-tracked readiness is **not** zero-gap readiness. It allows internal
`0.1.10` adoption-trust work and `0.2.0` migration parity to proceed while
preserving the external-readiness caveat.

## Recorded Decision (2026-06-17)

**Accepted posture:** provider-tracked readiness for the `0.1.10` path.

| Blocker | Count | Tracking |
| --- | ---: | --- |
| `ripr+` predicate_boundary | 6 | `ripr#1432`, `#1433`, `#1440`–`#1443` (`EffortlessMetrics/ripr-swarm#1303`) |
| `ripr+` error_variant bulk | 358 | `EffortlessMetrics/ripr-swarm#1304` (oracle-linking friction) |
| `unsafe-review+` badge gaps | 33 | `EffortlessMetrics/unsafe-review#541` |

Local cargo-allow actionable gaps remain **0**.

## What Blocks What

| Work | Strict required? | Provider-tracked sufficient? |
| --- | --- | --- |
| `0.1.10` adoption-trust patch | no | yes |
| `0.2.0` migration parity lanes | no | yes |
| spec-system / `.allow` design docs | no | yes |
| AST identity hardening | no | yes |
| external repo migration at scale | yes | no |
| ripr external adoption handoff | yes | no unless explicitly re-approved |

## Inputs

| Input | Required | Notes |
| --- | --- | --- |
| `docs/readiness/self-hosting.md` | yes | Primary readiness record |
| Provider receipts | when cited | ripr/unsafe-review outputs are external evidence |
| Local no-new receipt | yes | `cargo-allow check --mode no-new` |

## Outputs

| Output | Required | Notes |
| --- | --- | --- |
| Readiness classification | yes | strict, provider-tracked, or not ready |
| Blocker inventory | yes | Counts and upstream filing references |
| Claim boundary | yes | What the posture does and does not prove |

## Proof Commands

| Command | Establishes | Does not establish |
| --- | --- | --- |
| `cargo-allow check --mode no-new` | Local source-exception readiness | ripr+/unsafe-review+ zero |
| `cargo-allow check --profile spec-system --mode audit` | Structural graph readiness | Provider readiness |
| `ripr check --format repo-badge-plus-json` | External ripr+ posture | cargo-allow scan correctness |
| `unsafe-review badges` | External unsafe-review+ posture | Unsafe soundness proof |

## Linked Artifacts

- Proposal:
  [CARGO-ALLOW-PROP-0003](../proposals/CARGO-ALLOW-PROP-0003-readiness-policy.md)
- Implementation plan:
  [plans/readiness/implementation-plan.md](../../plans/readiness/implementation-plan.md)
- Primary record:
  [docs/readiness/self-hosting.md](../readiness/self-hosting.md)

## Claim Boundary

This spec governs readiness language and adoption claims. It does not execute
proof providers, fix upstream provider gaps, or prove semantic correctness,
unsafe soundness, test adequacy, or coverage.
