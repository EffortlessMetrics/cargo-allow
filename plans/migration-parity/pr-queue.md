# Migration Parity PR Queue

Actionable inventory for the `0.2.0` migration parity lane
([CARGO-ALLOW-SPEC-0002](../../docs/specs/CARGO-ALLOW-SPEC-0002-migration-parity.md)).

## Execution State (2026-06-17)

| PR | Status | Evidence |
| --- | --- | --- |
| B1 / B1r — gap inventory reconciliation | done | this queue + [gap-inventory.md](gap-inventory.md) |
| Goal registration (PR 1 / #1687) | done | `.codex/goals/active.toml`, `policy/doc-artifacts.toml` |
| B2 — no-panic-baseline evidence/lifecycle slice | done | #1691 (merge `1cd408e`) |
| B3 — migration fixture matrix | **in progress** | `tests/fixtures/migration/`, `migration_fixture_matrix_tests.rs` |

## Surfaces

| Lane | Legacy input | Compat kind | Primary paths |
| --- | --- | --- | --- |
| Non-Rust tracked files | `policy/non-rust-allowlist.toml` | `non-rust` | `allow-policy-legacy`, `docs/migration-from-xtask.md` |
| Generated files | generated policy | `generated` | `allow-policy-legacy` |
| Executable files | executable policy | `executable` | `allow-policy-legacy` |
| Workflow files | workflow policy | `workflow` | `allow-policy-legacy` |
| Dependency surface | dependency policy | `dependency-surface` | `allow-policy-legacy` |
| Process policy | process policy | `process` | `allow-policy-legacy` |
| Network policy | network policy | `network` | `allow-policy-legacy` |
| Panic allowlist | `policy/no-panic-allowlist.toml` | `no-panic-allowlist` | `allow-policy-legacy` |
| Panic baseline | `policy/no-panic-baseline.toml` | `panic` | `allow-policy-legacy` |
| Lint exceptions | lint policy | `lint-exception` | `allow-policy-legacy` |
| Unsafe allowlist | `policy/unsafe-allowlist.toml` | `unsafe` | `allow-policy-legacy` |
| Doc/spec-system policy | `policy/doc-artifacts.toml` | spec-system profile | `policy/spec-system.toml` |

## Per-Lane Acceptance

Each lane is parity-ready only when all of the following hold:

- legacy input parses without silent drops;
- migration preserves owner, reason, evidence, and links where present;
- missing evidence becomes visible debt, not approval;
- occurrence limits and `baseline_debt` remain visible;
- canonical `policy/allow.toml` output is stable across reruns;
- `cargo-allow check --compat --kind <kind>` compares legacy vs canonical;
- worklist and closeout queues route remaining debt;
- migration docs show the path and known deltas.

No compat lane currently meets all criteria. See [gap-inventory.md](gap-inventory.md).

## PR Queue (B1–B7)

### PR B1 — Inventory remaining legacy allowlist parity gaps (done / B1r)

Purpose: turn goal text into an actionable gap table per surface.

Status: done — gap inventory reconciled from `allow-policy-legacy` tests and open
issues (#1466, #1470).

Non-goals: no compat behavior changes.

Files: `plans/migration-parity/pr-queue.md`, `plans/migration-parity/gap-inventory.md`,
`.codex/goals/active.toml`.

Validation: spec-system audit; no-new guard.

Claim boundary: inventory and classification only.

### PR B2 — Close no-panic-baseline evidence/lifecycle gap (done, #1691)

Purpose: close the highest-friction panic-baseline slice — metadata/evidence
preservation, visible `baseline_debt` for missing evidence, and lifecycle fix for
`review_after` without `expires`.

Status: done (merge `1cd408e`).

Non-goals: no new compat kinds; no `0.2.0` release cut; no full panic-lane
parity claim.

Validation: `allow-policy-legacy` no-panic tests plus validation baseline.

Claim boundary: no-panic-baseline import slice only; not full xtask retirement.

### PR B3 — Add fixture matrix for all supported legacy lanes (next)

Purpose: characterization coverage across compat kinds under
`tests/fixtures/migration/`.

Files: `tests/fixtures/migration/`, compat tests in `allow-policy-legacy`.

Active goal work item: `migration-parity-b3`.

Claim boundary: fixture-backed observed behavior only.

### PR B4 — Add/refresh migration closeout guide

Purpose: make `cargo-allow.migrate.v1` summaries actionable without chat memory.

Files: `docs/how-to/migration-evidence-cookbook.md`, closeout routing docs.

Claim boundary: closeout routing metadata only.

### PR B5 — Add side-by-side dogfood receipt on cargo-allow

Purpose: run migration parity proof against this repository's own legacy surfaces.

Files: `docs/dogfood/`, receipts under `target/cargo-allow/`.

Claim boundary: dogfood evidence for this repo only.

### PR B6 — Close or split remaining import/parity issues

Purpose: resolve or explicitly defer tracked adoption blockers:

- [#1466](https://github.com/EffortlessMetrics/cargo-allow/issues/1466) —
  bespoke semantic-selector ledger import/parity
- [#1470](https://github.com/EffortlessMetrics/cargo-allow/issues/1470) —
  foreign-dialect `policy/allow.toml` discovery

Non-goals: no silent broadening.

Claim boundary: issue disposition recorded in plan/closeout.

### PR B7 — Stage 0.2.0 migration parity notes

Purpose: document milestone claim boundary before any `0.2.0` cut authorization.

Non-goals: no version bump without explicit authorization.

Files: `docs/release/` notes, CHANGELOG section, support-tier review.

Claim boundary: release notes only; not a parity proof.
