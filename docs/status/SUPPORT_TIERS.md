---
id: CARGO-ALLOW-SUPPORT-0001
kind: support_tier
status: active
owner: repo-infra
created: 2026-06-12
updated: 2026-07-29
linked_proposal: CARGO-ALLOW-PROP-0010
linked_spec: CARGO-ALLOW-SPEC-0011
---

# Support Tiers

## Purpose

This file maps user-facing claims to the proof command or retained evidence a
maintainer should review. It does not promote a product merely because a crate,
binary, fixture or local package smoke exists.

Cargo-allow, cargo-intent and cargo-proof have independent support and release
posture. Registry visibility, supported direct-library use, product support,
integrated dogfood and physical repository extraction are separate decisions.

## Tier vocabulary

| Tier | Meaning |
| --- | --- |
| Stable | Current supported product behavior with a direct proof route and published or explicitly selected channel. |
| Stabilizing | Useful current behavior whose wording, output, platform matrix or adoption evidence is still maturing. |
| Experimental | Landed behavior available for development and exact-candidate proof without a stable support contract. |
| Compatibility | Bounded historical or legacy route that delegates to a canonical owner or fails explicitly. |
| Advisory | Documented direction, governance control or non-blocking evidence mapping. |
| Not included | Deliberately outside the selected product/channel claim. |

Stable and Stabilizing rows require non-empty executable proof. Experimental
rows must name the exact current boundary and cannot imply publication or
stability. Compatibility rows must name a canonical owner and explicit failure
or retirement direction.

## Current product claims

| Surface | Tier | Claim | Proof or evidence | Limitations |
| --- | --- | --- | --- | --- |
| cargo-allow published source-exception ledger | Stable | Published `cargo-allow 0.1.11` scans selected source-tree/source-syntax surfaces and checks findings against `policy/allow.toml` without executing project code. | `cargo install cargo-allow --version 0.1.11 --locked` then `cargo-allow check --mode no-new` | Applies to the published 0.1.11 command/schema/support channel, not unreleased main. |
| cargo-allow 0.2 source candidate | Stabilizing | Current main contains the operated source-exception ledger and is being qualified for an exact evidence-backed 0.2 release. | `cargo run -p cargo-allow -- check --mode no-new`, V2 gates #2921–#2923 and exact candidate #2886 | Workspace version `0.2.0` is not a tag or authorization. Architecture, package and release-trust blockers remain. |
| PR posture | Stabilizing | `cargo-allow diff --base <base>` reports source-exception posture movement for an exact meaningful base/head pair. | `cargo-allow diff --base origin/main --format markdown` | Does not prove build, tests, coverage, unsafe correctness or complete semantic reachability. |
| Worklist routing | Stabilizing | `cargo-allow worklist --format json` emits bounded source-exception repair items for humans and agents. | `cargo-allow worklist --format json` | Suggested proof commands are not commands cargo-allow executed. |
| cargo-allow mutation | Stabilizing | Selected mutation commands route through repository-contained locking and atomic single-target application. | command-specific mutation receipts and cargo-allow no-new proof | Product-neutral repo-edit, underlying target identity, collision law and final replacement recheck remain release gates. |
| Legacy staged-precommit intent route | Compatibility | The selected compatibility operation delegates one-way to installed cargo-intent through `repo.analysis-receipt.v1`, or fails explicitly. | `scripts/spec-system-cutover-receipt.sh`, #2901 transport proof and installed-candidate interop smoke | Transport is bounded; graph-aware canonical semantics, parity and embedded-authority retirement remain incomplete under #2970. |
| Historical spec-system artifacts | Compatibility | Original generations remain readable for migration and provenance where exact readers are retained. | compatibility fixtures and move/parity receipts | Historical input cannot strengthen current cargo-intent or support claims. |
| cargo-intent | Experimental | A landed read-only shell provides product identity and staged-precommit change-status behavior. | `cargo run -p cargo-intent -- identity` and `cargo run -p cargo-intent -- --format json change status --staged --phase precommit` | Canonical graph cutover, broader queries, independent candidate/support and publication are incomplete. |
| cargo-proof | Experimental | Landed protocol, planning, dry-run, provider-contract and captured-receipt scaffolding can be exercised in the workspace. | `cargo run -p cargo-proof -- identity`, selected planner commands and package tests | Five package identities remain to collapse; real selected provider composition and the independent #2968 candidate are incomplete. |
| shared repository substrate | Experimental | Four logical shared crates exist for neutral transport, source views, safe edits and Rust structural indexing. | package tests plus V2 identity/closure gates | `effortless-*` names/paths, independent versions and dependency neutrality are incomplete; direct-library support is not promised. |
| observed 27-package workspace | Advisory | The current source contains the full extraction scaffold used to test candidate seams. | current Cargo metadata and V2 observed closure after #2922 | Current existence does not ratify all packages for publication or support. |
| target 22-package topology | Advisory | The retained target keeps 10 cargo-allow, 4 shared, 5 cargo-intent and 3 cargo-proof packages. | CARGO-ALLOW-SPEC-0011 and #2934 | Becomes current only after the five proof package identities are retired under #2939. |
| integrated three-product dogfood | Advisory | The monorepo exercises a bounded cross-product journey to detect wiring regressions. | `scripts/three-product-dogfood-smoke.sh` | Workspace proximity and fake/stub stages prevent a product-support or extraction claim. |
| physical repository extraction | Not included | No current product or receipt authorizes moving the families into separate repositories. | CARGO-ALLOW-SPEC-0011 and #2559 | Requires independent package/CI/support proof, public-boundary dogfood, shim/private-path retirement, simplification review and later explicit authorization. |

## Claim boundaries

Cargo-allow source scans do not compile code, run rustc/Clippy, execute build
scripts or proc macros, call GitHub, run RIPR/Hawk, execute tests or prove unsafe
correctness.

Cargo-intent structural compilation does not execute proof providers or silently
materialize authored decisions.

Cargo-proof may plan, execute or ingest only explicitly registered providers
under the selected product contract. It does not decide authored direction,
provider-private analyzer semantics or final merge policy.

## Promotion law

A stronger support tier requires a reviewed promotion with current:

- exact package/version identities;
- selected platform and toolchain evidence;
- deterministic schema and result contracts;
- negative controls for unavailable, partial, stale, malformed, incompatible,
  timeout and instrument failure;
- installed-candidate proof outside the source workspace where applicable; and
- documentation of what remains NotIncluded or NotProven.

The absence of a failure in a narrower smoke is not promotion evidence.

## Maintenance

Update this file when a user-facing claim, selected package graph, compatibility
window, publication channel or support boundary changes. Keep detailed behavior
in specifications and typed receipts rather than turning this table into a
second architecture or package manifest.
