---
id: CARGO-ALLOW-PLAN-0009
kind: implementation_plan
status: done
owner: repo-infra
created: 2026-06-19
linked_proposal: CARGO-ALLOW-PROP-0008
linked_spec: CARGO-ALLOW-SPEC-0008
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
support_tier_impact: advisory
policy_impact:
  - .allow/goals/active.toml
  - .allow/artifacts/doc-artifacts.toml
  - policy/allow.toml
---

# Implementation Plan: Core Exception Ledger Coherence and Change Control

## Purpose

Sequence PR-sized slices that make cargo-allow's exception-ledger workflows
behave like one system: canonical domain vocabulary, orthogonal movement and
posture delta, durable policy revision records, shared mutation receipts,
converged read surfaces, and a lifecycle scenario corpus.

## Linked Artifacts

- Proposal:
  [CARGO-ALLOW-PROP-0008](../../docs/proposals/CARGO-ALLOW-PROP-0008-ledger-coherence-change-control.md)
- Spec:
  [CARGO-ALLOW-SPEC-0008](../../docs/specs/CARGO-ALLOW-SPEC-0008-ledger-coherence-change-control.md)
- Support-tier surface: [CARGO-ALLOW-SUPPORT-0001](../../docs/status/SUPPORT_TIERS.md)
- Historical campaign tracker: [archived GOAL-0004](../../.allow/goals/archive/CARGO-ALLOW-GOAL-0004-core-exception-ledger.toml).

## Non-Goals

- External ripr migration or R0 preflight execution.
- Full import mode product behavior (#1466).
- Version bump, `0.1.10` release cut, or OIDC publish lanes.
- Kiro/Spec Kit expansion, CI/LLM gate redesign (#1477), interop docs (#1476).
- New scanner families.

## Claim Boundary

This plan sequences ledger-coherence work. Its bounded campaign is complete as
recorded by `CARGO-ALLOW-CLOSEOUT-0053`. It does not prove release readiness,
unsafe soundness, test adequacy, or coverage beyond the stated proofs, and it
does not grant the historical active-goal file current authority.

## Validation Baseline

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md`
- `cargo run -p cargo-allow -- check --profile spec-system --mode audit --format json --output target/cargo-allow/spec-system.json`

## PR Sequence

### PR 0: Register GOAL-0004 and reconcile issues

Purpose: governance-only registration of proposal, spec, plan, active goal,
doc-artifacts, closeout stub, and stale issue reconciliation.

Non-goals: behavior change, domain types, release authorization.

Files:

- `docs/proposals/CARGO-ALLOW-PROP-0008-ledger-coherence-change-control.md`
- `docs/specs/CARGO-ALLOW-SPEC-0008-ledger-coherence-change-control.md`
- `plans/ledger-coherence/implementation-plan.md`
- `plans/ledger-coherence/closeouts/goal-0004-registration.md`
- `.allow/goals/active.toml`
- `.allow/artifacts/doc-artifacts.toml`
- `policy/allow.toml`
- `CHANGELOG.md`

Validation:

- `cargo run -p cargo-allow -- check --mode no-new`
- `cargo run -p cargo-allow -- check --profile spec-system --mode audit`
- `cargo test -p allow-policy spec_system::tests::parses_current_repository_active_goal_manifest`

Claim boundary: governance registration and issue reconciliation only.

Rollback: revert governance artifacts; restore GOAL-0003 done stub in
`active.toml`.

### PR 1: Canonical ledger state model

Purpose: add canonical domain types (`PresenceMovement`, `PostureDelta`,
`LedgerPosture`) in `allow-core`; inventory and remove duplicated string
mappings across diff rows, receipt schemas, Markdown/human output, worklist
routing, PR summaries, and JSON artifacts.

Non-goals: user-visible semantic change.

Files: `crates/allow-core/`, `crates/allow-report/`, schema/docs as needed.

Validation: targeted unit tests; no-new guard; characterization tests proving
string parity with existing outputs.

Claim boundary: internal vocabulary consolidation only.

Rollback: revert type additions and mapping refactors.

### PR 2: Movement classification in `diff` (implements #1471)

Purpose: every diff row carries `movement`, `posture_delta`, `changed_in_diff`,
`allow_id`, `ledger_id`, `lane`; emit dual summary counts; project into human,
Markdown, JSON, receipt, and worklist without removing detailed policy-change
reasons.

Non-goals: revision-note enforcement.

Validation: `allow-diff` tests, saved artifact output tests, no-new guard.

Claim boundary: diff and PR-posture vocabulary alignment; does not enforce change
notes.

Rollback: revert diff classification changes.

### PR 3: Design policy revision contract (#1475 design slice)

Purpose: accept `.allow/revisions/` schema and matching rules; document which
changes require notes, multi-entry coverage, diff matching, expiry, and
append-only posture.

Non-goals: CLI enforcement.

Validation: spec-system audit; fixture-backed parse/validate tests once schema
lands.

Claim boundary: design contract only.

Rollback: withdraw revision schema docs and parse stubs.

### PR 4: Enforce change notes in `diff`

Purpose: `diff --require-change-note` and
`--write-change-note-template .allow/revisions/next.toml`; require notes for
governed weakening edits; exempt obvious improvements.

Non-goals: automatic approval or silent note generation.

Validation: fixture-backed accept/reject tests; dogfood prep only.

Claim boundary: enforcement on diff path; does not mutate policy automatically.

Rollback: remove enforcement flags.

### PR 5: Unify mutation receipts

Purpose: shared operation envelope for `add`, `propose`, `refresh`, `prune`,
`migrate`; split 5A–5D if needed.

Non-goals: changing mutation semantics beyond provenance alignment.

Validation: receipt schema tests per command.

Claim boundary: provenance envelope only.

Rollback: revert envelope consolidation per sub-slice.

### PR 6: Converge read surfaces

Purpose: shared view model for `list`, `explain`, `worklist`, `audit`, `check`,
`diff` with agreed status, posture, movement, headroom, and repair routing.

Non-goals: new commands.

Validation: cross-surface fixture corpus (initial subset).

Claim boundary: vocabulary alignment across read paths.

Rollback: revert view-model wiring per surface.

### PR 7: Ledger lifecycle scenario corpus

Purpose: compact fixture corpus covering matched, stale, expired, review_due,
drift, headroom, evidence debt, baseline_debt, invalid/ambiguous, mirror
divergence, weakening, improvement, and review-required states; run through all
major commands with semantic-consistency oracles.

Non-goals: exact every-line goldens.

Validation: corpus-driven integration tests.

Claim boundary: regression lens for product semantics.

Rollback: remove corpus and tests.

### PR 8: Dogfood policy change control

Purpose: fixture-only selector change on `policy/allow.toml`; prove diff
classification, missing revision note reporting, note addition, and reviewable
posture with committed receipt and closeout.

Non-goals: production policy weakening.

Validation: dogfood receipt; spec-system audit.

Claim boundary: in-repo control-loop proof.

Rollback: revert dogfood fixture and closeout.

### PR 9: Operator documentation

Purpose: add `docs/how-to/manage-an-exception.md` covering discover, add,
explain, review, refresh drift, reduce headroom, repair evidence, record policy
changes, prune stale entries, and read PR posture.

Non-goals: interop or CI architecture docs.

Validation: docs link check; spec-system audit.

Claim boundary: operator guide only.

Rollback: remove guide.

## Support-Tier Updates

No support-tier promotion in this lane until PR 7–8 dogfood evidence exists.
Review `CARGO-ALLOW-SUPPORT-0001` after PR 8 if claim boundaries change.

## Policy Updates

- PR 0 registers governed doc artifacts in `policy/allow.toml`.
- PR 3–4 may add `.allow/revisions/` examples and enforcement fixtures.
- PR 8 may add dogfood revision fixtures under `tests/fixtures/` or
  `docs/dogfood/`.

## Closeout Requirements

- PR 0 closeout:
  [goal-0004-registration.md](closeouts/goal-0004-registration.md)
- Final lane closeout after PR 9 records landed slices, proof commands, and
  deferred ripr/full-import follow-ups: `CARGO-ALLOW-CLOSEOUT-0053`.

## Rollback Path

Revert governance registration (PR 0) to restore GOAL-0003 done stub. Revert
implementation PRs independently; keep spec/plan artifacts updated if behavior
is withdrawn.

## Deferred Follow-Ups (Outside This Plan)

| Item | Blocker |
| --- | --- |
| External ripr preflight R0 | explicit adoption request |
| External ripr migration | explicit adoption request |
| Full import mode (#1466) | external adoption need |
| Interop docs (#1476) | separate lane |
| CI/LLM gate redesign (#1477) | separate lane |
