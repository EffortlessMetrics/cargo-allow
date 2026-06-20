---
id: CARGO-ALLOW-CLOSEOUT-0022
kind: closeout
status: done
owner: repo-infra
created: 2026-06-20
linked_plan: CARGO-ALLOW-PLAN-0009
linked_proposal: CARGO-ALLOW-PROP-0008
linked_spec: CARGO-ALLOW-SPEC-0008
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0004
support_tier_impact: advisory
policy_impact:
  - .allow/artifacts/doc-artifacts.toml
  - .allow/revisions/
---

# Closeout: GOAL-0004 PR 3 — Policy Revision Contract Design

## Summary

Design-only slice for #1475. Records the `.allow/revisions/` change-control
contract in CARGO-ALLOW-ADR-0002. Records reuse the canonical `policy_change_kind`
vocabulary, and the note requirement is the matched diff row's `posture_delta`
(`worsened`) from PR 1/2 — no parallel taxonomy. No runtime parsing or
enforcement; PR 4 implements `diff --require-change-note`.

## Landed

- `docs/adr/CARGO-ALLOW-ADR-0002-policy-revision-contract.md` — revision record
  schema, `change_kinds` reusing the canonical `policy_change_kind` vocabulary,
  the note rule keyed to the row's `posture_delta`, multi-entry coverage,
  identity-based diff matching, durable append-only posture, and ledger placement.
- `.allow/revisions/README.md` — directory governance, record format, and claim
  boundary; `.allow/revisions/examples/CARGO-ALLOW-REV-0001-example.toml`
  illustrating one worsened-edit record.
- SPEC-0008 updated: `linked_adrs` references ADR-0002 and the Policy Revision
  Contract section records the five design decisions.
- Registered ADR-0002 and this closeout in `.allow/artifacts/doc-artifacts.toml`;
  added the SPEC-0008 → ADR-0002 back-link.

## Design Decisions (ADR-0002)

| Question | Decision |
| --- | --- |
| Which edits require a note | Those classified `posture_delta = worsened`; improvements/neutral exempt; `review_required` edits need a note or reviewer ack (surface in PR 4) |
| Multi-entry coverage | `allow_ids` + `change_kinds` arrays; worsened row satisfied by union of matching records |
| Diff matching | By stable `allow_id` and change kind, never by file line; uncovered records inert |
| Expiry after merge | No — durable; `review_after` advisory only |
| Append-only | Yes — immutable, corrected via `supersedes` |

## Validation

| Check | Result |
| --- | --- |
| `cargo-allow check --profile spec-system --mode audit` | pass |
| `cargo-allow check --mode no-new` | pass |
| `cargo test -p allow-policy spec_system` | pass |

## Remaining

- **Ready:** `ledger-coherence-pr4-enforce-change-notes` (unblocked by this
  contract acceptance).
- Record parsing, JSON schema, federation-ledger registration, and matching
  enforcement remain PR 4.

## Claim Boundary

Design contract only. Does not parse or enforce revision records, classify real
diffs, unify mutation receipts, or authorize a release cut.
