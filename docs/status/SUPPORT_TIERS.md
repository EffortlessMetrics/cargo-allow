---
id: CARGO-ALLOW-SUPPORT-0001
kind: support_tier
status: active
owner: repo-infra
created: 2026-06-12
updated: 2026-07-28
linked_proposal: CARGO-ALLOW-PROP-0010
linked_spec: CARGO-ALLOW-SPEC-0011
---

# Support Tiers

## Purpose

This file maps user-facing claims to the proof command or retained evidence a
maintainer should review. It does not promote a product merely because a crate,
binary, fixture, or local package smoke exists.

Cargo-allow, cargo-intent, and cargo-proof have independent support and release
posture. Registry visibility, supported direct-library use, product support,
integrated dogfood, and physical repository extraction are separate decisions.

## Tier vocabulary

| Tier | Meaning |
| --- | --- |
| Stable | Current supported product behavior with a direct proof command and published or explicitly selected channel. |
| Stabilizing | Useful current behavior whose wording, output, platform matrix, or adoption evidence is still maturing. |
| Experimental | Landed product or package behavior available for development and exact-candidate proof, without a stable support contract. |
| Compatibility | Bounded historical or legacy route that delegates to a canonical owner or fails explicitly. |
| Advisory | Documented direction, governance control, or non-blocking evidence mapping. |
| Not included | Deliberately outside the selected product/channel claim. |

Stable and stabilizing rows require non-empty proof. Experimental rows must name
the exact current boundary and must not imply publication or compatibility.

## Current product claims

| Surface | Tier | Claim | Proof or evidence | Limitations |
| --- | --- | --- | --- | --- |
| cargo-allow published source-exception ledger | Stable | Published `cargo-allow 0.1.11` scans selected source-tree/source-syntax surfaces and checks findings against `policy/allow.toml` without executing project code. | `cargo install cargo-allow --version 0.1.11 --locked` then `cargo-allow check --mode no-new` | Applies to the published 0.1.11 command/schema/support channel, not unreleased main. |
| cargo-allow 0.2 source candidate | Stabilizing | Current main contains the operated source-exception ledger and is being qualified for an exact evidence-backed 0.2 release. | `cargo run -p cargo-allow -- check --mode no-new`, V2 authority #2921–#2923, and exact-candidate receipts #2924–#2926 | Workspace version `0.2.0` is not a tag or release authorization. Architecture/package/release blockers remain. |
| PR posture | Stabilizing | `cargo-allow diff --base <base>` reports source-exception posture movement for an exact meaningful base/head pair. | `cargo-allow diff --base origin/main --format markdown` | Does not prove build, tests, coverage, unsafe correctness, or complete semantic reachability. Base/head scanner completeness remains a release-trust gate. |
| Worklist routing | Stabilizing | `cargo-allow worklist --format json` emits bounded source-exception repair items for humans and agents. | `cargo-allow worklist --format json` | Suggested proof commands are not commands cargo-allow executed. |
| cargo-allow mutation | Stabilizing | Selected mutation commands route through repository-contained locking and atomic single-target apply. | command-specific mutation receipts and cargo-allow no-new proof | Canonical underlying target identity, collision law, and final pre-replace recheck remain release blockers. |
| Legacy spec-system staged precommit | Compatibility | The selected staged-precommit compatibility operation delegates one-way to installed cargo-intent through `repo.analysis-receipt.v1`, or fails explicitly. | `scripts/spec-system-cutover-receipt.sh`, #2901 transport proof, and installed-candidate interop smoke | The subprocess transport is bounded and hardened; canonical graph semantics, broader operation parity, and old evaluator/schema deletion remain incomplete. No silent fallback is supportable after cutover. |
| Historical spec-system artifacts | Compatibility | Original generations remain readable for migration and provenance where exact readers are retained. | compatibility fixtures and move/parity receipts | Historical input cannot strengthen current cargo-intent or support claims. |
| cargo-intent | Experimental | A landed read-only product shell provides identity and staged-precommit change-status walking-skeleton behavior. | `cargo run -p cargo-intent -- identity` and `cargo run -p cargo-intent -- --format json change status --staged --phase precommit` | Canonical many-source graph compilation, general queries, semantic parity, independent publication, and old-authority deletion are incomplete. |
| cargo-proof | Experimental | Landed protocol, planning, dry-run, provider-contract, captured-receipt, currentness, contradiction, and phase-gate skeletons can be exercised in the workspace. | `cargo run -p cargo-proof -- identity`, `cargo run -p cargo-proof -- plan --obligation-plan <path>`, and selected crate tests | Real selected provider execution and external RIPR cutover are incomplete; fake/stub paths are not product proof. |
| shared repository substrate | Experimental | The four logical shared crates exist for neutral protocol, source views, safe edits, and Rust structural indexing. | package-specific tests plus V2 identity/closure gates #2921–#2923 | Cargo package identities, explicit 0.1 versions, dependency neutrality, publication posture, and direct-use support are not yet converged. |
| integrated three-product dogfood | Advisory | The monorepo exercises one bounded cross-product journey to detect wiring regressions. | `scripts/three-product-dogfood-smoke.sh` | Stubbed/fake evidence stages and workspace proximity prevent a product-support or repository-extraction claim. |
| migration compatibility lanes | Advisory | `cargo-allow check --compat --kind <kind>` supports side-by-side source-exception migration evidence. | `cargo-allow check --compat --kind non-rust` | Compatibility bridges do not prove full xtask replacement or current intent/proof authority. |
| physical repository extraction | Not included | No current product or receipt authorizes moving the crate families into separate repositories. | CARGO-ALLOW-SPEC-0011 and the convergence plan | Requires independent package/CI/support proof, external dogfood, shim/private-path deletion, simplification review, and a later explicit authorization. |

## Claim boundaries

Cargo-allow source scans do not compile code, invoke Cargo metadata during normal
scans, run rustc or Clippy, execute build scripts or proc macros, call GitHub,
run RIPR/Hawk, execute tests, or prove unsafe correctness.

Cargo-intent structural compilation does not execute proof providers or silently
materialize authored decisions.

Cargo-proof may plan, execute or ingest only explicitly registered providers
under the selected product contract. It does not decide authored product
direction, provider-private semantics, or final merge policy.

## Promotion law

A stronger support tier requires an explicit reviewed promotion with current:

- exact package/version identities;
- selected platform and toolchain evidence;
- deterministic schema and result contracts;
- negative controls for unavailable, partial, stale, malformed, incompatible,
  timeout, and instrument-failure cases;
- installed-candidate proof outside the source workspace where applicable; and
- documentation that states what remains NotIncluded or NotProven.

The absence of a failure in a narrower smoke is not promotion evidence.

## Maintenance

Update this file when a user-facing claim, selected package graph, compatibility
window, publication channel, or support boundary changes. Keep detailed behavior
in specifications and implementation evidence in typed receipts rather than
turning this table into a second architecture or package manifest.
