---
name: cargo-allow-0.2-campaign
description: Orchestrate and execute reversible implementation issues for the cargo-allow 0.2.0 campaign (#3768). Use to select next unblocked issues, classify session lanes, implement bounded changes, run validation, open/update PRs, hand off for current-head review, and reconcile merged state. Do not use as the primary substantive PR review skill or for unauthorized release operations.
---

# Cargo-Allow 0.2 Campaign Orchestrator

## Trigger

Activate this skill when the task is to:

- orchestrate or execute work in the cargo-allow `0.2.0` campaign (#3768);
- select the next unblocked issue or active PR in the campaign train;
- implement or carry forward one bounded reversible issue;
- prepare a scoped implementation packet, run validation, and open/update a PR;
- hand off a finished author head to `review-current-head`;
- reconcile a merged child issue on `main` and route to the next unblocked lane.

Do not activate as the primary substantive PR review skill. Hand substantive PR review to `review-current-head` after the author head is final.

## Required Live Inputs

At session start, reload:

- `GEMINI.md`, `AGENTS.md`, and `CLAUDE.md`;
- `#3768` campaign controller and `docs/campaigns/cargo-allow-0.2.0.md`;
- selected child issue and predecessor issue acceptance states;
- open PRs, active branches, and their current base/head/check/review state;
- current `main` commit/tree and recent relevant merges;
- current external release/registry state when the issue depends on it;
- local branch, status, diff, and dirty worktrees when in a checkout.

Conversation memory, a prior issue summary, and a task-run narrative are not current state.

## Session Lane Classification

Before acting, classify the selected work as exactly one lane:

```text
ReversibleImplementation
ReadOnlyReview
ExternalObservation
RootDecision
IrreversibleOperation
BlockedOrStale
```

### ReversibleImplementation

May edit source, tests, docs, schemas, workflows, fixtures, and repository policy in a scoped branch; run validation; push; open/update a PR; hand off for current-head review; merge only after a fresh non-blocking current-pair review and terminal required checks; then verify `main` and update issues.

### ReadOnlyReview

Activate `review-current-head`. Do not mutate the branch while claiming independent review. If repairs are needed, return a bounded repair packet to one writer.

### ExternalObservation

Query GitHub Actions, releases, crates.io, docs.rs, checksums, branch/ruleset state, and retained artifacts. Write only observation/reconciliation source through a normal PR when the controlling issue authorizes that reversible record. Observation never authorizes mutation.

### RootDecision

Prepare the narrowest decision packet with recommendation, deciding criteria, consequences, and what would change the call. Stop and request human operator decision. Examples: selecting pilot repositories (#3771), choosing whether `rc.2` is required, accepting a supported limitation, and final release authorization (#3760).

### IrreversibleOperation

Stop unless the user has explicitly authorized the exact operation and controlling typed authorization/evidence exists. Prohibited autonomous actions: tag create/move/delete, crates.io publish/yank, GitHub Release publish/replace, live repository setting changes, and external target-repository mutation not already authorized.

### BlockedOrStale

Update or comment on the child issue with the exact blocker or stale premise and route to the current owner. Do not implement an obsolete issue body merely because it is labeled agent-ready.

## Issue Selection Algorithm

1. Start at controller #3768 (`docs/campaigns/cargo-allow-0.2.0.md`); do not use open issue count as a priority signal.
2. Select an issue explicitly listed in the active campaign rail whose predecessors are complete.
3. Check for an active PR or recent writer on that exact semantic owner.
4. If an active viable PR exists, coordinate or review rather than forking.
5. If a lane is stale, inspect its exact branch/head/comments/checks before deciding to continue or replace it.
6. Never open two implementation PRs for one semantic authority.
7. Prefer the smallest issue whose completion unblocks multiple later rails.
8. Keep experimental siblings (`cargo-intent`, `cargo-proof`) optional; do not import them as `cargo-allow` release blockers.

## Implementation Packet

Before making broad edits, retain in the issue or PR:

```text
controller and child issue
live main/base/head identity
purpose and exact owned seam
predecessor evidence consumed
scope and non-goals
highest-risk invariant / false-green route
required negative controls
expected changed files/consumers
validation plan
external/irreversible actions = none
claim boundary
```

## PR Lifecycle and Merge Rules

1. Start from current `main`.
2. Implement scoped changes on a bounded feature branch.
3. Run narrow tests first, then affected/full proof.
4. Run the default source-tree guard:
   ```bash
   cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md
   ```
5. Open or update PR using `.github/PULL_REQUEST_TEMPLATE.md` with exact head SHA.
6. Hand off to `review-current-head` for independent review.
7. If repairs are made, push repair and re-review affected dimensions.
8. Merge only when live PR head equals reviewed head, PR is non-draft/mergeable, substantive review threads are resolved, and all required CI checks are terminal green.
9. Synchronize `main`, rerun guards, and clean scratch/disposable worktrees.
10. Update child issue and #3768.

## Release Immutability Law

- Never delete, move, or recreate `v0.2.0-rc.1`.
- Never publish another `0.2.0-rc.1` package row.
- Never treat a tag push or latest green CI run as release authorization.
- Never continue a partial release from moving `main`.
- Never create `v0.2.0`, access publication tokens, or publish releases from an ordinary implementation session.
- After #2501 final candidate refreeze becomes Complete, hard STOP for separate explicit #3760 release authorization.
- Any repair after an externally observed release tag requires a new version candidate under a fresh freeze and authorization.

## Evidence and Post-Merge Reporting

After each merged child, report on the child and #3768:

```text
merged PR and commit/tree
owned acceptance rows satisfied
commands/checks and results
retained receipt/artifact identities
remaining limitations
external/irreversible actions performed = none
next unblocked issue or RootDecision
claim boundary
```

Do not claim a publish, release, install, review, or provider result without direct evidence.

## Claim Boundary

A shared Agent Skills implementation router for the cargo-allow final-0.2 campaign. It enables agents to select, implement, hand off, merge, and reconcile reversible issue work under checked boundaries; it does not authorize external or irreversible release operations.
