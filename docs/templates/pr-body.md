<!--
Use this body for source-of-truth and policy-governance PRs. Keep the PR scoped
to one proof obligation. Do not claim proof execution or semantic correctness
unless the validation evidence proves that exact claim.
-->

## Purpose

Describe the concrete PR-sized outcome.

Linked artifacts:

- Proposal:
- Spec:
- ADR:
- Implementation plan:
- Active goal or work item:
- Support-tier surface:
- Policy ledger:
- Closeout:

## Non-Goals

- Non-goal:
- Non-goal:

## Validation

- [ ] `rtk cargo fmt --all --check`
- [ ] `rtk cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `rtk cargo test --workspace`
- [ ] `rtk cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md`

Add profile-specific commands only after the profile exists. Mark checks as
`not run` when evidence is absent.

## Claim Boundary

State exactly what this PR proves. Do not claim proof execution, semantic
correctness, release readiness, unsafe soundness, test adequacy, coverage, or
default behavior changes unless this PR directly implements and validates those
claims.

## Rollback

Describe how to revert this PR and which generated artifacts, policy entries,
or follow-up docs would need cleanup.
