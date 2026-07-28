---
id: CARGO-ALLOW-PLAN-0010
kind: implementation_plan
status: active
owner: repo-infra
created: 2026-07-22
updated: 2026-07-28
linked_proposal: CARGO-ALLOW-PROP-0010
linked_spec: CARGO-ALLOW-SPEC-0011
linked_adr: CARGO-ALLOW-ADR-0002
linked_package_adr: CARGO-ALLOW-ADR-0003
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
---

# Plan: Three-Product Convergence and Release

## Purpose

Carry the landed 27-crate monorepo from transitional source ownership to three
independently understandable and independently qualifiable products:

```text
cargo-allow
cargo-intent
cargo-proof
```

This is no longer a crate-creation plan. Every logical crate in the ratified
Issue #2612 topology exists. The remaining work is to make package identity,
dependency direction, semantic authority, compatibility, deletion, CI,
packaging, and release evidence agree.

`plans/spec-system/implementation-plan.md` remains historical for the original
embedded profile. The generation-1 sequencing in CARGO-ALLOW-SPEC-0010 is
superseded by CARGO-ALLOW-SPEC-0011.

## Current starting state

| Area | Starting posture |
| --- | --- |
| cargo-allow | published `0.1.11`; source workspace on unreleased `0.2.0` |
| logical topology | all 27 crates landed |
| shared substrate | landed but generic package names, shared workspace version, and reverse product dependencies remain |
| cargo-intent | experimental staged-precommit vertical; real canonical graph cutover incomplete |
| cargo-proof | experimental protocol/planner/dry-run/adapters; real provider execution incomplete |
| compatibility | one staged operation delegates; transport hardening and broader operation cutover incomplete |
| embedded intent authority | old compiler/evaluator/schema/assets still reachable or retained |
| cargo-allow candidate | provisional ambient-workspace packaging assumptions |
| release | no exact candidate authorization, tag, registry publication, or GitHub Release |
| repository extraction | not authorized |

## Operating discipline

Every dependent stage uses one writer and one complete lifecycle:

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
→ verify merged main and issue state
→ create the next dependent branch from merged main
```

The following are progress signals, not completion:

```text
branch exists
PR opened
local tests pass
one CI job is green
ready for review
mergeable=true
```

Read-only investigation and independent review may run in parallel. Dependent
implementation does not outrun the authority generation it consumes.

## Safety gate — repair the landed process substrate

**Owner:** Issue #2883  
**May land before Stage A:** yes, because it repairs an already-merged defect.  
**May expand delegation:** no.

### Entry

- staged-precommit process delegation exists;
- piped stdout/stderr can block before child exit;
- envelope parsing and path handling have known correctness defects.

### Work

- concurrently drain stdout and stderr from process start;
- retain each stream under an explicit independent budget while continuing to
  drain excess bytes;
- keep over-budget output distinct from malformed JSON and generic instrument
  failure;
- terminate and reap the child on timeout or wait failure;
- bound reader settlement even when descendants inherit the pipes;
- pass OS-native paths to `Command::arg`;
- parse and validate one envelope exactly once;
- compare staged identity only after envelope validation;
- reject analysis-receipt requests outside JSON mode;
- run real-binary, dual-stream, over-budget, descendant-pipe, timeout, malformed,
  identity, and Windows/Linux fixtures.

### Exit

One reusable bounded provider-process runner is safe enough for later operation
cutover. No additional legacy operation delegates in this gate.

### Rollback

Revert the repair and disable staged delegation. Never restore broader cutover
on the unbounded runner.

## Stage A — retained generation-2 authority

**Owner:** Issue #2882.

### Entry

- full logical topology and walking skeletons exist;
- retained proposal/spec/plan/support language still describes crate creation,
  future products, shared publish-false assumptions, or completed cutover that
  current source does not prove.

### Work

- update CARGO-ALLOW-PROP-0010 with landed product status;
- retain CARGO-ALLOW-ADR-0002 as semantic ownership authority;
- add CARGO-ALLOW-ADR-0003 for logical/package/library identity and independent
  version lines;
- supersede CARGO-ALLOW-SPEC-0010 sequencing by exact requirement ID;
- add CARGO-ALLOW-SPEC-0011 for convergence and release behavior;
- update support tiers and current projections;
- register the successor artifacts;
- add a fresh-agent reconstruction fixture and parsed tests.

### Exit

A fresh builder can answer from retained repository authority:

- which product owns each semantic boundary today;
- which logical/package/library identities name each crate;
- which shared dependencies are still non-final;
- which old spec-system paths are canonical, compatibility-only, historical, or
  deletion targets;
- what blocks cargo-allow 0.2;
- what remains independent sibling-product work; and
- why physical repository extraction remains unauthorized.

### Deletion output

Obsolete Wave-0 sequencing ceases to be current authority. Historical files stay
readable with explicit successor links.

### Rollback

Revert the documentation generation. No package or source identity changes occur
in this stage.

## Stage B — strict identity and product-closure authority

**Owner:** Issue #2884.  
**Recommended PRs:** B1, B2, B3.

### B1 — strict generation-2 schemas

Implement checked records equivalent to:

```text
ProductCrateArchitectureV2
ProductPackageTopologyV2
CargoIdentityMapV1
```

Every crate retains:

```text
logical_id
workspace_path
cargo_package_name
rust_library_name
dependency_aliases
product_or_shared_owner
crate_role
explicit package version and source
publication/support/candidate/CI posture
```

Requirements:

- exact schema ID and numeric generation;
- missing generation is invalid;
- unknown fields fail unless explicitly compatible;
- duplicate or ambiguous identity fails;
- every workspace member is represented exactly once;
- generation-1 input is historical/migration input, never a current clean result.

### B2 — exact Cargo graph input and closure validation

Use two planes:

```text
normal source-controlled plane
  parse checked manifests without invoking Cargo

explicit build/CI plane
  repository workflow runs cargo metadata for selected feature/target sets
  Rust validators consume the bounded artifact
```

Model normal, dev, build, target-specific, optional, feature-activated,
workspace/path, registry, and process-compatibility edges. Emit shortest direct
and transitive violation paths.

### B3 — current workspace manifests and enforcement

- migrate current 27-crate authorities to V2;
- register only genuine exact transitional edges;
- expose current reverse dependencies and incomplete wiring honestly;
- enable no-new enforcement against future identity or dependency drift;
- make move, shim, parity, package, and architecture denominators agree.

### Exit

Selected package/feature/target closures resolve from real Cargo identities back
to stable logical ownership. Current non-final edges are visible and expiring.
No package name or version changes occur yet.

### Rollback

Revert V2 selection and retain generation 1 as historical. Do not proceed to the
package rename without a clean current identity denominator.

## Stage C — atomic package identity and version migration

**Owner:** Issue #2885.

### Entry

- Stage A authority is merged;
- Stage B can represent aliases, package names, library names, and independent
  versions;
- every package selector and candidate artifact consumer has been inventoried.

### Work

Apply one pre-publication migration:

```text
cargo-allow family                     0.2.0
shared effortless-* packages           0.1.0
cargo-intent family                     0.1.0
cargo-proof family                      0.1.0
```

Rename Cargo packages only:

```text
repo-protocol      → effortless-repo-protocol
repo-snapshot      → effortless-repo-snapshot
repo-edit          → effortless-repo-edit
rust-source-index  → effortless-rust-source-index
```

Retain concise logical IDs, workspace paths, dependency aliases, and Rust library
imports. Update atomically:

- all Cargo manifests and Cargo.lock;
- CI, tests, scripts, docs, and `cargo -p` selectors;
- `.crate` filename construction;
- candidate/local-registry/release order and verification;
- V2 architecture and package authorities;
- move/shim/parity/cutover identities;
- support and package metadata.

### Exit

No generic shared Cargo package identity or accidental shared `0.2.0` version
remains in current sources. No package is published. The selected cargo-allow
candidate remains blocked until dependency neutrality is proven.

### Rollback

Normal commit revert before any registry publication. After first publication,
identities are immutable and corrections require new package/version bytes.

## Stage D — shared dependency neutralization

**Owners:** Issues #2580, #2602, #2583, #2587, #2584.

### Recommended PR sequence

```text
D1  effortless-repo-snapshot removes allow-core/allow-inventory dependencies
D2  effortless-repo-edit completes neutral target/lock/write authority and
    removes allow-core
D3  effortless-rust-source-index removes allow-core
D4  intent-model removes allow-core and becomes pure authored domain
D5  selected-closure validation rejects all unexpired shared→product edges
```

Product-specific translations move to product adapters. Shared errors, results,
and identities use neutral contracts rather than cargo-allow ontology.

### Exit

Every selected shared package builds and tests without product-domain crates.
Temporary reverse edges are deleted or remain exact non-clean blockers with
expiry and removal conditions.

### Rollback

Revert one neutralization PR and keep the edge registered as a bounded transition.
Do not hide the edge to obtain a clean closure.

## Stage E — canonical intent model, protocol, engine, and application

**Owners:** Issues #2584, #2585, #2586, #2599.

### E1 — canonical authored model

Move or reconcile requirements, slices, seams, evidence purpose/mapping,
authority roles, support claims, transactions, history, and compatibility
readers into `intent-model`. Delete duplicate current definitions after parity.

### E2 — bounded public protocol

Expose stable query, view, change, obligation, diagnostic, action, edit-plan, and
settlement values. Keep raw graph nodes private.

### E3 — one read-only compiler

Move into `intent-engine`:

```text
source/dialect/authority adapters
profile and source resolution
many-source discovery
normalization and private graph compilation
exact parent/candidate comparison
impact closure and phase obligations
inference, posture reconciliation and history
bounded domain queries and caches
```

Replace the hard-coded self-hosted four-file composition with configured roots.
Keep that composition as a parity fixture only.

### E4 — real cargo-intent application

Add thin application operations over protocol values, including selected:

```text
identity
audit
change status
change diff
requirement/initiative explain
affected closure
obligations
worklist
```

CLI, rendering, exits, and TTY behavior remain in `cargo-intent`; they do not
reimplement semantic policy.

### E5 — exact semantic parity

For every selected old operation:

```text
same exact source subject
→ old result
→ new result
→ normalized semantic comparison
→ accepted parity or exact reviewed difference
```

### Exit

One authored model, one compiler, one phase evaluator, and one domain-query
implementation remain selected. Cargo-intent returns real graph-aware results.
Proof execution and repository mutation remain outside `intent-engine`.

### Rollback

Keep the old operation available only inside the explicit parity harness before
cutover. Never create a permanent in-process cargo-allow compatibility library.

## Stage F — compatibility cutover

**Owners:** Issues #2601 and #2883.

### Entry

- bounded process runner is complete;
- selected cargo-intent operation has exact installed-candidate parity;
- provider product/protocol/request/source identities are stable.

### Work

For one operation at a time:

```text
legacy cargo-allow request
→ discover exact installed cargo-intent
→ invoke structured operation
→ validate one bounded envelope
→ compare exact source identity
→ render compatibility projection
→ explicit unavailable/incompatible/stale/failure result
→ no fallback
```

Update move, shim, parity, support, package, and CI authorities with each cutover.

### Exit

Every retained compatibility operation delegates or fails explicitly. No current
intent semantic decision is made inside cargo-allow.

### Rollback

Before cutover, revert the operation adapter. After cutover, do not restore the
old evaluator; disable/deprecate the compatibility operation instead.

## Stage G — embedded current-intent deletion

**Owner:** Issue #2568.

### Delete or reclassify

- `allow-policy::spec_system` current domain/compiler/policy exports;
- cargo-allow private graph/workspace/query/precommit implementations;
- current cargo-allow-owned intent report schemas and producer registration;
- canonical intent templates/assets from the cargo-allow package;
- default cargo-allow CI claims that qualify intent implicitly;
- private source reads and dev-path dependencies that let package smokes find
  sibling product code.

Retain only explicitly historical readers, bounded compatibility projection, and
migration guidance.

### Exit

Cargo-allow builds, tests, packages, installs, and runs with intent/proof source
trees absent. Architecture checks find no second graph or phase evaluator.

### Rollback

A deleted semantic evaluator is not restored after cutover. Repair the canonical
cargo-intent operation or explicitly mark compatibility unavailable.

## Stage H — topology-selected exact cargo-allow candidate

**Owner:** Issue #2886.  
**Recommended PRs:** H1, H2, H3.

### H1 — typed candidate and mixed-version packaging

Generate an exact candidate from V2 topology and clean selected closure. Package
every and only selected logical/package/version rows and verify manifests,
dependencies, features, assets, versions, order, and `.crate` digests.

### H2 — isolated local registry and resolved graph

Build a classic local registry, deny the workspace source checkout and ambient
binary, install exact cargo-allow offline, and compare the actual resolved
package/version/checksum graph with the candidate artifact.

### H3 — complete first-hour/lifecycle journey and CI lane

Run from the isolated binary in a fresh consumer repository:

```text
--version and --help
doctor
init or propose
audit
no-new failure
why and reviewed add/apply
no-new success
list, explain and worklist
diff
refresh, prune and rollback paths selected by support matrix
```

Validate emitted artifacts and produce a typed exact-candidate receipt. Remove
`cargo package --workspace` plus exclusions from cargo-allow release
qualification.

### Exit

The exact mixed-version cargo-allow bytes compose and operate outside the
monorepo on selected platforms. No tag, upload, or public release occurs.

### Rollback

Revert candidate tooling. Do not fall back to an ambient workspace smoke as
release authority.

## Stage I — evidence-backed cargo-allow 0.2 closeout

**Owners:** Issues #2371, #2489, #2491, #2492–#2499, #2509, #2282,
#2501, and #2502.

Complete independently:

```text
canonical mutation target and final identity recheck
per-file current and base/head scanner completeness
canonical deterministic release payload
retained candidate/registry/auth/platform evidence reconciliation
typed Complete workflow gate
immutable release action inventory
consumer-style provenance/checksum verification
complete public asset closeout
exact partial-release recovery
live Trusted Publishing for the final selected package graph
support, upgrade/rollback, docs and platform reconciliation
```

Then:

```text
#2501 exact candidate refreeze
→ explicit maintainer authorization naming exact commit/tree
→ #2502 tag, publish selected packages, verify registry/install,
  attest exact subjects, publish complete release, reconcile main and issues
```

Any failure after first registry upload is a retained release incident. A later
passing rerun does not rewrite history as an initially clean release.

## Stage J — cargo-proof execution, external dogfood, simplification, extraction decision

This stage does not block cargo-allow core unless its support matrix explicitly
selects an integrated compatibility claim.

### Work

- implement real provider execution or captured-result ingestion;
- include selected command, cargo-allow, RIPR and Hawk adapters in the product;
- bind receipts to exact source/config/tool identity and currentness;
- replace fake/stub product paths with exact external RIPR dogfood;
- remove duplicated external proof planners after accepted parity;
- measure compile, dependency, support, and operational costs of every crate;
- merge back boundaries that do not earn their cost;
- prepare independent package, CI, support, and repository-extraction evidence.

### Exit

Cargo-intent and cargo-proof can run their own exact qualification trains. A
separate decision may then authorize physical repository extraction without
changing semantic ownership again.

## Global claim boundary

This plan sequences convergence from the landed monorepo through exact
cargo-allow 0.2 qualification and later sibling-product maturity. It does not
itself prove a stage complete, publish a package, authorize a tag, promote an
experimental product, or authorize physical repository extraction.
