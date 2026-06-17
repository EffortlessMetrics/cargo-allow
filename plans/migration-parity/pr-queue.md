# Migration Parity PR Queue

Actionable inventory for the `0.2.0` migration parity lane
([CARGO-ALLOW-SPEC-0002](../../docs/specs/CARGO-ALLOW-SPEC-0002-migration-parity.md)).

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

## Gap Inventory Template

Track each open gap in this table as lanes are inventoried in PR B1:

| Lane | Gap | Owner | Evidence needed | PR target |
| --- | --- | --- | --- | --- |
| | | | | |

## PR Queue (B1–B7)

### PR B1 — Inventory remaining legacy allowlist parity gaps

Purpose: turn goal text into an actionable gap table per surface.

Non-goals: no compat behavior changes.

Files: `plans/migration-parity/pr-queue.md`, `plans/migration-parity/gap-inventory.md`,
`docs/migration-from-xtask.md` (delta notes only).

Validation: spec-system audit; no-new guard.

Claim boundary: inventory and classification only.

### PR B2 — Close one compat/evidence gap for highest-value lane

Purpose: pick one lane with highest adoption friction and land a focused fix
with fixture-backed characterization.

Candidate lanes: unsafe evidence migration, panic occurrence limits, lint evidence
preservation, non-Rust/generated executable evidence.

Non-goals: no new compat kinds; no `0.2.0` release cut.

Validation: targeted tests plus validation baseline from
[implementation-plan.md](implementation-plan.md).

Claim boundary: one lane delta documented; not full xtask retirement.

### PR B3 — Add fixture matrix for all supported legacy lanes

Purpose: characterization coverage across compat kinds.

Files: `tests/fixtures/migration/`, compat tests.

Claim boundary: fixture-backed observed behavior only.

### PR B4 — Add/refresh migration closeout guide

Purpose: make `cargo-allow.migrate.v1` summaries actionable without chat memory.

Files: `docs/how-to/migration-evidence-cookbook.md`, closeout routing docs.

Claim boundary: closeout routing metadata only.

### PR B5 — Add side-by-side dogfood receipt on cargo-allow

Purpose: run migration parity proof against this repository's own legacy surfaces.

Files: `docs/dogfood/`, receipts under `target/cargo-allow/`.

Claim boundary: dogfood evidence for this repo only.

### PR B6 — Close or split remaining import/parity issue #1466

Purpose: resolve or explicitly defer the tracked import/parity issue.

Non-goals: no silent broadening.

Claim boundary: issue disposition recorded in plan/closeout.

### PR B7 — Stage 0.2.0 migration parity notes

Purpose: document milestone claim boundary before any `0.2.0` cut authorization.

Non-goals: no version bump without explicit authorization.

Files: `docs/release/` notes, CHANGELOG section, support-tier review.

Claim boundary: release notes only; not a parity proof.
