---
id: CARGO-ALLOW-PLAN-0005
kind: implementation_plan
status: draft
owner: repo-infra
created: 2026-06-17
linked_proposal: CARGO-ALLOW-PROP-0005
linked_spec: CARGO-ALLOW-SPEC-0005
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
support_tier_impact: advisory
policy_impact: none
---

# Implementation Plan: Structural Identity Quality

## Purpose

Fixture-backed hardening of source-visible identity without broad scanner
rewrites.

## PR Sequence (D1–D8)

| PR | Purpose | Primary paths |
| --- | --- | --- |
| D1 | Inventory current identity gaps by finding kind | `plans/structural-identity/gap-inventory.md`, `docs/identity.md` |
| D2 | AST identity fixture matrix (unsafe/panic/lint/index) | `tests/fixtures/structural-identity/` |
| D3 | Improve container identity for ambiguous contexts | `crates/allow-rust/` |
| D4 | Improve receiver/target identity for method/index | `crates/allow-rust/` |
| D5 | Improve lint attribute target identity | `crates/allow-rust/` |
| D6 | Assert selector precision with new fields | `crates/allow-match/` |
| D7 | Verify weakening/improvement with new identity fields | `crates/allow-diff/` |
| D8 | Scanner limitation examples and claim boundary docs | `docs/identity.md`, `docs/claim-boundaries.md` |

## Execution State (2026-06-18)

| PR | Status | Evidence |
| --- | --- | --- |
| D1 — gap inventory | done | [gap-inventory.md](gap-inventory.md), [docs/identity.md](../../docs/identity.md) |
| D2 — refactor-pair fixture matrix | done | #1701 (merge `2165848`); `tests/fixtures/structural-identity/`, `allow-rust` `structural_identity` tests, policy allow-0215..0234 |
| D3 — container identity in nested modules | done | #1724 (merge `ffc4a47`); module-qualifies unqualified containers; fixture `container_same_name_sibling_modules` |
| D4 — receiver/target identity for method/index | done | #1726 (merge `4f19e298`); parameter-slot receiver fingerprints (`param:N`); index selector target fingerprints; fixtures `rename_local`, `callee_same_receiver_diff`, `index_same_form_diff_targets` |

## Validation Baseline

Targeted identity tests plus default no-new guard per PR.

## Claim Boundary

Each PR proves source-syntax identity behavior for its fixture slice only.
