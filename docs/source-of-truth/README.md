# Source-of-Truth Stack

This directory defines the source-of-truth stack for cargo-allow's spec-system
profile.

cargo-allow's default product remains the source-tree exception ledger. It
scans repository files without executing project code, matches syntax-visible
findings against `policy/allow.toml`, and emits reports, receipts, and worklists
for retained exceptions.

The `spec-system` profile extends that same source-tree governance model one
level up: it lints the repository's proposal, spec, ADR, plan, active goal,
support-tier, policy-ledger, and closeout graph. That profile is one opt-in
governance profile and does not change default `cargo-allow check` behavior.

Current command shape:

```bash
cargo-allow check --profile spec-system
cargo-allow audit --profile spec-system
cargo-allow worklist --profile spec-system --format json
cargo-allow doctor --profile spec-system
cargo-allow init --profile spec-system
cargo-allow explain <artifact-id> --profile spec-system
```

This profile is heavier than the default source-exception ledger, so adoption
should start advisory or shadow and promote only proven structural checks after
local burn-in.

## What The Stack Is For

The stack should make source-of-truth state durable enough for maintainers,
reviewers, and agents to use without relying on chat history:

- A new contributor can find why work exists.
- Codex and other agents can find the next ready slice.
- Reviewers can see claim boundaries and proof commands.
- CI can detect broken governance links for opted-in profiles.
- Worklists can point to bounded source-of-truth repairs.

This is not a generic Markdown linting lane. The profile is about the governance
graph: known artifacts, required fields, linked IDs, artifact paths, claim
boundaries, and proof-command references.

## Artifact Roles

Each artifact owns one part of the repo's durable product story:

| Artifact | Owns |
| --- | --- |
| Proposal | Why work matters, user value, alternatives, and success criteria. |
| Spec | Required behavior, evidence surfaces, and acceptance examples. |
| ADR | Durable architecture decisions and tradeoffs. |
| Implementation plan | PR-sized sequence, proof commands, rollback, and release notes. |
| Active goal manifest | What Codex or agents should execute now. |
| Support tiers | User-facing claim to proof-command mapping. |
| Policy ledger | Exceptions, source-tree governance, and profile state. |
| Closeout | What landed, what proved it, and what remains. |

The detailed artifact contract is in
[artifact taxonomy](artifact-taxonomy.md). Link rules are in
[linking model](linking-model.md). The initial machine-readable registry is
[doc artifact ledger](doc-artifact-ledger.md). Agent use is in
[agent operating model](agent-operating-model.md).
Starter templates are in [proposal](../templates/proposal.md),
[spec](../templates/spec.md), [ADR](../templates/adr.md),
[implementation plan](../templates/implementation-plan.md),
[plan item](../templates/plan-item.md), [closeout](../templates/closeout.md),
and [PR body](../templates/pr-body.md).
The first accepted proposal for this lane is
[CARGO-ALLOW-PROP-0001](../proposals/CARGO-ALLOW-PROP-0001-spec-system-profile.md).
The first accepted spec is
[CARGO-ALLOW-SPEC-0001](../specs/CARGO-ALLOW-SPEC-0001-spec-system-profile.md).
The first support-tier map is
[CARGO-ALLOW-SUPPORT-0001](../status/SUPPORT_TIERS.md).
The current active goal manifest is an archived stub for
[CARGO-ALLOW-GOAL-0003](../../.allow/goals/active.toml) (full history in
[archive](../../.allow/goals/archive/CARGO-ALLOW-GOAL-0003-portable-governance-substrate.toml)).
The current implementation plan is
[CARGO-ALLOW-PLAN-0001](../../plans/spec-system/implementation-plan.md).
First-hour adoption guidance is in
[Adopt the spec-system profile](../how-to/adopt-spec-system-profile.md), and CI
guidance is in
[Run the spec-system profile in CI](../how-to/run-spec-system-in-ci.md).

## Claim Boundary

The profile is source-tree-only structural checking. It may parse TOML and
Markdown, verify IDs, paths, statuses, links, template fields, support-tier proof
fields, active goal references, and closeout links.

It must not execute proof commands, run tests, call GitHub APIs, run ripr,
unsafe-review, coverage, Cargo, rustc, Clippy, build scripts, or proc macros.
It must not claim semantic correctness, proof execution, test adequacy, unsafe
soundness, or release readiness.

Other tools can provide evidence. The `spec-system` profile owns only the
durable source-tree graph that records where that evidence is expected.

## Non-Goals

- Do not make source-of-truth linting part of default `cargo-allow check`.
- Do not require Cargo metadata, rustc, Clippy, build scripts, or proc macro
  expansion for source-of-truth checks.
- Do not call network services or GitHub APIs during cargo-allow's own scan.
- Do not duplicate support-tier truth inside specs.
- Do not put full PR queues inside specs.
- Do not use generated or chat-only state as durable source of truth.

## Adoption

The intended adoption ladder is:

```text
Level 0: default cargo-allow source exception ledger.
Level 1: multi-ledger and per-lane posture.
Level 2: opt-in spec-system profile.
Level 3: integrated work routing, receipts, and repair queues.
```

First-hour adoption guidance is in
[Adopt the spec-system profile](../how-to/adopt-spec-system-profile.md). CI
guidance is in
[Run the spec-system profile in CI](../how-to/run-spec-system-in-ci.md).
