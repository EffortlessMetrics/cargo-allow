---
id: CARGO-ALLOW-PLAN-0008
kind: implementation_plan
status: draft
owner: repo-infra
created: 2026-06-18
linked_proposal: CARGO-ALLOW-PROP-0004
linked_spec: CARGO-ALLOW-SPEC-0004
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
support_tier_impact: advisory
policy_impact: none
---

# ripr Spec-System Adoption Handoff

This is the first external-dogfood handoff after the published cargo-allow
`0.1.8` onboarding cleanup. It prepares the `ripr` adoption lane without
changing `ripr` yet.

## Purpose

Adopt cargo-allow in `ripr` one step at a time:

1. inventory the existing source-exception posture.
2. preview the opt-in `spec-system` bootstrap.
3. map existing source-of-truth artifacts into the generated profile.
4. upload advisory artifacts in CI.
5. fix one objective structural worklist class.
6. file cargo-allow issues for portability friction.

## Non-Goals

- Do not make spec-system part of default cargo-allow behavior.
- Do not block `ripr` CI on spec-system in the first adoption PR.
- Do not execute proof commands as part of cargo-allow's scan.
- Do not call GitHub APIs, network services, Cargo, rustc, Clippy, ripr,
  unsafe-review, coverage, build scripts, or proc macros from the scan.
- Do not migrate every `ripr` planning artifact in the first PR.
- Do not fix cargo-allow portability issues with silent `ripr`-local
  workarounds.

## Preflight

Run these from the `ripr` repository root. Use the published crate, not a local
cargo-allow checkout:

```bash
cargo install cargo-allow --version 0.1.8 --locked --force
cargo-allow --version
git status --short --branch
gh pr list --state open --limit 20
```

Then capture default cargo-allow posture:

```bash
cargo-allow doctor
cargo-allow audit --format json --output target/cargo-allow/audit.json
cargo-allow check \
  --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

Record whether the default ledger passes, fails with a clear repair queue, or
needs a separate policy bootstrap before spec-system work starts.

## Candidate ripr Artifacts To Map

Verify these paths live before editing. They are candidate inputs for
`policy/doc-artifacts.toml`, not assumptions:

| Source-tree surface | Candidate paths |
| --- | --- |
| active execution state | `.ripr/goals/active.toml`, `.ripr/goals/*.toml` |
| proposals | `docs/proposals/*.md` |
| specs | `docs/specs/*.md` |
| ADRs | `docs/adr/*.md` |
| CI policy and lanes | `docs/ci/*.md`, `.github/workflows/*.yml` |
| support tiers | `docs/status/SUPPORT_TIERS.md` |
| plans and closeouts | `plans/`, `docs/handoffs/*.md` |
| repo policy | `policy/`, `ripr.toml.example`, `docs/policy/*.md` |

If the generated cargo-allow layout does not fit these surfaces cleanly, file a
cargo-allow adoption-friction issue before inventing a repo-local convention.

## Bootstrap Sequence

Preview first:

```bash
cargo-allow init --profile spec-system --dry-run
```

If the dry run matches the repo shape, bootstrap:

```bash
cargo-allow init --profile spec-system
cargo-allow doctor --profile spec-system
cargo-allow check \
  --profile spec-system \
  --mode audit \
  --format json \
  --output target/cargo-allow/spec-system.json
cargo-allow worklist \
  --profile spec-system \
  --format json \
  --output target/cargo-allow/spec-system-worklist.json
```

Keep `active_goal_required = false` during the first-hour bootstrap. Flip it to
`true` only after real proposal/spec/plan/support-tier/closeout links are
registered and the graph check has no unknown-link findings for those edges.

## Initial Ledger Seed

Start with the smallest useful graph:

1. one active or recently accepted proposal.
2. the spec that owns that proposal's behavior.
3. a plan or handoff that implements the spec.
4. the support-tier row that defines the public claim boundary.
5. one closeout if the plan item is already done.

Prefer existing `ripr` IDs and filenames. If a candidate artifact lacks a
machine-readable ID, use the worklist item to decide whether to add front
matter or register the current path with a stable ledger ID.

## CI Artifact Plan

Add only non-blocking artifact upload in the first adoption PR:

```bash
cargo-allow check \
  --profile spec-system \
  --mode audit \
  --format json \
  --output target/cargo-allow/spec-system.json

cargo-allow worklist \
  --profile spec-system \
  --format json \
  --output target/cargo-allow/spec-system-worklist.json
```

Upload `target/cargo-allow/` on success and failure. Keep default no-new as the
source-exception gate if the repository is ready for it; keep spec-system
advisory until the first worklist class is understood.

## First Repair Class

Fix one objective structural class first:

- duplicate artifact IDs.
- missing registered artifact files.
- invalid artifact kinds or statuses.
- unknown linked artifact IDs.
- artifact files that do not contain their declared IDs.
- profile config or doc-artifact ledger parse failures.

Keep these advisory during the first `ripr` adoption:

- stale active goals.
- missing closeouts.
- support-tier completeness.
- README or release claim coverage.

## Feedback Loop

File cargo-allow issues for:

- confusing init layout.
- profile config that does not fit `ripr`.
- missing artifact kinds or edge types.
- unclear doctor readiness.
- vague worklist messages.
- false-positive graph findings.
- schema/report mismatches.
- CI integration friction.
- documentation gaps.

Use the cargo-allow adoption-friction issue template and attach focused
snippets from `target/cargo-allow/spec-system.json`,
`target/cargo-allow/spec-system-worklist.json`, `policy/spec-system.toml`, and
`policy/doc-artifacts.toml`.

## Validation

For the `ripr` adoption PR, record:

```bash
cargo-allow --version
cargo-allow doctor
cargo-allow check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md
cargo-allow doctor --profile spec-system --format json --output target/cargo-allow/spec-system-doctor.json
cargo-allow check --profile spec-system --mode audit --format json --output target/cargo-allow/spec-system.json
cargo-allow worklist --profile spec-system --format json --output target/cargo-allow/spec-system-worklist.json
```

If `ripr` needs repository-specific tests for the PR, run those separately.
Do not describe those tests as cargo-allow proof execution.

## Claim Boundary

Spec-system is structural source-tree graph validation. It may parse TOML and
Markdown and inspect repository files. It may verify IDs, paths, statuses,
links, support-tier proof fields, active-goal references, and closeout links.

It does not execute proof commands, run `ripr`, run tests, call GitHub APIs,
inspect remote PR state, run Cargo, rustc, Clippy, build scripts, proc macros,
unsafe-review, coverage, or network checks as part of the cargo-allow scan.

## Rollback

If the first adoption PR creates noise:

1. remove the non-blocking spec-system CI artifact job.
2. keep or remove generated `policy/spec-system.toml` and
   `policy/doc-artifacts.toml` based on whether they are useful locally.
3. file cargo-allow issues for the observed friction.
4. leave default cargo-allow source-exception checks independent of the
   spec-system rollout.
