---
id: CARGO-ALLOW-SUPPORT-0001
kind: support_tier
status: active
owner: repo-infra
created: 2026-06-12
updated: 2026-08-12
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
| Stable | Current supported product behavior with a direct proof route and published or explicitly selected channel. |
| Stabilizing | Useful current behavior whose wording, output, platform matrix, or adoption evidence is still maturing. |
| Experimental | Landed behavior available for development and exact-candidate proof without a stable support contract. |
| Compatibility | Bounded historical or legacy route that delegates to a canonical owner or fails explicitly. |
| Advisory | Documented direction, governance control, or non-blocking evidence mapping. |
| Not included | Deliberately outside the selected product/channel claim. |

Stable and Stabilizing rows require non-empty executable proof. Experimental
rows must name the exact current boundary and cannot imply publication or
stability. Compatibility rows must name a canonical owner and explicit failure
or retirement direction.

## Current product claims

| Surface | Tier | Claim | Proof command | Limitations |
| --- | --- | --- | --- | --- |
| cargo-allow published source-exception ledger | Stable | Published `cargo-allow 0.1.11` scans selected source-tree/source-syntax surfaces and checks findings against `policy/allow.toml` without executing project code. | `cargo install cargo-allow --version 0.1.11 --locked` then `cargo-allow check --mode no-new` | Applies to the published 0.1.11 command/schema/support channel, not unreleased main. This is a bounded lexical scan, not complete source-language coverage: documented gaps include conditional-compilation handling, path-qualified selectors, BOM-prefixed attributes, and non-UTF-8 input handling. |
| cargo-allow 0.2 source candidate | Stabilizing | Current main contains the operated source-exception ledger and is being qualified for an exact evidence-backed 0.2 release. | `cargo run -p cargo-allow -- check --mode no-new`, V2 gates #2921–#2923 and exact candidate #2886 | Workspace version `0.2.0` is not a tag or authorization. Architecture, package, and release-trust blockers remain. |
| PR posture | Stabilizing | `cargo-allow diff --base <base>` reports source-exception posture movement for an exact meaningful base/head pair. | `cargo-allow diff --base origin/main --format markdown` | Does not prove build, tests, coverage, unsafe correctness, or complete semantic reachability. |
| Worklist routing | Stabilizing | `cargo-allow worklist --format json` emits bounded source-exception repair items for humans and agents. | `cargo-allow worklist --format json` | Suggested proof commands are not commands cargo-allow executed. |
| cargo-allow mutation | Stabilizing | Selected mutation commands route through repository-contained locking and atomic single-target application. | command-specific mutation receipts and cargo-allow no-new proof | Product-neutral repo-edit, underlying target identity, collision law, and final replacement recheck remain release gates. |
| Multi-ledger federation | Advisory | Same-repository canonical, mirror, and imported ledger roles use deterministic precedence and expose dialect, provenance, divergence, and drain-window state without silently merging competing views. | `cargo-allow check --profile spec-system --mode audit` and `cargo-allow check --deny mirror_divergence` | Federation is bounded to the implemented same-repository lanes; imported evaluation, external federation, full import mode, release readiness, and support promotion remain outside this claim. |
| Legacy staged-precommit intent route | Compatibility | The selected compatibility operation delegates one-way to installed cargo-intent through `repo.analysis-receipt.v1`, or fails explicitly. | `scripts/spec-system-cutover-receipt.sh`, #2901 transport proof, and installed-candidate interop smoke | Transport is bounded; graph-aware canonical semantics, parity, and embedded-authority retirement remain incomplete under #2970. |
| Historical spec-system artifacts | Compatibility | Cargo-intent owns current intent authority; exact retained historical readers may read original generations for migration and provenance. | compatibility fixtures and move/parity receipts | An unavailable reader or unsupported historical generation fails explicitly as historical input unavailable/unsupported; no current evaluator fallback is permitted, and readers retire when their migration evidence window closes. |
| cargo-intent | Experimental | A landed read-only shell provides product identity and staged-precommit change-status behavior. | `cargo run -p cargo-intent -- identity` and `cargo run -p cargo-intent -- --format json change status --staged --phase precommit` | Canonical graph cutover, broader queries, independent candidate/support, and publication are incomplete. |
| cargo-proof | Experimental | Landed protocol, planning, dry-run, provider-contract, and captured-receipt scaffolding can be exercised in the workspace. | `cargo run -p cargo-proof -- identity`, selected planner commands, and package tests | Real selected provider composition, semantic-owner convergence, and the independent #2968 candidate are incomplete. |
| shared repository substrate | Experimental | Four `effortless-*` crates exist with independent `0.1.0` versions for neutral transport, source views, safe edits, and Rust structural indexing. | package tests plus V2 identity/closure gates | Dependency neutrality remains incomplete; direct-library support is not promised. |
| current 22-package workspace topology | Advisory | Current source and the retained topology both contain exactly 10 cargo-allow, 4 shared, 5 cargo-intent, and 3 cargo-proof packages. | `Cargo.toml`, `policy/product-crates-v2.toml`, and `policy/product-package-topology-v2.toml` | Package-count convergence does not establish semantic ownership, dependency neutrality, publication, or support. |
| historical 27-package extraction scaffold | Compatibility | The former 27-package maximum scaffold remains migration provenance for the five proof-package absorptions. | CARGO-ALLOW-PROP-0010, CARGO-ALLOW-SPEC-0011, #2937, and #2938 | This is historical evidence, not a description of current source or a supported package set. |
| integrated three-product dogfood | Advisory | The monorepo exercises a bounded cross-product journey to detect wiring regressions. | `scripts/three-product-dogfood-smoke.sh` | Workspace proximity and fake/stub stages prevent a product-support or extraction claim. |
| physical repository extraction | Not included | No current product or receipt authorizes moving the families into separate repositories. | CARGO-ALLOW-SPEC-0011 and #2559 | Requires independent package/CI/support proof, public-boundary dogfood, shim/private-path retirement, simplification review, and later explicit authorization. |

## Command maturity

User-facing command guides use this table as their maturity source. `Published
0.1.11` is the frozen command channel described by
[`published-command-registry.toml`](../dogfood/fixtures/getting-started/published-command-registry.toml).
`Current main` describes the source candidate and is not a publication claim.

| Command | Published 0.1.11 | Current main | Primary guide |
| --- | --- | --- | --- |
| `init` | Stable | Stabilizing | [Adopt cargo-allow](../how-to/adopt-cargo-allow.md) |
| `audit` | Stable | Stabilizing | [Adopt cargo-allow](../how-to/adopt-cargo-allow.md) |
| `check` | Stable | Stabilizing | [Run in CI](../how-to/run-in-ci.md) |
| `diff` | Stable | Stabilizing | [Review PR posture](../how-to/review-pr-posture.md) |
| `list` | Stable | Stabilizing | [Explain an allow entry](../how-to/explain-an-allow.md) |
| `explain` | Stable | Stabilizing | [Explain an allow entry](../how-to/explain-an-allow.md) |
| `why` | Stable | Stabilizing | [Explain why a finding is unreceipted](../how-to/explain-why-a-finding.md) |
| `add` | Stable | Stabilizing | [Manage an exception](../how-to/manage-an-exception.md) |
| `propose` | Stable | Stabilizing | [Adopt no-new-debt](../how-to/adopt-no-new-debt.md) |
| `worklist` | Stable | Stabilizing | [Feed agent worklists](../how-to/feed-agent-worklists.md) |
| `migrate` | Stable | Stabilizing | [Migrate from xtask](../how-to/migrate-from-xtask.md) |
| `refresh` | Stable | Stabilizing | [Manage an exception](../how-to/manage-an-exception.md) |
| `prune` | Stable | Stabilizing | [Prune stale allows](../how-to/prune-stale-allows.md) |
| `doctor` | Stable | Stabilizing | [Adopt cargo-allow](../how-to/adopt-cargo-allow.md) |
| `adopt` | Not included | Experimental | [Adopt cargo-allow](../how-to/adopt-cargo-allow.md) |
| `capabilities` | Not included | Experimental | [Getting started](../getting-started.md) |
| `vocabulary` | Not included | Experimental | [Source exception ledger](../source-exception-ledger.md) |
| `tool` | Not included | Experimental | [Run in CI](../how-to/run-in-ci.md) |
| `completions` | Not included | Experimental | [Install shell completions](../how-to/install-shell-completions.md) |
| `reference` | Not included | Experimental | [Getting started](../getting-started.md) |
| `hooks` | Not included | Experimental | [Run in CI](../how-to/run-in-ci.md) |

The table intentionally separates published stability from current-main
maturity. A guide marker must not be read as a claim that the source candidate
is already published or that a command proves build, runtime, or semantic
coverage outside cargo-allow's source-tree scan boundary.

## Claim boundaries

Cargo-allow source scans do not compile code, run rustc/Clippy, execute build
scripts or proc macros, call GitHub, run RIPR/Hawk, execute tests, or prove unsafe
correctness.

Cargo-intent structural compilation does not execute proof providers or silently
materialize authored decisions.

Cargo-proof may plan, execute, or ingest only explicitly registered providers
under the selected product contract. It does not decide authored direction,
provider-private analyzer semantics, or final merge policy.

## Promotion law

A stronger support tier requires a reviewed promotion with current:

- exact package/version identities;
- selected platform and toolchain evidence;
- deterministic schema and result contracts;
- negative controls for unavailable, partial, stale, malformed, incompatible,
  timeout, and instrument failure;
- installed-candidate proof outside the source workspace where applicable; and
- documentation of what remains NotIncluded or NotProven.

The absence of a failure in a narrower smoke is not promotion evidence.

## Maintenance

Update this file when a user-facing claim, selected package graph, compatibility
window, publication channel, or support boundary changes. Keep detailed behavior
in specifications and typed receipts rather than turning this table into a
second architecture or package manifest.
