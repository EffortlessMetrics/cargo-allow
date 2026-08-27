---
id: CARGO-ALLOW-ADR-0005
kind: adr
status: accepted
owner: repo-infra
created: 2026-08-27
linked_spec: CARGO-ALLOW-SPEC-0012
supersedes: none
superseded_by: none
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# ADR: Temporary Baseline-Debt Lifecycle

## Context

Adoption often starts with existing exceptions that have not yet received
owners, reasons, or evidence. Refusing to represent that starting state makes
adoption impractical; treating it as a reviewed policy makes the gate lie.
cargo-allow therefore needs a temporary representation that remains visible,
bounded, and reviewable.

## Decision

Generated adoption entries use the explicit `baseline_debt` classification,
`owner = "unowned"`, and a reason stating that human review is required.
Baseline debt is an adoption state, not a clean final state and not equivalent
to a reviewed exception.

Every baseline-debt entry has a short expiry: `expires` must be no more than
120 days after `created`. When `created` is absent, validation uses the tool's
deterministic fixture date. The validator rejects invalid dates and does not
auto-extend expiry. Extending or reclassifying an entry is a visible policy
change that must pass normal review.

Proposal output remains machine-readable TOML, while summaries and routing
signals are emitted separately. Generated unsafe baseline entries remain in
the review queue until they receive real evidence and human approval.

## Consequences

### Positive

- Repositories can adopt cargo-allow without hiding existing debt.
- Temporary exceptions have an explicit owner gap and end date.
- Expired debt creates pressure to review, remove, or re-approve the entry.
- Automation can route baseline work without treating it as proof.

### Negative

- Adoption produces visible debt and follow-up work.
- Deterministic fixture dates and lifecycle validation must remain aligned.
- A repository that wants to retain an exception must convert it to reviewed
  policy rather than repeatedly extending temporary debt.

## Non-Goals

- Automatically deciding that an existing exception is acceptable.
- Turning a generated baseline into evidence of safety or correctness.
- Choosing a universal review process for downstream repositories.

## Claim Boundary

This ADR records how temporary adoption debt is represented and bounded. It
does not prove that any baseline entry is safe, necessary, current, or
adequately tested.

## Rollback Or Supersession

Supersede this ADR only with a replacement that preserves an explicit,
machine-checkable distinction between temporary adoption debt and reviewed
policy, or explains why adoption no longer needs a temporary state.
