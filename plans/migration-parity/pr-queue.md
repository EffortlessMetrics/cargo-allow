# Migration Parity PR Queue

Actionable inventory for the `0.2.0` migration parity lane
([CARGO-ALLOW-SPEC-0002](../../docs/specs/CARGO-ALLOW-SPEC-0002-migration-parity.md)).

## Execution State (2026-06-18)

| PR | Status | Evidence |
| --- | --- | --- |
| B1 / B1r — gap inventory reconciliation | done | this queue + [gap-inventory.md](gap-inventory.md) |
| Goal registration (PR 1 / #1687) | done | `.codex/goals/active.toml`, `policy/doc-artifacts.toml` |
| B2 — no-panic-baseline evidence/lifecycle slice | done | #1691 (merge `1cd408e`) |
| B3 — migration fixture matrix | done | #1693 (merge `cd0ab7b`) |
| B4 — migration closeout routing | done | #1695 (merge `64832c5`); closeout goal #1696 (merge `e9b4f9f`) |
| B5 — panic-baseline dogfood receipt | done | #1697 (merge `26a6873`); [cargo-allow-panic-baseline.md](../../docs/dogfood/cargo-allow-panic-baseline.md) |
| B6 — import/parity issue disposition | done | #1470 closed (#1699/`53ea19aa`, #1700); #1466 split deferred to adoption-substrate-pr-005 |
| D2 — structural identity refactor-pair matrix | done | #1701 (merge `2165848`); `tests/fixtures/structural-identity/` |
| Release hardening E1 (preflight) | done, dormant | #1703; not an active release lane |
| Release hardening E1b (dry-run + registry visibility) | done, dormant | #1704; not an active release lane |
| Release hardening E1c (install-smoke) | done, dormant | #1705; not an active release lane |
| #1478 — spec-system profile hygiene | closed | #1706; [plans/spec-system/closeout.md](../spec-system/closeout.md) |

**Active lane:** adoption-substrate PRs 2–6 in [`.codex/goals/active.toml`](../../.codex/goals/active.toml).
**Dormant:** `0.1.10` release cut per [0.1.10-readiness.md](../../docs/release/0.1.10-readiness.md).

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

### PR B3 — Add fixture matrix for all supported legacy lanes (done, #1693)

Purpose: characterization coverage across compat kinds under
`tests/fixtures/migration/`.

Status: done (merge `cd0ab7b`).

Files: `tests/fixtures/migration/`, compat tests in `allow-policy-legacy`.

Active goal work item: `migration-parity-b3`.

Claim boundary: fixture-backed observed behavior only.

### PR B4 — Add/refresh migration closeout guide (done, #1695)

Purpose: make `cargo-allow.migrate.v1` summaries actionable without chat memory.

Status: done (merge `64832c5`).

Files: `docs/how-to/migration-evidence-cookbook.md`, `docs/schemas/migrate.schema.json`,
`crates/allow-report/src/migrate_closeout.rs`.

Active goal work item: `migration-parity-b4`.

Claim boundary: closeout routing metadata only.

### PR B5 — Add side-by-side dogfood receipt on cargo-allow (done, #1697)

Purpose: run migration parity proof against this repository's own legacy surfaces.

Status: done (merge `26a6873`).

Files: `docs/dogfood/cargo-allow-panic-baseline.md`, `docs/dogfood/fixtures/`,
`docs/dogfood/receipts/`.

Active goal work item: `migration-parity-b5`.

Claim boundary: dogfood evidence for this repo only; one characterized
panic-baseline slice.

### PR B6 — Close or split remaining import/parity issues (done)

Purpose: resolve or explicitly defer tracked adoption blockers:

- [#1470](https://github.com/EffortlessMetrics/cargo-allow/issues/1470) —
  foreign-dialect `policy/allow.toml` discovery — **closed** in #1699 (merge
  `53ea19aa`) and #1700
- [#1466](https://github.com/EffortlessMetrics/cargo-allow/issues/1466) —
  bespoke semantic-selector ledger import/parity — **open umbrella**; governance
  split tracked in `adoption-substrate-pr-005`

Status: done for #1470 disposition; #1466 split deferred to adoption lane.

Non-goals: no silent broadening; no import implementation in disposition PR.

Claim boundary: issue disposition recorded in plan/closeout/gap-inventory.

### PR B7 — Stage 0.2.0 migration parity notes

Purpose: document milestone claim boundary before any `0.2.0` cut authorization.

Status: pending — after adoption-substrate PRs 2–6.

Non-goals: no version bump without explicit authorization.

Files: `docs/release/` notes, CHANGELOG section, support-tier review.

Claim boundary: release notes only; not a parity proof.

## Adoption Substrate Queue (active)

Internal coherence and modularization on the migration path. See
[gap-inventory.md](gap-inventory.md) adoption section and
`.codex/goals/active.toml` work items `adoption-substrate-pr-002` through
`adoption-substrate-pr-006`.

| PR | Work item | Status |
| --- | --- | --- |
| PR 2 | migration lane descriptors | ready |
| PR 3 | evidence/lifecycle helpers | blocked (after PR 2) |
| PR 4 | closeout queue normalization | blocked (after PR 3) |
| PR 5 | split #1466 governance | blocked (after PR 4) |
| PR 6 | advisory occurrence counts | blocked (after PR 5) |

## Dormant Release Lane (0.1.10)

Release automation groundwork landed on `main` (#1703–#1705). Cut is **deferred**
pending adoption/cleanup lane; operator OIDC/dry-run steps are not blocking
normal PR work. See [0.1.10-readiness.md](../../docs/release/0.1.10-readiness.md)
and [0.1.10-implementation-plan.md](../release/0.1.10-implementation-plan.md).
