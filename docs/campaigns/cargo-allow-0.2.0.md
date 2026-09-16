# Cargo-allow 0.2.0: Current Execution Plan

Controlling issue: [#3768](https://github.com/EffortlessMetrics/cargo-allow/issues/3768)

Observed basis: `main@3a12a486aa4001d3d1a902749197f5f6a7ea9c80` on 2026-09-16.

This document owns campaign sequencing, current blocker classification, and stop
boundaries. Child issues own their exact acceptance criteria. Live GitHub state
outranks this snapshot when a branch, PR, external observation, or maintainer
decision moves.

## Release identity and non-negotiable boundaries

- Product target: `cargo-allow 0.2.0`.
- Public prerelease baseline: `0.2.0-rc.1`, retained as useful pilot evidence
  with append-only incident lineage. Its tag, package bytes, authorization, and
  receipts are not reusable as final `0.2.0` authority.
- Stable rollback baseline: `0.1.11`.
- Final package graph: ten cargo-allow-family packages at exact `0.2.0` plus
  three already-published shared prerequisites at exact public `0.1.0`
  checksums.
- Rust/MSRV: `1.95` for the final line.
- `cargo-intent` and `cargo-proof` remain independent experimental siblings.
  Their absence may not break the cargo-allow core journey.
- `0.2.0-rc.2` is not selected. It requires a new explicit maintainer decision
  after evidence of a package-byte defect that warrants another public pilot.
- No agent may create, move, or delete a release tag; read a publication token;
  upload or yank a crate; publish or replace a GitHub Release; mutate live
  repository settings; or modify an external pilot repository without the
  exact separate authorization for that operation.

## Current verdict

Most product, packaging, rehearsal, manifest, and freeze machinery has landed.
The remaining route is not “close every open issue.” It is:

```text
route the retained freeze
+ settle the moving source head
+ finish the live registry observer
+ finish the production authorization model and workflow gate
+ make the clean-pilot / release-claim decision
→ qualify one exact selected head
→ produce one current Complete final freeze
→ STOP
→ separately authorize one exact operation
→ publish once, verify, and reconcile
```

Three different kinds of work remain and must not be collapsed:

| Class | Meaning | Current owners |
| --- | --- | --- |
| `RootDecision` | A maintainer must select policy, scope, or external authority. | #3768/#2501 freeze routing, #4204 security-update route, #3150 pilot selection |
| `ReversibleImplementation` | Source-controlled code, schema, workflow, test, or documentation work. | #3850, #3789, #3790 |
| `Qualification` | Existing producers must be rerun against one exact final subject and current external state. | #3774, #3151, #2501 and their retained evidence inputs |
| `IrreversibleOperation` | Tag, token, registry, release, or live-setting mutation. | Separate maintainer authorization and #2502 only |

## The retained final freeze

A retained `CargoAllowFinalFreezeReceiptV1 = Complete` exists for
`63248416c2bd73edd63e22f064a1f242afcc0622`, with its exact package graph,
manifest binding, and replay evidence under
[`docs/dogfood/receipts/final-freeze/`](../dogfood/receipts/final-freeze/).
That receipt remains valid historical evidence for exactly those bytes. It must
not be rewritten or weakened merely because `main` moved.

It is not the current planned final subject. Selected release-critical work has
landed after that freeze, including the #3849/#4270 final-registry-preflight
contract, and current required work in #3850, #3789, and #3790 is absent from
the frozen source. Current dependency and evidence PRs also move package,
lockfile, or retained-evidence identity.

**Recommended maintainer decision:** preserve the retained freeze as immutable
historical evidence and supersede it for the final operation. Produce a new
freeze only after this plan’s source, decision, and qualification phases
converge.

The call changes only if the maintainer explicitly selects the exact old frozen
subject for publication and correspondingly removes every later selected
package/release-control requirement from the final denominator. Ordinary issue
closure, a green replay, or a desire to avoid requalification is insufficient.

## Critical path

```text
Phase 0  maintainer decisions
   ├─ retained freeze: supersede or deliberately select exact old subject
   ├─ #4204 security-update control
   └─ #3150 clean-pilot audit / claim posture

Phase 1  settle all source movement selected before freeze
   ├─ #4273 merge or defer
   ├─ #4274 absorb #4284 evidence, then merge or defer
   └─ #4170 resolve #4204, rebase/re-review, then merge or defer

Phase 2  remaining source implementation
   ├─ #3850 credential-free registry observation adapter
   └─ #3789 production authorization model/compiler
          ↓
      #3790 pre-tag / pre-token workflow enforcement

Phase 3  installed usability and external-claim evidence
   ├─ exact-candidate installed journey
   ├─ #3150 initial real-repository audit decision
   ├─ #2466 clean pilot or explicit NotProven claim narrowing
   └─ #3151 final release-experience result

Phase 4  one exact selected-head qualification
          ↓
Phase 5  new #2501 Complete freeze
          ↓
        HARD STOP
          ↓
Phase 6  separate human authorization → #2502 publication and closeout
```

## Phase 0 — record the maintainer decisions

### 0A. Route the retained freeze

Record on #3768/#2501 one closed decision:

```text
SupersedeForFinal
  preserve the 63248416 freeze and replay as historical evidence
  select a later reviewed head after the remaining work below
  produce fresh package bytes, evidence, observations, and freeze

SelectExactRetainedFreeze
  name 63248416 as the release subject
  explicitly defer every later selected package/control change
  refresh only the observations and controls the retained contract permits
```

The recommended result is `SupersedeForFinal` in substance. Do not mutate the
retained receipt to record the decision; add a separate campaign/freeze
handoff.

### 0B. Decide the security-update route — #4204

PR #4170 correctly adds 14-day cooldowns for ordinary Cargo and GitHub Actions
version updates, but automatic Dependabot security updates are currently
disabled. A cooldown exemption does not create a security-update job.

Recommended decision: explicitly authorize enabling Dependabot security
updates, verify the live readback, correct the source comment/PR wording, then
rebase and freshly review #4170. The accepted alternative is a documented
prompt-remediation control with a named owner and evidence path. Leaving the
route implicit blocks #4170.

This document does not authorize the live setting change.

### 0C. Decide the clean-pilot route — #3150

The merged six-repository comparison does not establish a clean target.
`EffortlessMetrics/env-check` is the smallest initial-audit candidate, but its
panic/assertion-shaped test plumbing has not been measured by an exact installed
cargo-allow audit. `adze` remains the proposed materially different brownfield
target and is deliberately post-final by default.

Recommended decision:

1. authorize a read-only exact-candidate audit of `env-check` when the selected
   candidate identity is available;
2. do not authorize policy, workflow, source, branch-protection, or other target
   mutation through that read-only audit;
3. after the measured result, either select `env-check` for #2466 with exact
   allowed writes and rollback scope, select a replacement, or accept an
   explicit `NotProven` low-friction-adoption claim;
4. retain `adze` as `NotIncludedBeforeFirstFinal` unless a separate decision
   promotes it.

Agents may prepare the decision packet and audit plan. They may not infer
consent or mutate either repository.

## Phase 1 — settle the moving head

No qualification or freeze result is current while selected source movement is
unresolved.

| Item | Current posture | Required disposition before qualification |
| --- | --- | --- |
| #4273 `yaml-rust2` 0.13 | Open, non-draft dependency PR | Complete current-head review and CI, then merge; or close/defer explicitly until after 0.2.0. |
| #4274 `jsonschema` 0.56 | Open, non-draft dependency PR with breaking upstream API changes | Transfer and verify the generated retained evidence, review the actual consumer/API impact, then merge; or close/defer explicitly. |
| #4284 one-shot evidence carrier | Draft, explicitly non-mergeable | Use only to generate/transfer exact #4274 evidence. Close after the target branch contains and verifies the intended fast-forward result. Never merge it. |
| #4170 Dependabot cooldown | Draft, blocked on #4204 and based on an older main | Resolve #4204, rebase onto current main, refresh exact-pair validation/review, then merge; or defer the cooldown. |

After these dispositions, record one stabilization baseline. From that point,
any selected change to shipped source, package manifests, `Cargo.lock`,
topology, support/channel claims, release documents, schemas, workflows,
authorization logic, or evidence producers invalidates the affected
qualification and freeze inputs.

## Phase 2 — finish the remaining source implementation

### 2A. Credential-free registry observation — #3850

Implement one bounded adapter over the merged #3849 production evaluator.

It must:

- load the exact ten final upload rows and three shared prerequisite rows from
  real candidate/shared authorities rather than caller-supplied lookalikes;
- query supported read-only crates.io/index/API surfaces with bounded retries,
  timeouts, response sizes, and retained provider identity;
- preserve exact-version visibility, yank posture, registry checksum, owner
  observations when safely available, collection time, referenced response
  bytes/digests, missing inputs, malformed rows, and surplus observations;
- distinguish name unavailable, version missing, propagation delay, rate limit,
  malformed provider data, provider unavailable, immutable conflict, and
  residual permission risk;
- call the production evaluator and validate the emitted canonical receipt
  against the checked schema;
- expose a bounded freshness policy and the smallest required refresh action;
- never read `CARGO_REGISTRY_TOKEN`, Cargo credential files, environment dumps,
  or private/unbounded provider content; and never upload, mutate owners, tag,
  authorize, or create a release.

The resulting observation is consumed by #3774, #2501, #3789, #3790, and
#2502. Run it during candidate qualification, refresh after final bytes are
frozen, refresh immediately before authorization, and refresh again before the
token boundary.

### 2B. Production exact authorization contract — #3789

Move the authorization authority out of test-local characterization and into
production release evidence/domain code.

The production contract must bind:

- operation `publish_cargo_allow_final_0_2_0`;
- repository, `0.2.0`, `v0.2.0`, stable channel, and
  `github_prerelease=false`;
- exact freeze, commit, tree, `Cargo.lock`, topology, and ordered 13-row graph;
- ten final package names, versions, sizes, and digests;
- three shared prerequisite identities and expected registry checksums;
- package/docs, registry preflight/freshness, support/platform/channel/assets,
  prepublication manifest, rehearsal, source/live controls, and workflow/action
  inventory;
- selected authentication class without credential material;
- exact structured maintainer source reference, actor, statement digest,
  issued/expiry or one-run scope, nonce, and clean-publication operation class;
- deterministic canonical bytes/digest and closed non-clean result classes.

Arbitrary prose, assignment, issue state, a tag, a Complete freeze by itself,
RC.1 evidence, another head, or digest-shaped copied strings are not
authorization. The real authorization remains outside the frozen tree. #3789
uses synthetic fixtures only and performs no external operation.

### 2C. Enforce authorization before tag/token reachability — #3790

After #3789’s production contract is stable, wire the checked release path so
that it executes this order:

```text
load exact final freeze
→ load exact external authorization
→ validate operation, release identity, subject, package graph, evidence,
  controls, workflow inventory, currentness, expiry, and one-use posture
→ permit tag-create planning
→ permit CARGO_REGISTRY_TOKEN lookup
→ permit ordered publication
```

Implement and fixture-test the append-only operation transition:

```text
Available
→ SelectedForRun
→ IrreversibleOperationStarted
→ ConsumedComplete | ConsumedIncident
```

Ordinary PR, push, and default `workflow_dispatch` paths must remain zero-token
and nonpublishing. A skipped, cancelled, stale, malformed, or failed predecessor
cannot reach the token step. Clean publication authority cannot become recovery,
yank, or containment authority.

### Parallelization

#3850 and #3789 may proceed in parallel when each has one semantic writer.
#3790 follows the stable #3789 contract. Do not create parallel models,
adapters, state vocabularies, or workflow gates for the same authority.

## Phase 3 — finish installed usability and external claim evidence

### 3A. Exact installed candidate journey

Run the existing package/install/journey producers against the selected final
head and invoke the installed binary by absolute path. Preserve exact package,
binary, commit/tree, lockfile, topology, platform/toolchain, and invocation
identities. A checkout binary, `cargo install --path`, ambient `PATH`, RC.1, or
another internal version cannot satisfy the result.

### 3B. Clean external pilot — #3150/#2466

After the read-only target audit and maintainer selection, execute only the
explicitly authorized target-repository changes. The pilot must prove:

- no fake baseline debt on a clean path;
- a usable `adopt → doctor/audit → no-new` first hour;
- one deliberate in-scope finding turns the gate red;
- repair/removal returns it to green;
- failure artifacts remain available;
- rollback touches only adoption-owned files and preserves an unrelated file;
- every defect, documentation gap, missing capability, supported limitation,
  repository decision, and instrument failure is separately dispositioned.

If no suitable target or authorization is available, record `NotProven` and
remove the low-friction external-adoption claim from the release. `NotProven`
may be an accepted claim-narrowing decision; it is not a Complete pilot and may
not be rendered as one.

### 3C. Final release experience — #3151

Compile one current `CargoAllowReleaseExperienceV1` from the exact installed
candidate, help/docs/support/command-registry coherence, first-hour and
finding-to-green journey, selected clean-pilot result or explicit NotProven
posture, and friction dispositions. Keep the candidate result separate from the
post-publication public-final observation.

The brownfield pilot #2467 remains `NotIncludedBeforeFirstFinal` by default and
runs against exact public `0.2.0` after #2502. It does not silently satisfy
clean-pilot evidence and does not block the first upload unless a maintainer
promotes it.

## Phase 4 — qualify one exact selected head

Every required input must name the same selected repository subject or an
explicitly compatible external prerequisite. At minimum retain and reconcile:

- candidate-preparation result and exact source/target identities;
- exact ten-package `.crate` set, normalized manifests, sizes, SHA-256s, and
  release-coupled `=0.2.0` requirements;
- the three shared `0.1.0` expected/observed registry checksums;
- isolated local-registry install and resolved graph;
- exact installed first-hour/finding-to-green journey;
- package/docs, included assets, crate-doc warning-clean, and support/channel
  coherence;
- exact `0.1.11 → final candidate → 0.1.11` upgrade/rollback evidence;
- current #3850 registry feasibility observation;
- current release experience and explicit external-pilot posture;
- full zero-tag, zero-token-read, zero-upload, zero-release rehearsal;
- source and live release-control readbacks plus immutable workflow/action
  inventory;
- selected platform, install-channel, asset, and supported-limitation matrix;
- ReleaseManifestV2 prepublication result, never fabricated public Complete;
- full required CI, current exact-head independent review, and no-new source
  guard.

A passing predecessor from another commit, old freeze, old lockfile, different
package bytes, stale external observation, or changed workflow/control identity
is non-current. Do not repair mismatch by copying digests or relabeling
results.

## Phase 5 — produce the current final freeze and stop

On the settled, reviewed, qualified subject, compose one new
`CargoAllowFinalFreezeReceiptV1 = Complete` that binds the exact source,
package, evidence, external observation, support, workflow, and control
identities above.

Preserve the earlier `63248416` freeze and its replay unchanged. Record the new
freeze as the selected successor for the final operation; do not erase or
rewrite the earlier lineage.

The freeze operation performs no tag creation, token lookup, registry mutation,
attestation, external pilot mutation, live-setting change, or GitHub Release
operation.

After Complete:

```text
HARD STOP
```

Ordinary implementation on the frozen subject stops. Any selected byte- or
meaning-changing repair requires fresh affected qualification and a new freeze.

## Phase 6 — separately authorize, publish, verify, and reconcile

A maintainer separately supplies the real structured authorization outside the
frozen tree, naming the exact new freeze and operation. The #3790 gate validates
it before tag or token reachability.

Then #2502 performs one operation:

1. refresh registry feasibility and live controls;
2. create annotated `v0.2.0` once at the authorized commit;
3. verify the tag peels to the exact authorized commit/tree;
4. load or reproduce only the authorized `.crate` bytes and require digest
   equality;
5. verify the three shared prerequisites against retained expectations;
6. read `CARGO_REGISTRY_TOKEN` only after every prior gate passes;
7. publish the ten final rows in dependency order;
8. require observed registry checksum equality before advancing dependents;
9. run exact public-registry install and supported journeys;
10. produce the public Complete manifest, attestations, checksums, and selected
    assets from the exact tag;
11. keep the GitHub Release draft/private until actual-asset reconciliation is
    Complete;
12. publish once, then reconcile crates.io, docs.rs, support/docs/install
    guidance, main, and controlling issues.

Any failure after the first irreversible row becomes append-only release
incident evidence. Do not move the tag, rebuild from moving `main`, replace
immutable bytes, reuse clean authority for recovery, or let a later green retry
erase the incident.

## Follow-ups that do not automatically block 0.2.0

- #4271: UB Review must retain deferred required-proof obligations in gate
  summaries. Treat it as a release blocker only if that review proof class is
  explicitly selected as required final evidence.
- #3753: CI caching/economy work is semantic release work only when current
  reliability or cost prevents or weakens selected proof. Any workflow or
  evidence-producer change still lands before freeze or invalidates it.
- #2467: the brownfield operated-product pilot remains a post-final gate by
  default.
- Broad backlog, optional sibling-product work, editor/LSP work, and issue-count
  cleanup are not the release denominator.
- Open umbrellas whose production acceptance is already landed need truthful
  reconciliation/closeout; their open state alone is not implementation work.

## Session routing

At the start of every campaign session:

1. reload `GEMINI.md`, `AGENTS.md`, `CLAUDE.md`, this document, #3768, the
   selected child, current `main`, open PRs, and relevant external state;
2. classify the lane as `ReversibleImplementation`, `ReadOnlyReview`,
   `ExternalObservation`, `RootDecision`, `IrreversibleOperation`, or
   `BlockedOrStale`;
3. select only an unblocked owner with no active competing writer;
4. post/update a bounded execution packet when the issue body is stale;
5. keep one semantic authority per PR and hand the final head to
   `review-current-head`;
6. treat pending, skipped, stale, quota-limited, unavailable, or
   non-discriminating evidence as non-clean;
7. after merge, verify synchronized `main`, rerun affected guards, update the
   child and #3768, and route the next lane;
8. stop at every root-decision and irreversible boundary.

## Completion checklist

### Decisions and source convergence

- [ ] #3768/#2501 records whether the retained freeze is superseded or selected.
- [ ] #4204 selects and proves the security-update control; #4170 is merged or deferred.
- [ ] #3150 records the read-only audit and target/claim decision.
- [ ] #4273 is merged or explicitly deferred.
- [ ] #4274 receives #4284 evidence and is merged or explicitly deferred; #4284 is closed unmerged.
- [ ] A stabilization baseline is recorded.

### Remaining implementation

- [ ] #3850 production registry observer is merged and current.
- [ ] #3789 production authorization model/compiler is merged and current.
- [ ] #3790 pre-tag/pre-token gate and replay law are merged and current.

### Qualification and release

- [ ] Exact installed candidate and package/docs/upgrade evidence are current.
- [ ] Clean pilot completes or the final claim is explicitly narrowed to NotProven.
- [ ] #3151 release-experience result is current.
- [ ] Current registry, rehearsal, manifest, support, workflow, and control evidence agree.
- [ ] A new selected `CargoAllowFinalFreezeReceiptV1 = Complete` is retained.
- [ ] Ordinary work stops at the authorization boundary.
- [ ] A separate maintainer authorization names the exact freeze and operation.
- [ ] #2502 publishes, verifies, and reconciles once without rewriting incident history.

## Claim boundary

This campaign plan routes the remaining work from current `main` and the
retained historical freeze to one newly selected, exact, separately authorized
cargo-allow `0.2.0` operation. It distinguishes source implementation,
maintainer decisions, current-subject qualification, irreversible execution,
and post-release proof. It does not itself select a pilot, change a live
setting, invalidate or authorize a freeze, create a tag, read a token, publish a
crate, or publish a GitHub Release.

## Closeout contract

The checked active-child denominator is enforced by
`.github/workflows/campaign-issue-closeout.yml`. Before closing a checked child,
the issue body must retain one bounded `CampaignIssueCloseoutV1` payload after
the marker below:

~~~text
<!-- cargo-allow:campaign-closeout.v1 -->
~~~
```json
{
  "schema_id": "cargo-allow.campaign-issue-closeout.v1",
  "issue": 3846,
  "result": "Complete",
  "closeout_id": "CARGO-ALLOW-CLOSEOUT-3846",
  "merged_pr": 3854,
  "evidence_surfaces": ["typed-surface-id"]
}
```

`Complete` requires a merged PR targeting `main` whose merge commit remains
reachable from `main`, and a non-empty `evidence_surfaces` list naming the
checked `policy/evidence-surface-inventory.toml` rows that back the issue's
acceptance. Every named row must exist, and at least one must carry one of the
named sufficient evidence classes (`StructuredShapeValidation`,
`TypedModelValidation`, `ProductionBehaviorValidation`,
`ExternalObservationValidation`, `LiveControlReadback`). Acceptance backed only
by any other class, including one unknown to the guard, is rejected. `Duplicate`
requires an accepted replacement issue; `NotPlanned` requires a bounded reason.
Missing, malformed, stale, or instrument-failure evidence posts one bounded
diagnostic and reopens the issue. The guard is scoped to the checked
denominator, is idempotent, and cannot merge, publish, tag, close issues, or
change release/live controls.
