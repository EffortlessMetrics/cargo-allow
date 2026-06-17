---
id: CARGO-ALLOW-SPEC-0006
kind: spec
status: accepted
owner: repo-infra
created: 2026-06-17
linked_proposal: CARGO-ALLOW-PROP-0006
linked_adrs: []
support_tier_impact: none
policy_impact: none
---

# Spec: Adoption-Trust Release 0.1.10

## Summary

This spec defines scope, prerequisites, and non-claims for the `0.1.10`
adoption-trust patch. Release automation exists on `main`; this spec governs
operational proof before the first tag-triggered automated release.

## Scope (In)

- post-`0.1.9` test hardening already under `[Unreleased]`;
- tag-triggered release workflow on `main`;
- provider-tracked readiness policy documentation;
- migration parity and identity groundwork (docs/inventory/fixture PRs);
- release operator documentation and dry-run evidence.

## Scope (Out)

- `0.2.0` migration parity milestone cut;
- full `.allow` migration or import adapters;
- external repo migration at scale;
- `ripr+` / `unsafe-review+` zero claims;
- spec-system stable tier promotion.

## Prerequisites

| Prerequisite | Verification |
| --- | --- |
| Trusted Publishing on 10 crates | crates.io settings per [docs/release/README.md](../release/README.md) |
| Workflow dry-run | Actions → Release → workflow_dispatch on `main` |
| Token fallback documented | Only if OIDC not configured for all crates |
| Readiness policy recorded | [CARGO-ALLOW-SPEC-0003](CARGO-ALLOW-SPEC-0003-readiness-policy.md) |
| No-new guard green on release head | `cargo-allow check --mode no-new` receipt |

## Release Operator Steps

1. Merge release-prep PR (version bump, `docs/release/0.1.10.md`,
   `docs/release/github/v0.1.10.md`, CHANGELOG section).
2. Confirm Trusted Publishing or approved token fallback.
3. Run workflow_dispatch dry-run if not recently green.
4. Push annotated tag `v0.1.10` only after explicit authorization.
5. Record workflow run id, registry visibility, and install-smoke in release
   closeout.

## Linked Artifacts

- Proposal:
  [CARGO-ALLOW-PROP-0006](../proposals/CARGO-ALLOW-PROP-0006-adoption-trust-0.1.10.md)
- Implementation plan:
  [plans/release/0.1.10-implementation-plan.md](../../plans/release/0.1.10-implementation-plan.md)
- Release automation:
  [docs/release/README.md](../release/README.md)

## Claim Boundary

This spec governs `0.1.10` release scope and prerequisites. It does not prove
successful publish, install-smoke, or external adoption readiness.
