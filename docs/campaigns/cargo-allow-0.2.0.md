# Campaign 0.2.0: Public RC Evidence, Usability, Agent Controls, and Final Release

Controlling issue: [#3768](https://github.com/EffortlessMetrics/cargo-allow/issues/3768)

## Target

- Product target: `cargo-allow 0.2.0`
- Public prerelease baseline: `0.2.0-rc.1` (usable pilot evidence with incident lineage; not reusable as final package bytes or authorization)
- Stable rollback baseline: `0.1.11`
- Experimental sibling line: `0.1.0` (`cargo-intent`, `cargo-proof`)

## Child Rails and Issue Graph

1. **Agent Control Plane**
   - [#3731](https://github.com/EffortlessMetrics/cargo-allow/issues/3731): Gemini workspace context, repository configuration, PR template
   - [#3770](https://github.com/EffortlessMetrics/cargo-allow/issues/3770): Shared Gemini/Antigravity campaign skill
   - [#3747](https://github.com/EffortlessMetrics/cargo-allow/issues/3747): Machine-visible review blocking posture

2. **Parallel Read-Only & Reconciliation Work**
   - [#3759](https://github.com/EffortlessMetrics/cargo-allow/issues/3759): RC.1 per-run and per-package reconciliation
   - [#3771](https://github.com/EffortlessMetrics/cargo-allow/issues/3771): Clean and brownfield pilot target selection

3. **Package & Checksum Authority**
   - [#3744](https://github.com/EffortlessMetrics/cargo-allow/issues/3744): Package topology integrity
   - [#3755](https://github.com/EffortlessMetrics/cargo-allow/issues/3755): Registry checksum authority

4. **Release Identity & Evidence**
   - [#3752](https://github.com/EffortlessMetrics/cargo-allow/issues/3752): One Rust release-identity authority
   - [#3761](https://github.com/EffortlessMetrics/cargo-allow/issues/3761): ReleaseManifestV2 typed identity consumption
   - [#3760](https://github.com/EffortlessMetrics/cargo-allow/issues/3760): Typed external CargoAllowReleaseAuthorizationV1

5. **Workflow & Rehearsal**
   - [#2497](https://github.com/EffortlessMetrics/cargo-allow/issues/2497), [#3724](https://github.com/EffortlessMetrics/cargo-allow/issues/3724), [#3726](https://github.com/EffortlessMetrics/cargo-allow/issues/3726)
   - [#3751](https://github.com/EffortlessMetrics/cargo-allow/issues/3751): Tag-equivalent zero-upload release rehearsal

6. **Real Installed Dogfood & Usability Pilots**
   - [#2466](https://github.com/EffortlessMetrics/cargo-allow/issues/2466): Clean or near-clean installed adoption
   - [#2467](https://github.com/EffortlessMetrics/cargo-allow/issues/2467): Brownfield installed adoption
   - [#3151](https://github.com/EffortlessMetrics/cargo-allow/issues/3151): Integrated first-hour finding-to-green experience
   - [#2485](https://github.com/EffortlessMetrics/cargo-allow/issues/2485): Diagnostic clarity improvements

7. **Final Candidate Preparation & CI Economy**
   - [#3750](https://github.com/EffortlessMetrics/cargo-allow/issues/3750): Typed candidate preparation
   - [#3773](https://github.com/EffortlessMetrics/cargo-allow/issues/3773): Manifest normalization and package contents
   - [#3774](https://github.com/EffortlessMetrics/cargo-allow/issues/3774): Fresh read-only preflight availability
   - [#3753](https://github.com/EffortlessMetrics/cargo-allow/issues/3753): Safe caching and fail-fast CI

8. **Final Release Freeze & Authorization Boundary**
   - [#2501](https://github.com/EffortlessMetrics/cargo-allow/issues/2501): Exact 0.2.0 refreeze
   - **STOP POINT**: Hard stop for external human release authorization ([#3760](https://github.com/EffortlessMetrics/cargo-allow/issues/3760))
   - [#2502](https://github.com/EffortlessMetrics/cargo-allow/issues/2502): One-time final release execution

## Immediate Next Unblocked Work

- [#3731](https://github.com/EffortlessMetrics/cargo-allow/issues/3731): Agent control plane landed
- [#3770](https://github.com/EffortlessMetrics/cargo-allow/issues/3770): Campaign execution skill in `.agents/skills/cargo-allow-0.2-campaign/`
- [#3747](https://github.com/EffortlessMetrics/cargo-allow/issues/3747): Machine-visible review blocking
- Parallel read-only [#3759](https://github.com/EffortlessMetrics/cargo-allow/issues/3759) (RC.1 reconciliation) and [#3771](https://github.com/EffortlessMetrics/cargo-allow/issues/3771) (pilot comparisons)

## Claim Boundary

This campaign map provides a structured overview of active controller #3768. Live GitHub issues own authoritative acceptance criteria and execution state.

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
acceptance. Every named row must exist, and at least one must carry one of
the named sufficient evidence classes (`StructuredShapeValidation`,
`TypedModelValidation`, `ProductionBehaviorValidation`,
`ExternalObservationValidation`, `LiveControlReadback`) — acceptance backed
only by any other class, including ones unknown to the guard, is rejected
(#3810 criterion 7). `Duplicate` requires an accepted
replacement issue; `NotPlanned` requires a bounded reason. Missing, malformed,
stale, or instrument-failure evidence posts one bounded diagnostic and reopens
the issue. The guard is scoped to the checked denominator, is idempotent, and
cannot merge, publish, tag, close issues, or change release/live controls.
