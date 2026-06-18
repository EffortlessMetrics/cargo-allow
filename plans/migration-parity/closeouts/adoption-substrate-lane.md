---
id: CARGO-ALLOW-CLOSEOUT-0003
kind: closeout
status: done
owner: repo-infra
created: 2026-06-18
linked_plan: CARGO-ALLOW-PLAN-0002
linked_proposal: CARGO-ALLOW-PROP-0002
linked_spec: CARGO-ALLOW-SPEC-0002
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0002
support_tier_impact: advisory
policy_impact:
  - policy/doc-artifacts.toml
  - .codex/goals/active.toml
---

# Closeout: Adoption Substrate Lane (10-PR Cleanup Queue)

## Summary

Closeout for the adoption-substrate cleanup queue (`adoption-substrate-pr-002`
through `adoption-substrate-pr-013`) on the path to migration and evidence
parity. Landed modularization, advisory ratcheting, governance split, two
in-repository dogfood receipts, structural identity D3 container
module-qualification (#1724, merge `ffc4a47`), D4 receiver/target
fingerprints (#1726, merge `4f19e298`), and D5 lint attribute target
identity (#1728, merge `7b2f2785`), and D6 matcher selector precision (#1730, merge `10e98453`).

This closeout records planning, characterization, and scanner-hardening work
only. It does not claim full xtask replacement, side-by-side dogfood parity
across all compat lanes, `0.2.0` milestone parity, or import-mode (#1466)
implementation.

## Landed Slices

### Migration lane descriptors (adoption-substrate-pr-002)

- Shared migration lane descriptor table in `allow-policy-legacy` (#1709, merge
  `35e1f70a`).

### Evidence and lifecycle helpers (adoption-substrate-pr-003)

- Shared evidence/lifecycle metadata helpers (#1711, merge `04facd42`).

### Closeout queue normalization (adoption-substrate-pr-004)

- Shared `migrate_closeout_queues` and `CloseoutQueueHints` (#1712).

### #1466 governance split (adoption-substrate-pr-005)

- Child issues #1713–#1718; umbrella #1466 remains open for import execution.

### Advisory occurrence counts (adoption-substrate-pr-006)

- Receipt `advisory` counters for ratcheting metadata.

### `--deny <status>` escalation (adoption-substrate-pr-007)

- `check --deny <status>` promotes receipt advisory counts to exit 1 (#1474).

### Per-lane posture model (adoption-substrate-pr-008)

- Optional `[lanes.<kind>]` posture with advisory/shadow/blocking modes (#1473).

### Second dogfood receipt (adoption-substrate-pr-009)

- Unsafe-allowlist side-by-side receipt (#1723, merge `3bd815fd`).
- Panic-baseline receipt from B5 (#1697) remains the first in-repo dogfood slice.

### Structural identity D3 (adoption-substrate-pr-010)

- Module-qualifies unqualified container names in nested modules (#1724, merge
  `ffc4a47`).
- Fixture: `container_same_name_sibling_modules`.

### Structural identity D4 (adoption-substrate-pr-011)

- Parameter-slot receiver fingerprints (`param:N`) and index selector target
  fingerprints (#1726, merge `4f19e298`).
- Fixtures: `rename_local`, `callee_same_receiver_diff`,
  `index_same_form_diff_targets`.

### Structural identity D5 (adoption-substrate-pr-012)

- Lint attribute target identity via `container`/`module` and
  `inner_attribute_item` scope collection (#1728, merge `7b2f2785`).
- Fixture: `lint_same_different_items`.

### Structural identity D6 (adoption-substrate-pr-013)

- `allow-match` `selector_precision` characterization over structural-identity
  fixtures (#1730, merge `10e98453`).
- Policy entries `allow-0215`..`0234` and `allow-0243`..`0246`; no matcher
  logic changes.

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test -p allow-policy-legacy` | pass | adoption-substrate PRs 2–4 |
| `cargo test -p cargo-allow check_deny lane_posture` | pass | PRs 7–8 |
| `cargo test -p allow-rust structural_identity` | pass | D3-D5 fixture matrix |
| `cargo test -p allow-match selector_precision` | pass | D6 matcher characterization |
| `cargo-allow check --mode no-new` | pass | `target/cargo-allow/check.receipt.json` |
| `cargo-allow check --profile spec-system --mode audit` | pass | goal manifest validation |

## Non-Goals

- Version bump or release cut (`0.1.10` remains deferred).
- Import-mode implementation for #1466 child issues.
- Full per-lane dogfood receipts beyond panic-baseline and unsafe-allowlist.
- Structural identity D7 diff posture characterization.

## Claim Boundary

Adoption-substrate queue completion evidence only. `partial` dogfood and D3–D4
rows in gap inventories are not parity claims. Receipts prove scoped in-repository
slices only.

## Remaining Work

- Structural identity D7 (`adoption-substrate-pr-014` in
  `.codex/goals/active.toml`).
- #1466 child issue execution (#1713–#1718) or deferred ripr dogfood (#1718).
- B7 `0.2.0` migration parity release notes after remaining parity proof.
- Per-lane `partial` compat rows — additional side-by-side dogfood still open.

## Follow-Up Links

- PR: #1730 (D6)
- Closeout predecessor: CARGO-ALLOW-CLOSEOUT-0002
- Next ready lane: `adoption-substrate-pr-014` (structural identity D7)
