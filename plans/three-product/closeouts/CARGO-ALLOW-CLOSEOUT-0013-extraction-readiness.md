---
id: CARGO-ALLOW-CLOSEOUT-0013
kind: closeout
status: accepted
owner: repo-infra
created: 2026-07-24
linked_plan: CARGO-ALLOW-PLAN-0010
linked_spec: CARGO-ALLOW-SPEC-0010
linked_adr: CARGO-ALLOW-ADR-0002
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
  - tests/fixtures/extraction-readiness/checklist-v1.toml
---

# Closeout: Extraction Readiness Receipt (#2559)

## Summary

Emit a monorepo extraction-readiness checklist receipt that aggregates Wave 6
dogfood (#2558), simplification (#2208), and exact-candidate interop (#2605)
evidence surfaces without creating external repositories or authorizing a
physical split.

## Landed Changes

- `tests/fixtures/extraction-readiness/checklist-v1.toml` — nine gate items
- `scripts/extraction-readiness-receipt.sh` — checklist, topology, support-tier,
  forbidden-dependency, rollback, and prerequisite-receipt validation
- Offline characterization tests and example receipt
- CI wiring in the `test` job after simplification audit

## Validation Evidence

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test -p cargo-allow extraction_readiness` | pass | characterization |
| `bash scripts/extraction-readiness-receipt.sh` | pass | readiness receipt |
| `cargo-allow check --mode no-new` | pass | policy allows for new artifacts |

## Claim Boundary

**Establishes:** checked monorepo readiness for independent packaging posture,
public boundary registries, distinct support documentation, forbidden production
dependency absence on product binaries, and documented rollback for Wave 6
packets.

**Does not establish:** physical repository extraction, independent per-product
CI lane split, automated release choreography for future repos, or promotion of
cargo-intent/cargo-proof beyond experimental posture.

## Wave 6 Status

| Issue | Status |
| --- | --- |
| #2605 exact-candidate interop | merged |
| #2558 three-product dogfood | merged |
| #2208 simplification | merged |
| #2559 extraction readiness | this closeout |

Physical repository extraction remains blocked until separate explicit
authorization after this evidence is reviewed.

## Rollback

Revert the checklist fixture, receipt script, CI step, closeout, and policy
allows. No product runtime behavior changes to roll back.
