---
id: CARGO-ALLOW-CLOSEOUT-0004
kind: closeout
status: done
owner: repo-infra
created: 2026-06-18
linked_plan: CARGO-ALLOW-PLAN-0004
linked_proposal: CARGO-ALLOW-PROP-0004
linked_spec: CARGO-ALLOW-SPEC-0004
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0002
support_tier_impact: advisory
policy_impact:
  - policy/doc-artifacts.toml
  - .codex/goals/active.toml
---

# Closeout: Import Parity Execution Lane (#1713–#1718)

## Summary

Closeout for the import-parity execution lane split from umbrella #1466.
Landed characterization slices #1713 (semantic selector fields, #1735 merge
`d5983b81`), #1714 (advisory drift / `last_seen`, #1737 merge `4dffb1e7`),
#1715 (recorded re-bless / `refresh`, #1738), #1716 (multi-family
`LegacyImportBatch`, #1739), #1717 (owner/reason/evidence acceptance fixture,
#1740), and #1718 (ripr-style in-repo multi-family dogfood receipt, #1741 merge
`41359151`).

This closeout records fixture-backed import characterization and in-repository
dogfood evidence only. It does not claim full import mode, external `ripr`
migration, xtask retirement, or `0.2.0` milestone parity.

## Landed Slices

### Semantic selector import (#1713 / import-parity-1713)

- Preserved `receiver_fingerprint`, `target_fingerprint`, `symbol`, and
  `normalized_snippet_hash` from legacy nested selector tables (#1735).

### Advisory drift import (#1714 / import-parity-1714)

- Preserved `last_seen` and `line_hint` on clippy import; emit
  `MatchStatus::LocationDrift` advisories without failing no-new (#1737).

### Recorded re-bless receipts (#1715 / import-parity-1715)

- `cargo-allow refresh` command and `cargo-allow.refresh.v1` receipt for
  operator-approved `last_seen` updates after advisory drift (#1738).

### Multi-family legacy ledger model (#1716 / import-parity-1716)

- `LegacyImportBatch` / `LegacyImportFamily` policy-dir batch import with
  deterministic lane-descriptor ordering (#1739).

### Owner/reason/evidence acceptance (#1717 / import-parity-1717)

- `import_parity_metadata_acceptance_tests.rs` characterizes governance
  round-trip across no-panic, lint, and unsafe lanes (#1740).

### Ripr-style adoption dogfood (#1718 / import-parity-1718)

- `docs/dogfood/cargo-allow-ripr-style-adoption.md` records multi-family
  panic+unsafe+lint compat→migrate→check→worklist→closeout for in-repo fixtures
  (#1741).

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test -p allow-policy-legacy semantic_selector` | pass | #1735 |
| `cargo test -p allow-policy-legacy advisory_drift` | pass | #1737 |
| `cargo test -p allow-report refresh` | pass | #1738 |
| `cargo test -p allow-policy-legacy legacy_import_batch` | pass | #1739 |
| `cargo test -p allow-policy-legacy import_parity_metadata_acceptance` | pass | #1740 |
| ripr-style dogfood proof commands | pass | `docs/dogfood/cargo-allow-ripr-style-adoption.md` |
| `cargo-allow check --mode no-new` | pass | `target/cargo-allow/check.receipt.json` |

## Non-Goals

- Full `.allow` namespace import mode (CARGO-ALLOW-PLAN-0004 C2–C12).
- External `ripr` repository migration or proof-command execution.
- Closing umbrella #1466 — remains open for full import mode and external adoption.
- Version bump or release cut (`0.1.10` / `0.2.0` remain deferred).

## Claim Boundary

Import-parity execution lane characterization evidence only. `partial` rows in
`gap-inventory.md` are not parity claims. Dogfood receipts prove scoped
in-repository slices only.

## Remaining Work

- Umbrella #1466: full import mode product behavior and external adoption proof.
- Per-lane `partial` compat rows — additional side-by-side dogfood still open.
- B7 `0.2.0` migration parity release notes after remaining parity proof.

## Follow-Up Links

- Umbrella: #1466 (reopened; child slices #1713–#1718 closed)
- Closeout predecessor: CARGO-ALLOW-CLOSEOUT-0003
- Gap inventory: `plans/migration-parity/gap-inventory.md`
