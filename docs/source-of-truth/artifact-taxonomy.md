# Artifact Taxonomy

The source-of-truth stack separates artifacts by responsibility. A document can
link to other artifacts, but it should not absorb their jobs.

## Proposal

Proposals explain why work should exist.

They should cover:

- problem statement.
- affected users or surfaces.
- user value.
- alternatives considered.
- success criteria.
- risks.
- non-goals.
- specs expected to follow.

They should not contain the full PR queue or implementation checklist. That
belongs in an implementation plan.

## Spec

Specs define required behavior.

They should cover:

- behavior contract.
- inputs and outputs.
- accepted states.
- rejected states.
- evidence surfaces.
- acceptance examples.
- claim boundary.

They should not duplicate support-tier claim maps, implementation order, or
release history.

## ADR

ADRs record durable design decisions.

They should cover:

- context.
- decision.
- alternatives.
- consequences.
- replacement or supersession path.

They should not be used as mutable task trackers.

## Implementation Plan

Implementation plans sequence the work.

They should cover:

- PR-sized slices.
- purpose for each slice.
- non-goals.
- validation.
- rollback path.
- release or migration notes when relevant.

They should not redefine the behavior contract. Specs own that contract.

## Historical Goal Artifacts

Historical goal artifacts record completed or superseded execution campaigns.
They are read-only evidence, not a current agent execution surface.

It should cover:

- completed objective and claim boundary.
- linked proposal, spec, plan, and closeout.
- historical work-item statuses.
- proof commands recorded as evidence.

They are not product runtime state, release state, a current issue pointer, or
a replacement for the artifact ledger. Current work belongs to GitHub issues,
PR-local implementation slices, and disposable local session focus.

## Support Tiers

Support tiers map user-facing claims to proof commands.

They should cover:

- surface or feature.
- tier or posture.
- user-facing claim.
- proof command or evidence source.
- notes and limitations.

Specs can link to support-tier surfaces, but support tiers own the claim to
proof-command mapping.

## Policy Ledgers

Policy ledgers record governed source-tree state.

For current cargo-allow behavior, `policy/allow.toml` owns retained source
exceptions. For the opt-in `spec-system` profile, `policy/doc-artifacts.toml`
owns registered governance artifacts and `policy/spec-system.toml` owns profile
requirements.

Policy ledgers should be machine-readable and reviewed. They should not hide
baseline debt, broaden policy silently, or auto-extend expiry.

## Closeout

Closeouts record what landed.

They should cover:

- completed work.
- validation evidence.
- remaining gaps.
- deferred work.
- rollback notes.

Closeouts should not re-open the original proposal or spec unless the contract
changed.
