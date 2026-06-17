---
id: CARGO-ALLOW-PROP-0003
kind: proposal
status: accepted
owner: repo-infra
created: 2026-06-17
linked_specs:
  - CARGO-ALLOW-SPEC-0003
support_tier_impact: advisory
policy_impact:
  - policy/doc-artifacts.toml
---

# Proposal: Self-Hosting Readiness Policy

## Summary

cargo-allow needs an explicit readiness policy so maintainers can distinguish
local source-tree readiness from external proof-provider readiness. Without a
recorded decision, the project oscillates between two incompatible definitions
of done.

## Problem

The self-hosting record shows clean local proof for docs, fmt/clippy/tests,
default no-new, and spec-system, while `ripr+` and `unsafe-review+` remain
blocked on provider-tracked gaps filed upstream. That state is honest but
ambiguous:

- **Strict readiness** would block all forward work until providers report zero.
- **Provider-tracked readiness** would allow internal adoption-trust and
  migration-parity work while preserving the external-readiness caveat.

## Users And Surfaces

- Maintainers deciding whether to cut `0.1.10` or start `0.2.0` migration work.
- Reviewers judging external repo adoption claims.
- Agent operators executing bounded lanes without overstating readiness.

## Proposed Shape

Record two readiness definitions and accept **provider-tracked readiness** for
the `0.1.10` adoption-trust path:

```text
local cargo-allow actionable gaps = 0
provider blockers filed upstream with artifacts
external migration at scale remains blocked
```

Do not call provider-tracked readiness zero-gap readiness.

## Non-Goals

- Do not claim `ripr+ = 0` or `unsafe-review+ = 0` while provider blockers
  remain.
- Do not execute ripr or unsafe-review from cargo-allow's own scan.
- Do not start external repo migration at scale under provider-tracked posture.

## Claim Boundary

This proposal records a governance decision. It does not fix provider gaps,
execute proof commands, or prove external adoption readiness.
