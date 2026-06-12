---
id: CARGO-ALLOW-PROP-0001
kind: proposal
status: accepted
owner: repo-infra
created: 2026-06-12
linked_specs:
  - CARGO-ALLOW-SPEC-0001
support_tier_impact: advisory
policy_impact:
  - policy/doc-artifacts.toml
  - policy/spec-system.toml
  - policy/allow.toml
---

# Proposal: Spec-System Profile

## Summary

cargo-allow should grow an opt-in `spec-system` profile that governs retained
source-of-truth structure the same way default cargo-allow governs retained
source exceptions.

Default cargo-allow stays small: source-tree exception audit, check, diff,
explain, list, and worklist commands remain the first useful run. The
`spec-system` profile is explicit behavior for repositories that want governed
proposal, spec, ADR, implementation-plan, active-goal, support-tier, policy, and
closeout links.

## Problem

cargo-allow already answers durable source-exception questions:

- what retained exceptions exist.
- why they are allowed.
- who owns them.
- what evidence is linked.
- what a human or agent should fix next.

The same drift problem exists one level up in repo governance. Proposals, specs,
ADRs, implementation plans, active goals, support tiers, policy ledgers, PRs,
proof commands, and closeouts can diverge even when each document is locally
reasonable. When that happens, maintainers and agents have to reconstruct why a
lane exists, what behavior is actually required, which proof commands matter,
and what work remains.

That reconstruction should not depend on chat history or ad hoc memory. It
should come from a source-tree graph that cargo-allow can scan structurally and
turn into reports, receipts, and repair worklists.

## Users And Surfaces

- Maintainers: need a durable way to see why governance work exists and which
  artifacts own which claims.
- Reviewers: need PRs to name claim boundaries, proof commands, and rollback
  paths without mixing proposal, spec, and plan responsibilities.
- Agent operators: need repo-native work items that can be executed without
  relying on chat history.
- Product surface: future explicit `cargo-allow <command> --profile
  spec-system` commands.
- Repo surface: `docs/proposals/`, `docs/specs/`, `docs/adr/`, `plans/`,
  `.codex/goals/`, `docs/status/SUPPORT_TIERS.md`, `policy/doc-artifacts.toml`,
  and `policy/spec-system.toml`.

## User Value

The profile should make the source-of-truth stack inspectable and repairable:

- a new contributor can find why work exists.
- maintainers can see which artifacts are authoritative for a claim.
- reviewers can detect broken links and missing proof-command references.
- agents can receive bounded repair items instead of broad prose tasks.
- CI can eventually upload structural receipts for source-of-truth drift.

The value is governed structure, not more documentation for its own sake.

## Proposed Shape

Add a profile that is disabled unless selected explicitly:

```bash
cargo-allow check --profile spec-system
cargo-allow audit --profile spec-system
cargo-allow worklist --profile spec-system --format json
cargo-allow doctor --profile spec-system
```

Later, once the graph model is stable:

```bash
cargo-allow init --profile spec-system
cargo-allow explain CARGO-ALLOW-SPEC-0001 --profile spec-system
```

The profile should structurally lint source-tree artifacts:

- artifact registry parsing.
- unique artifact IDs.
- artifact paths and expected roots.
- visible IDs in files.
- recognized statuses and owners.
- proposal, spec, ADR, plan, active-goal, support-tier, policy, and closeout
  links.
- support-tier proof-command fields.
- active-goal work item fields.
- closeout links for completed work.

It should emit the same kinds of operator surfaces cargo-allow already uses:

- human and Markdown reports.
- JSON artifacts and receipts.
- agent-safe worklist items.
- setup readiness from `doctor`.

## Why cargo-allow Hosts It

cargo-allow is already the source-tree governance scanner for retained
exceptions. The spec-system profile is the same structural pattern applied to
retained repo governance artifacts.

This should live in cargo-allow because:

- it reads source-tree files and policy ledgers.
- it fails or reports based on durable source state.
- it emits bounded repair worklists.
- it preserves claim boundaries instead of proving external claims.

It should not become a generic Markdown linter. It should stay a governance
graph scanner.

## Alternatives Considered

| Alternative | Reason not chosen |
| --- | --- |
| Keep source-of-truth checks in bespoke xtasks | Repeats one-off graph rules across repos and prevents a shared worklist and receipt contract. |
| Make spec-system checks part of default `cargo-allow check` | Makes the first useful run too heavy and violates cargo-allow's small adoption spine. |
| Build a separate public crate immediately | Premature API boundary; the profile should prove itself inside existing cargo-allow crates first. |
| Use a generic Markdown linter | Misses the actual product need: artifact identity, links, statuses, proof-command references, and repair routing. |
| Use chat history or generated notes as source of truth | Not durable, reviewable, or scannable from the repository source tree. |

## Success Criteria

- Default `cargo-allow check`, `audit`, and `diff` behavior remains focused on
  source exceptions unless a profile is selected.
- The repo can register governed source-of-truth artifacts in a machine-readable
  ledger.
- `cargo-allow check --profile spec-system` can validate safe structural graph
  rules without executing proof commands.
- `cargo-allow audit --profile spec-system` can explain current source-of-truth
  posture.
- `cargo-allow worklist --profile spec-system --format json` can emit bounded
  repair items for missing artifacts, broken links, missing proof-command
  fields, and closeout gaps.
- `cargo-allow doctor --profile spec-system` can report setup readiness.
- Later maturity commands can bootstrap the profile with `cargo-allow init
  --profile spec-system` and explain one artifact with `cargo-allow explain
  <artifact-id> --profile spec-system`.
- Dogfood starts advisory, can later move to shadow, and blocks only safe
  structural lints after burn-in.

## Specs To Create

- `CARGO-ALLOW-SPEC-0001`: spec-system profile behavior, config, artifact
  ledger, lint outputs, worklist items, and source-tree claim boundary.

## Support-Tier Impact

The profile should add an advisory support-tier surface for source-of-truth
graph linting. Support tiers should own the user-facing claim to proof-command
mapping. Specs should link to that surface instead of duplicating the claim map.

## Policy Impact

The proposal expects later policy artifacts:

- `policy/doc-artifacts.toml` for the governed artifact registry.
- `policy/spec-system.toml` for profile roots and requirements.
- `policy/allow.toml` entries for any new tracked documentation, TOML, schema,
  or config files introduced while dogfooding the stack.

No policy behavior changes in this proposal by itself.

## Required Evidence

Before the profile can be considered implemented, later PRs should provide:

- parsing tests for profile config and doc artifact ledgers.
- validation tests for duplicate IDs, missing files, invalid statuses, and
  unknown links.
- report or receipt tests that show source-tree-only claim boundaries.
- worklist tests for agent-safe repair items.
- CLI tests showing default behavior is unchanged without `--profile
  spec-system`.
- dogfood evidence from cargo-allow's own source-of-truth stack.

## Non-Goals

- Do not make spec-system linting part of default cargo-allow behavior.
- Do not execute proof commands from the spec-system scanner.
- Do not call GitHub APIs or network services during cargo-allow's own scan.
- Do not invoke Cargo, rustc, Clippy, build scripts, proc macros, ripr,
  unsafe-review, or coverage tooling from the scanner.
- Do not claim semantic correctness, release readiness, unsafe soundness, test
  adequacy, or coverage from structural graph linting.
- Do not duplicate support-tier truth inside specs.
- Do not put full PR queues inside proposals or specs.
- Do not create a new public crate before the profile model is stable.

## Claim Boundary

This proposal records the product direction for an opt-in source-of-truth graph
profile. It does not implement the profile, validate any graph, execute any
proof command, or prove that any future support-tier claim is true.

Any future `spec-system` report must stay inside structural source-tree claims:
files parsed, IDs found, links resolved, statuses recognized, required fields
present, and work items emitted.

## Risks

| Risk | Mitigation |
| --- | --- |
| The profile makes cargo-allow feel heavy. | Keep it opt-in and preserve the current first-run source-exception path. |
| The profile becomes a generic docs linter. | Limit checks to governed artifact identity, links, statuses, proof-command fields, and source-tree policy state. |
| Structural linting is mistaken for proof execution. | Emit explicit claim boundaries and scanner limitations in reports and receipts. |
| Support-tier claims drift into specs. | Keep support tiers as the claim-to-proof-command map and have specs link to them. |
| Agents receive broad or unsafe tasks. | Use worklist items with artifact IDs, paths, owners, suggested actions, and proof commands. |

## Rollback Or Withdrawal

If the profile direction is withdrawn, remove or supersede this proposal and any
linked spec, plan, policy, support-tier, and active-goal artifacts that depend
on it. Default cargo-allow source-exception behavior must remain unaffected.
