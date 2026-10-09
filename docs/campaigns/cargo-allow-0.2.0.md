# Cargo-allow 0.2.0: Current Execution Plan

Controlling issue: [#3768](https://github.com/EffortlessMetrics/cargo-allow/issues/3768).
Long-lived roadmap: [#2045](https://github.com/EffortlessMetrics/cargo-allow/issues/2045).

Observed basis: [`main@67d80f2fb4c50c8f74b1a99bfb3482c2d126deac`](https://github.com/EffortlessMetrics/cargo-allow/tree/67d80f2fb4c50c8f74b1a99bfb3482c2d126deac)
on 2026-10-09. The inspected [main CI run](https://github.com/EffortlessMetrics/cargo-allow/actions/runs/37973786441)
has 25 successful jobs; final `0.2.0` is not cut. Source CI does not establish
final qualification, a production publication operation, or public release closeout.

This document owns current sequencing and blocker classification. Child issues
own implementation acceptance; live GitHub state outranks this snapshot when
source, ownership, evidence, or a maintainer decision changes. Recommendations
below are not recorded maintainer rulings.

## Selected product and release boundary

- Product target: `cargo-allow 0.2.0`, stable channel, `v0.2.0`.
- Stable rollback baseline: `0.1.11`.
- Public pilot: `0.2.0-rc.1`, with append-only moved-tag/publication incident
  lineage. Preserve its tag, packages and retained history; none is final
  package-byte, freeze or authorization evidence.
- Final closure: ten cargo-allow-family packages at exact `0.2.0`, with
  release-coupled requirements `=0.2.0`, plus three selected shared
  prerequisites at public `0.1.0` expected registry checksums.
- Final Rust/MSRV: `1.95`. The `0.1.11` rollback build floor is separate.
- Cargo-intent and cargo-proof remain independently experimental siblings.
  Their absence must not break the cargo-allow core journey.
- The selected crates.io substrate is #3389's GitHub Actions
  `CARGO_REGISTRY_TOKEN` path. Provenance attestation and crates.io
  authentication are separate authorities.
- `0.2.0-rc.2` is not selected. Another public prerelease requires an explicit
  maintainer decision based on changed package bytes and pilot risk.
- This plan grants no tag, token, registry, GitHub Release, live-setting or
  external-pilot mutation authority. Those operations require their exact
  separate authorization and checked prerequisites.

## What has landed

The September source queue is no longer the active work list.

| Foundation | Current disposition |
| --- | --- |
| Registry observation #3850 | Credential-free adapter merged through #4287. Refresh actual observations for the selected candidate; do not build another observer. |
| Authorization #3789 / #3790 | Production model/compiler and post-tag gate exist through #4291/#4292; #4299 binds the annotated tag object. Complete production composition remains below. |
| Dependencies and security-update routing | #4204/#4170 and #4273/#4274 are resolved source lanes. #4284 closed unmerged after evidence transfer. |
| Configuration #3875 / #3876 | Shared resolution and supported-consumer cutover are complete. #3877 still owns the selection decision. |
| Default burden #3883 | The measured corpus is delivered. #3884/#3885 own the profile decision and implementation. |
| Installed candidate #3151 | #4390 delivered a nonpublishing glibc 2.35-compatible candidate and bounded installed-host journey. It did not complete external adoption or final ReleaseExperience. |
| Supporting tools #4403 | All six reported items are delivered through #4410–#4414 and #4416; the issue is closed. |

Earlier scanner-completeness work under #2486/#2492/#2493/#2494 remains accepted
within its demonstrated scope. The newly found committed-revision reader gap
needs a bounded repair of contradicted acceptance, not a restart of that train.
An open umbrella may still need final proof or truthful closeout even when its
source foundation has landed.

## Stage 1 — repair release evidence admission

These defects can turn insufficient evidence into an apparent successful
handoff. Finish them before trusting a successor final freeze.

| Owner | Required production result |
| --- | --- |
| [#4425](https://github.com/EffortlessMetrics/cargo-allow/issues/4425), under #2501/#3842 | Admit only the canonical rehearsal phase set with valid successful reversible outcomes, the documented unconsumed-authorization posture, and required zero-mutation proof. Missing, unknown, duplicate, foreign or failed phases must remain non-Complete through composition, readiness and replay. |
| [#4426](https://github.com/EffortlessMetrics/cargo-allow/issues/4426), under #2501/#3842/#3774 | Replace shared-row-count currentness with typed exact registry outcome, identity, checksums and timestamp/freshness admission. Missing, stale, malformed and provider-failed observations remain non-clean. |
| [#4427](https://github.com/EffortlessMetrics/cargo-allow/issues/4427), under #2497/#3761 | Preserve the downloaded publication receipt when the GitHub Release job reconstructs package assets. A package-only receipt must not overwrite published evidence or bypass the final manifest's publication checks. |
| [#4423](https://github.com/EffortlessMetrics/cargo-allow/issues/4423), under #3790 | Let ordinary dispatch reach its nonpublishing dry-run branch without authorization input, while denying every real tag/token/upload route without exact authorization. PR #4424 is in review at this snapshot. |

#4249 remains the separate per-field rehearsal subject-binding test gap. Tests
for the new admission defects must invoke the real consumer and distinguish the
specific rejection from an unrelated failure. A green DTO/schema test is not
production-consumer proof. These children repair existing authorities rather
than defining another release model.

## Stage 2 — finish the production release operation

The existing authorization, custody, lease and operation types are useful
foundations. Their existence and the post-tag check do not prove this complete
sequence:

1. **#3789/#3790:** assemble expected inputs from trusted freeze, package,
   support, registry, workflow and control authorities. Validate the exact
   external authorization before creating a tag or making a token reachable.
   #4423 fixes only the ordinary dispatch branch.
2. **#3940/#3927/#3925:** bind one operation identity to independently read-back
   authorization custody, durable lease and append-only single-use state.
   A caller-provided expected-context file or run-local success flag is
   insufficient authority.
3. **#3930:** create and observe one annotated tag transaction only after
   admission; verify its object identity and peeled commit/tree.
4. **#3389/#3922/#2509:** publish only frozen bytes in canonical dependency
   order. Independently observe exact registry checksum equality before
   unlocking dependent rows. Retain partial, conflicting and ambiguous outcomes
   with their original-candidate recovery lineage.
5. **#3761/#2497/#3726/#3933:** preserve publication/install evidence, produce
   the typed public manifest, verify provenance, and independently read back the
   actual draft asset set before public finalization. Draft creation plus
   upload success is not actual-asset closeout.

Ordinary PR, push and default dispatch remain nonpublishing. Skipped, cancelled,
stale, malformed or failed predecessors cannot admit a token-bearing step.
Clean publication authority cannot become recovery, containment or yank
authority. Exercise production composition through bounded fixtures before
claiming this stage complete; no live release mutation is part of implementation.

## Stage 3 — settle the supported core and first-hour contract

These current defects have existing owners. Independent work may run in
parallel when its source/evidence authority does not overlap another writer.

| Owner | Current remaining scope |
| --- | --- |
| #4349 | Make diff/worklist producers and their closed schemas agree, including emitted coverage movement and location fields. |
| #4350 | Make entry explanation preserve full-world ambiguity; evaluating one entry alone cannot establish that it authorizes the finding. |
| #4351 | Reconcile expires-today semantics across matcher, check, worklist and cadence. Record the chosen law and prove every projection against it. |
| #4345 | Preserve precise error classification and an actionable hard-error route without deriving semantics from human prose. |
| #4334 | Finish init/propose collision and existing/tracked evidence guidance. The original same-plan-path collision is repaired; recovery still needs explicit root/config and quoted-path coverage. |
| #4335 | Teach the measured initial-red behavior of ordinary repositories under the current default instead of presenting empty init as a generally green adoption path. |
| #3149 / #3882 | Reconcile the remaining operator grammar and supported help front door against the actual installed commands. |

**Committed-revision reader [#4428](https://github.com/EffortlessMetrics/cargo-allow/issues/4428),
under #2494/#3300/#1916, is a supported diff-completeness and bounded-input
blocker.** Current source inspection found unbounded batch blob output and
lossy UTF-8 decoding without the current-tree file-size limit, while revision
completeness can report no skipped Rust files. The repair must retain typed
per-file rejection/completeness and current-tree/revision parity, preserving
useful Git batching. It repairs the contradicted acceptance without reopening
the entire scanner design.

### Product and support decisions

| Decision owner | Recommendation to accept or correct | Downstream owner |
| --- | --- | --- |
| #3877 configuration | SimpleCore needs one binary/policy; advanced federation is explicit. Preserve existing selection during a concrete, visible legacy migration window. | #3878 |
| #3884 default profile | Use the #3883 burden corpus to separate syntax/high-signal blocking families from ordinary presence-only registration and test-context panic noise. Retain explicit complete-file governance when selected. | #3885 |
| #3777 support policy | Resolve all five support-matrix TODOs: maintenance window, security response, backports, platform commitment and MSRV changes. Make the smallest sustainable promise and name its transition/review trigger. | #3795 / #3796 |
| #3150 clean-pilot/claim scope | Audit a selected exact installed candidate against env-check before deciding whether it is suitable for #2466; keep adze/brownfield post-final unless explicitly promoted. | #3771 / #2466 / #3151 |

None of those recommendations changes the selected policy by itself. Existing
tests, assignment or silence are not the ruling. Product choices must state
whether they ship in final 0.2.0 or receive an explicit supported-limit/deferral
disposition. A default change requires fresh installed and upgrade proof, plus
an explicit decision if another public prerelease is warranted.

The six-repository pilot comparison did not establish a clean target. #4390's
installed-host evidence does not establish external adoption. Read-only target
audit, target selection and target writes remain distinct scopes. If a clean
pilot cannot complete, #2501/#3151 need an explicit narrowed `NotProven`
external-adoption claim, not a fabricated Complete pilot. That claim decision
must be supported by the actual receipt consumer; it cannot waive a required
Complete input merely through prose. Brownfield #2467 is post-first-final by
default.

## Stage 4 — qualify one settled final subject

After selected source and claim decisions converge, record one stabilization
baseline. Retain and reconcile:

- the exact ten-package candidate, normalized `=0.2.0` internal requirements,
  sizes and SHA-256s, plus the three shared expected registry checksums;
- candidate preparation, isolated registry install and exact resolved graph;
- first-hour/finding-to-green journey invoked through the exact installed
  binary, with identity, completeness, actions and failure artifacts;
- package/docs/help/command-registry/support coherence and selected release
  experience, including explicit external-pilot posture;
- exact `0.1.11 → final candidate → 0.1.11` upgrade/rollback evidence;
- selected platform, libc, install-channel and asset proof; unselected claims
  remain `NotIncluded` or `NotProven`;
- current typed registry observations, live release-control readbacks and
  immutable workflow/action inventory;
- the complete zero-tag, zero-token-read, zero-upload, zero-release rehearsal
  and typed prepublication manifest/evidence result;
- current independent review, required CI and no-new source guard.

Every input must bind the same selected subject or an explicitly compatible
external prerequisite. A source/package/workflow/support change stales affected
evidence. Do not copy digests or relabel predecessor receipts to repair a
mismatch. Use #3850 for fresh registry observations and the corrected
#4425/#4426 consumers for admission.

## Stage 5 — successor freeze, then exact authorization

The retained `CargoAllowFinalFreezeReceiptV1 = Complete` records
`63248416c2bd73edd63e22f064a1f242afcc0622`; its bytes and replay remain under
[the final-freeze receipts](../dogfood/receipts/final-freeze/).
Preserve them unchanged as historical evidence. They neither select current
main nor prove a successor through the newly identified admission defects.

**Recommended routing remains `SupersedeForFinal`:** finish the selected work,
requalify one later reviewed subject and retain a new successor Complete
freeze. Selecting the exact old frozen subject instead requires an explicit
maintainer decision disposing of every later selected requirement; a green
replay or desire to avoid requalification is insufficient. Record the decision
on #3768/#2501 without editing the historical receipt.

#2501 must compose the successor through the corrected production consumer.
After it becomes Complete, ordinary implementation on that subject stops.
Any selected subsequent change requires affected requalification and refreeze.

**HARD STOP:** the real #3760/#3927 authorization is supplied separately outside
the frozen tree, naming that exact freeze and operation. A Complete freeze,
green CI, tag event or broad campaign instruction is not the typed authorization.

## Stage 6 — publish, observe and reconcile once

Only after current freeze, exact authorization, custody/lease/state and live
controls are admitted, #2502:

1. refreshes registry feasibility and live-control observations;
2. creates and observes `v0.2.0` once at the authorized commit/tree;
3. loads or reproduces only the frozen package bytes and requires digest
   equality, then verifies shared prerequisite expectations;
4. reads the publication token downstream of all gates and publishes ten rows
   in dependency order, with observed equality before dependent uploads;
5. executes exact public-registry installed journeys for selected claims;
6. produces the public Complete manifest, verifies exact provenance/checksums,
   and requires actual draft-asset closeout before public release visibility;
7. publishes once and reconciles crates.io, docs.rs, stable/support/install and
   command projections, changelog, main and controlling issues.

Any failure or ambiguous response after irreversible work retains an incident.
Do not move the observed tag, rebuild from moving main, replace immutable bytes,
reuse clean authority for recovery, or erase the incident after a later success.
Source/channel promotion remains #2364/#3783 and the reviewed reconciliation
owners, driven by observed public truth.

## After final: operated 0.2.x and measured 0.3

The release ends with a usable, honestly bounded source-exception ledger.
Existing controllers own the next stages; no additional umbrella is needed.

| Stage | Existing owners | Next earned outcome |
| --- | --- | --- |
| Operated 0.2.x | #2460 / #2467 / #2483 / #3151 | Sustain clean and brownfield no-new CI, safe reviewed repair, owned rollback and retained operator-friction evidence using exact released bytes. |
| Reference measurements | #2468 / #2514 / #4374 / #4339 | Measure cold, repeated-process warm, one-file edit, policy-only, why/diff, full JSON payload and memory on retained exact subjects. The harness is now V4; preserve delivered agent-loop/payload controls. |
| Remaining scale costs | #1809 / #2515 / #2571 / #4237 | Complete only measured matcher/cache/revision/currentness gaps. Git batching and some parallel source paths already exist; inspect before extending them. |
| Machine and diagnostic coherence | #2188 / #2511 / #3887 / #3888 / #2567 | One evaluator supplies typed currentness/completeness/results and safe actions through an independently installed read-only provider. |
| Continuous feedback | #2576 → #2513 | One bounded watch/provider operation and thin editor bridge, with incremental/full parity and explicit overlay/worktree limits. |
| Evidence-led breadth and independence | #1776 / #2570 / #1875 / #2691 / #2568 / #2559 | Add useful sensors/channels from adopter evidence and retire duplicate compatibility authority without making sibling products mandatory. |

Cargo-intent 0.3 under #3941/#2163 is a separate product/version programme.
Cargo-allow 0.3 remains the measured scale/platform/editor stage in #2045.
Retained budgets, bounded memory/artifacts, cached/uncached and incremental/full
parity under stale/corrupt/partial inputs, and multiple adopter feedback loops
govern expansion. Numerical targets follow current reference measurements.
General CI economy #3753, branch hygiene #4340 and optional integrations block
0.2 only when a concrete current gap prevents or weakens selected proof.

## Session routing and completion

For each lane, reload `GEMINI.md`, `AGENTS.md`, `CLAUDE.md`, this map, #3768,
the selected child, current main, open PRs and relevant external observations.
#3731/#3770 own agent context and the campaign router; #3747 and
`review-current-head` own review-readiness consequences. Do not alter these
contracts to make the campaign appear complete.

Classify the lane as `ReversibleImplementation`, `ReadOnlyReview`,
`ExternalObservation`, `RootDecision`, `IrreversibleOperation` or
`BlockedOrStale`. Use one writer per authority, refresh ownership before
starting, and hand the finished head to independent review. After merge,
verify current main, retain required guards/evidence, reconcile the child and
controller, and clean only the lane's owned disposable work.

- [ ] #4423/#4425/#4426/#4427 and selected core regressions have current closure.
- [ ] Production pre-tag, custody, durable state, registry and actual-asset
      composition is proven under its existing owners.
- [ ] Config/default/support/pilot decisions and shipped projections agree.
- [ ] Exact installed, package/docs/upgrade/platform and external/control
      evidence is current for one settled subject.
- [ ] #2501 records routing and a corrected, selected successor Complete freeze.
- [ ] A separate exact out-of-tree authorization passes the real entry gates.
- [ ] #2502 publishes, verifies and reconciles once, preserving incident history.
- [ ] #2460/#2045 receive the operated-product and measured 0.3 handoff.

## Claim boundary

This is the current routing map from observed source to one exact qualified,
separately authorized final cargo-allow release and bounded follow-on work. It
does not choose unratified product/support policy, waive receipt requirements,
select or mutate a pilot, authorize a tag/token/upload/live-setting change,
perform publication, or declare final readiness.

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
