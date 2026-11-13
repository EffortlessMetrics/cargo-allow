# Ordinary CI through the native merge queue

Owner: [#4602](https://github.com/EffortlessMetrics/cargo-allow/issues/4602),
under #3753/#3731. Steven's 2026-11-04 ruling selects native queue qualification
with zero development Actions. This replaces the earlier explicit-request/direct
squash proposal and the periodic main-health proposal.

## Source contract and activation boundary

This source generation is **step 1**: it adds the `merge_group` trigger and the
exact-G proof path while retaining the existing `pull_request` and `push: main`
triggers. It does not remove those triggers, activate a repository rule, prove
an enqueue operation, replace the trusted H publisher, or authorize a release.
Keep it unmerged until the coordinating owner has a satisfiable source/settings
migration. Do not remove an existing required context before its replacement
and admission path have been qualified together.

Because the retained triggers are still live, a `push: main` run still starts
`ci.yml`'s `queue-subject` job and, when `proof-route` stays wide for a push
event, the ordinary proof jobs. A `pull_request` run still starts the same
graph. Step 2 removes those triggers only after one real merge-group run and one
queue landing have been observed; until then, ordinary main updates do replay
proof and their results must be read back.

Development commit, push, PR open/synchronize/reopen, Draft, Ready, labels and
body edits allocate no Actions jobs, including metadata wrappers, **only after
step 2 lands**. The owning session runs focused checks, substantive review,
repairs and current product judgment. Explicit deep/audit and release operations
retain their distinct purposes and authority.

## H, G and M answer different questions

1. **H — selected source and conditional authority.** The owner records the
   objective, exact reviewed head, base/merge-base, applicable proof, material
   findings and product/scope judgment. Readiness is UI state. Before enqueue,
   verify that the observed head is still H and the current authorization
   permits native protected landing if G qualifies. A retained label or earlier
   review cannot authorize a successor or suppress a new finding.
2. **G — actual integration candidate.** GitHub constructs the merge group from
   the queue's base and predecessors. `merge_group: checks_requested` is the
   only ordinary proof trigger. Every checkout uses the event's immutable
   `merge_group.head_sha`; dependency and feature routing use its immutable
   `base_sha`. No producer substitutes bare H or a newly fetched moving main.
3. **M — observed protected landing.** The native queue lands or removes and
   recomposes the candidate. Enqueueing is conditional landing permission; do
   not invent a manual pause after G becomes green. The owner reads back the
   actual main commit/tree, associates it with the queue receipt, reconciles
   the objective and retains any residual owner/action. A successful enqueue
   response or green G alone is not a completed objective.

Changed composition needs a new G. New blocking findings at unchanged H require
withdrawal and readback. Separate reads and removal are not an atomic lock: if
landing wins the race, record actual M and route the required repair. Do not
claim that changing Draft state alone revoked an in-flight queue operation.

## Workflow and required-check mapping

The table is the proposed source mapping, **not a live settings readback**.
The coordinating owner must enumerate actual current required contexts and
accepted publisher identities before applying the generation. Preserve any
additional required context until its obligation has a reviewed disposition.

| Existing surface | This source generation | Enforcement disposition |
| --- | --- | --- |
| `ci.yml` PR/draft/main proof | `merge_group` with exact G checkout, plus the retained `pull_request` and `push: main` triggers until step 2 | Require `queue-qualification` on the actual G after admission/control qualification. Retained main-push and PR runs still execute the proof graph and must be read back. |
| `pregate` | Same syntax/fmt/no-new denominator, now bound to G/base | Required dependency of `queue-qualification`; failed/skipped result cannot pass it. |
| `msrv`, `package-smoke`, `shallow-diff-smoke`, `operator-latency`, `test`, `feature-configuration-proof`, `test-core-platforms`, `compat-delegation`, `coverage`, `cargo-deny` | Existing mandatory commands, platform matrices and toolchain requirements retained | All are dependencies of `queue-qualification`, derived from `docs/ci-lanes.toml`; matrix jobs must succeed. |
| New `queue-subject` | Verify event/repository/ref, fetched base ancestry, exact G checkout/tree and run/attempt; execute local regression controls | Required dependency of all `ci.yml` producers and the aggregate. |
| Workflow syntax/security/construction and other existing informational CI lanes | Still execute on G with their original policy posture | Do not silently promote sibling/advisory failures into cargo-allow product gates; preserve any actual live extra requirement during migration. |
| `sentinel.yml` / `sentinel` | Independent `merge_group` root parsing the actual G workflows | Require separately; its independent root preserves a signal when `ci.yml` is malformed. |
| `frozen-subject-lock.yml` / `frozen-subject-lock` | Existing freeze check on G | Require separately; no freeze/publication obligation is waived. |
| `ub-review.yml` / `changie-contract`, `changelog-hook-contract` | Deterministic review/changelog contracts on G | Require separately; Go/Changie/module identity and nonmutation tests are retained. |
| `review-readiness.yml` publisher | Retained with its `push` and `merge_group` triggers; it resolves the PR from the merge group. Typed review projection/disposition code retained | Replace through an observed, trusted H admission/publisher join before retiring any live requirement. No green substitute is emitted by this patch. |
| Model-backed `ub-review.yml` / `review` | Actions job removed | Review and disposition remain in-session obligations under #3977/#4252/#4255. Missing, quota-limited or failed review is not clean. |
| Feature-configuration full union and sensor-burden audit | Explicit `workflow_dispatch` only | Separate deep/audit requests; no recurring main proof or development-event bridge. |
| Floor-receipt regeneration | Existing explicit dispatch | Existing exact-source regeneration purpose/owner retained. |
| `release.yml`, `release-authorized.yml` | Existing tag/authorization-path triggers and release producers unchanged | Separate release authority, immutable versions, package/MSRV/platform/freeze requirements remain. |
| Campaign closeout issue guard | Existing issue-close observation | Does not request ordinary product qualification. |

The queue result refuses missing, extra, malformed, failed, cancelled, skipped
or neutral mandatory job records. It wraps #3358's existing required portfolio;
it does not make experimental siblings required. Independent guard workflows
still need their own protected contexts. A context name alone is insufficient:
verify its actual publisher and G/run identity in the activated generation.

## The missing H control join

Removing the PR-event publisher is not evidence that H authorization is
enforced. This patch deliberately does not turn a job skipped on PR events into
a green admission check or run a cheap PR wrapper.

The coordinating owner must join the existing review/disposition authority to
the actual supported native enqueue operation. That join must demonstrate:

- an exact current H, selected objective, current material-findings disposition
  and conditional landing authority;
- successful enqueue without requiring a not-yet-created G check on H, and
  without inventing a fake pre-G success;
- wrong-head, withdrawn, stale/foreign/missing-proof refusals;
- duplicate or uncertain response readback of the native queue operation;
- effective late-finding withdrawal and an honest landing/removal race outcome;
- publisher credentials isolated from candidate-executed code.

The retained `project-review-readiness.sh` assumes its historical PR synthetic
event or main-push enumeration. Do not manufacture those events to call it from
a warm session or run it as a central observer. The semantic owner must adapt
or replace that transport while preserving its typed findings, source binding,
neutral/missing distinction and currentness checks. Worker-local GitHub reads
remain the default; coordinating capacity and dependencies does not require a
central read gateway.

## Coordinated activation and rollback

1. Record the old source/settings generation, current required context names
   and publishers, queue settings, review controls and cache writer rules. Bind
   this candidate to an immutable commit/tree and record the replacement map.
2. Qualify the H control join above, the new actual-G contexts and the source
   migration against the live admission semantics. Keep existing protections
   satisfiable throughout; neither a bypass nor a temporary unprotected main
   is the migration procedure. The bootstrap source PR needs an explicit
   coordinated route because its ordinary PR Actions have been removed.
3. Apply source and settings only under the coordinating owner's authority.
   Verify one positive authorized-H → actual-G → protected-M case, and one
   failed/cancelled/missing-proof case that cannot land. Observe withdrawal and
   recomposition. Record actual G, base, queue membership/order, run/attempt,
   context publisher and M/readback—not just the intended settings file.
4. Observe Draft/open/synchronize/Ready/body/label cases creating zero Actions
   runs and a normal landing creating no ordinary main replay **after step 2**.
   Until step 2 lands, a landing still starts the retained `push: main` proof
   graph; read those results back rather than assuming no replay. Verify the
   release trigger paths independently without publishing a test release.
5. If any gate or admission assumption fails, retain the blocker and restore
   the recorded coherent source/settings generation. Do not revert just the
   queue trigger while leaving impossible requirements, or just delete checks
   to clear a blocked candidate.

The current cache action remains restore-only for PR/merge-group refs; its
default-branch push/dispatch writer restriction is unchanged. Removing main
proof therefore also removes its ordinary cache refresh opportunity. Cache
publication/reuse needs the existing cache owner's measured, trusted plan; do
not grant queue candidates shared default-branch write authority as a shortcut.

The <=20-minute active qualification target includes setup, required children,
finalization and publication. This patch does not claim that the retained full
portfolio meets it. Measure queue wait, aggregate compute and shifted local
work separately; preserve proof before optimizing its denominator.

## Owning session and evidence

Keep the useful owner warm through focused proof, differentiated review, joined
repair, selection, enqueue, G observation and M reconciliation. Reuse unchanged
proof with its original subject/run identity; refresh only affected dimensions.
A normal wait is not a handoff. Use native events/supported waits or bounded
checks and read back uncertain effects before retrying them.

For a real interruption retain the candidate/objective, current findings,
authority, G/base/queue/run/attempt where created, accessible proof bytes, owner
and one next action. Before deleting the only copy of evidence or moving to
another executor, verify that the receiver can retrieve it. Cache capacity and
proof custody are separate. Do not discard useful warm build state merely to
mark a worker complete.

## Local validation

`python3 scripts/test-merge-group-lifecycle.py` uses real temporary Git histories
with a predecessor and candidate. It checks G versus bare H, exact checkout and
base/ref/repository/run identity, nonancestor and dirty-source refusals, every
mandatory job result, event triggers, immutable-base routing and seeded PR/bare-H
wiring regressions. PyYAML is required for the workflow-wiring checks, as it is
for the independent sentinel. The subject/result producer itself uses only
Python's standard library and Git.

These checks establish source behavior, not hosted execution, current required
check settings, live H publication, queue admission, duration or protected M.
The native Rust contracts, no-new guard and full actual-G qualification still
need their own execution receipts.

GitHub references: [merge queues](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue)
and [required status checks](https://docs.github.com/en/pull-requests/how-tos/merge-and-close-pull-requests/troubleshooting-required-status-checks).
