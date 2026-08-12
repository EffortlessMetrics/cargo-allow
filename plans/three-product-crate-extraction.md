---
id: CARGO-ALLOW-PLAN-0010
kind: implementation_plan
status: active
owner: repo-infra
created: 2026-07-22
updated: 2026-08-12
linked_proposal: CARGO-ALLOW-PROP-0010
linked_spec: CARGO-ALLOW-SPEC-0011
linked_adr: CARGO-ALLOW-ADR-0002
linked_package_adr: CARGO-ALLOW-ADR-0003
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
---

# Plan: Three-Product Convergence and Release

## Purpose

Record the completed physical convergence from the historical 27-package
extraction scaffold to the current 22-package architecture, then sequence the
remaining semantic and release qualification for independently qualifiable
products:

```text
cargo-allow
cargo-intent
cargo-proof
```

This is no longer a crate-creation plan. The remaining work is to make semantic
ownership, current and target package identity, dependency direction,
compatibility, retirement, CI, packaging and release evidence agree.

The historical generation-1 plan called its first stage `Wave 0`, and issue
#2598 owned the initial source-move and deletion denominator. These markers are
retained only for the strict compatibility test and are removed by #2967 when
that test consumes the generation-2 contract semantically.

## Current starting state

| Area | Starting posture |
| --- | --- |
| cargo-allow | published `0.1.11`; source workspace on unreleased `0.2.0` |
| current topology | 22 packages: 10 allow, 4 shared, 5 intent, 3 proof |
| historical scaffold | maximum 27-package graph retained as migration provenance |
| shared substrate | `effortless-*` package/path/version migration landed; dependency neutrality remains incomplete |
| cargo-intent | experimental staged-precommit vertical; canonical graph cutover incomplete |
| cargo-proof | experimental three-package family; five former packages were absorbed into modules |
| compatibility | bounded process transport exists; embedded semantic authority remains |
| candidate packaging | ambient-workspace assumptions remain |
| release | no exact candidate authorization, tag or publication |
| repository extraction | not authorized |

## Operating discipline

Every dependent stage follows one complete lifecycle:

```text
reconstruct from merged main
→ implement one bounded outcome
→ run focused proof
→ open or update the PR
→ inspect automated and human review
→ fix every valid finding
→ rerun the complete hosted matrix
→ resolve review threads
→ merge
→ verify merged main
→ create the next dependent branch from merged main
```

Branch existence, PR opening, one green job or “ready for review” is not
completion. Read-only research may run in parallel; dependent implementation
must not outrun merged authority.

## Stage A — retained generation-2 authority

### A1 — normative authority

**Owner:** #2966

Record the historical 27-package scaffold, current retained 22-package topology,
five completed package-to-module destinations, final semantic owners, shared
paths and package identities, independent release boundaries and the current
convergence train.
This PR contains retained documentation, artifact registration and exact
source-policy receipts only.

### A2 — parsed reconstruction

**Owner:** #2967  
**Entry:** A1 merged and verified.

Update strict fixtures, artifact lifecycle checks, support-tier compatibility and
contract-checked projections. Prove observed/target counts, collapse
destinations, current/target path mappings and release/extraction non-authorization
without phrase-based Wave-0 assertions.

### Exit

A fresh builder can reconstruct the destination and distinguish retained
normative authority from current implementation state.

## Stage B — strict current and target machine authority

**Owners:** #2921, #2922, #2923, in the final ownership direction from #2942.

### B1 — strict DTOs and historical reader

```text
intent-model
  strict architecture/package/move/shim/parity DTOs
  current and target identity/disposition
  local deterministic validation
  explicit HistoricalGenerationV1 reader

allow-policy
  temporary compatibility adapter only
```

Missing generation, duplicate identity, ambiguous alias and denominator mismatch
fail deterministically.

### B2 — exact observed and target Cargo closures

```text
intent-engine
  bounded Cargo metadata input
  exact package/version/source/feature/target facts
  historical 27-package scaffold provenance
  current retained 22-package closure
  shortest dependency and transition diagnostics
```

Workflow scripts run Cargo. Rust validators consume bounded artifacts. Ordinary
cargo-allow scans never spawn Cargo.

### B3 — current V2 enforcement

```text
cargo-intent / repository CI
  validate current authority
  emit deterministic typed receipt
```

V2 is selected with the current workspace and retained topology both at 22.
V1 and the former 27-package scaffold are historical input only. Register only
exact current transitions.

### Exit

One truthful denominator governs members, package identities, moves, shims,
parity, candidates and CI.

## Stage C — cargo-allow release-critical convergence

Two bounded lanes may proceed in parallel after B3 when their writers do not
overlap.

### C1 — canonical intent cutover

**Owner:** #2970

```text
selected source/authority compiler in intent-engine
→ graph-aware cargo-intent operation
→ exact old/new semantic parity
→ installed bounded process delegation
→ no fallback
→ retire embedded compiler/evaluator/query/schema/template/CI authority
→ prove cargo-allow packages without intent/proof sources
```

Only operations exposed or claimed by cargo-allow `0.2.x` are release-critical.
General cargo-intent explain, history, authoring, LSP and independent product
qualification remain separate.

### C2 — neutral selected repo-edit closure

**Owner:** #2969

```text
neutral target/request/result/failure DTOs
→ canonical underlying target and alias-convergent lock
→ final pre-replace identity recheck
→ honest selected single-target apply receipt
→ cargo-allow adapter migration
→ remove repo-edit product-domain dependency
→ prove isolated package and clean selected cargo-allow closure
```

Multi-target intent authoring remains parent work unless selected cargo-allow
behavior requires it.

### Exit

Cargo-allow no longer owns current intent semantics and its selected shared
packages contain no product-domain reverse dependency.

## Stage D — proof semantic convergence after package convergence

The physical package convergence landed through absorption in #2937 and #2938;
#2939's deletion-only alternative was superseded. Remaining semantic-owner work
does not require full cargo-proof support qualification.

```text
#2936  consume canonical intent-protocol obligations
#2943  keep proof-protocol data-oriented; proof-engine owns semantics
#2937  completed: provider host/command absorbed into retained packages
#2938  completed: cargo-allow/RIPR/Hawk providers absorbed into cargo-proof modules
#2939  superseded: deletion-only alternative was not used
```

Current package state:

```text
current packages  = 22
retained packages = 22
```

The full isolated cargo-proof candidate belongs to #2968 and does not block
cargo-allow core by default.

## Stage E — survivor package, path and version migration

**Owner:** #2885.

Entry requires selected V2 authority, observed/target topology both 22, a clean
selected shared closure and no obsolete proof package identity.

Apply one pre-publication migration:

```text
cargo-allow family                     0.2.0
shared effortless-* packages           0.1.0
cargo-intent family                     0.1.0
retained cargo-proof family             0.1.0
```

The four shared directories now use matching `crates/effortless-*` paths,
`effortless-*` dependency aliases and Cargo package names, and `effortless_*`
Rust import names. The completed migration updated Cargo manifests/lockfile, V2
authorities, scripts, fixtures, selectors, candidate builders, receipts, docs,
and CI together. No package was published by the migration.

## Stage F — exact cargo-allow candidate

**Owner:** #2886 and #2924–#2926.

```text
Complete V2 selected closure
+ exact source commit/tree/Cargo.lock
→ deterministic mixed-version package bytes
→ packaged manifest/asset/checksum inspection
→ isolated local registry
→ source-checkout and ambient-binary denial
→ exact offline install and resolved-graph equality
→ supported clean/brownfield/lifecycle journey
→ CargoAllowExactCandidateReceiptV2
```

The ambient `cargo package --workspace` plus exclusions path is not release
authority.

## Stage G — release trust closeout

Complete independently:

```text
canonical mutation target and final identity recheck
per-file current/base/head scanner completeness
canonical deterministic release payload
retained evidence reconciliation and typed Complete gate
immutable SHA-pinned release action inventory
consumer-style checksum/provenance verification
exact partial-release recovery
Trusted Publishing for the final selected graph
support, upgrade/rollback, docs and platform reconciliation
```

Then:

```text
#2501 exact candidate refreeze
→ explicit maintainer authorization naming exact commit/tree
→ #2502 publish selected packages, verify, attest and reconcile
```

A failure after first registry upload remains a release incident.

## Independent sibling-product qualification

- Broader cargo-intent commands, authoring, editor consumers and independent
  candidate/support proof continue after the minimum C1 cutover.
- #2968 independently qualifies the exact three-package cargo-proof product with
  isolated install, provider feature matrix, canonical intent obligations and a
  bounded provider journey.

## Cleanup lanes

- #2940 retires extraction-only marker structs, boundary arrays and public parity
  path locators after real APIs are selected.
- #2941 inverts `allow-report → allow-policy-legacy` through a bounded migration
  projection and may land independently when clean.

## Claim boundary

This plan records physical convergence from the historical 27-package scaffold
to the current 22-package architecture and sequences remaining semantic and exact
cargo-allow `0.2.x` qualification while preserving independent sibling-product
release gates. It does not claim remaining semantic stages complete, publish a
package, authorize a tag, or authorize repository extraction.
