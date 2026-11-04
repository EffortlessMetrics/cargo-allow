# cargo-allow Agent Instructions

## Operating Model

Start from live state. Check branch, status, open PRs, relevant CI, release
state, repo guidance, and dirty worktree state before choosing a lane.

Keep each lane coherent, reviewable, and tied to a clear proof obligation.
Independent objectives may proceed concurrently; one lead owns each candidate's
integration and one writer mutates its branch at a time. Include the code, tests
and documentation needed to complete the objective rather than splitting them
artificially. Larger focused PRs are acceptable with adequate review and proof.
For campaign execution and issue routing, follow
[`.agents/skills/cargo-allow-0.2-campaign/SKILL.md`](.agents/skills/cargo-allow-0.2-campaign/SKILL.md).

When the user says to proceed in an active swarm, PR, or release lane, treat
that as authorization to carry the lane through its normal reversible lifecycle
unless they narrow the scope. A consequential external or irreversible effect
still needs its actual authority; do not infer release permission from coding
or merge permission.

## Queue-only CI and continuous ownership

The selected ordinary lifecycle is warm in-session proof and substantive review,
current product/scope judgment at exact head H, conditional authorization to the
native merge queue, required proof on its actual composed candidate G, and
protected landing M with readback. The same useful owner normally keeps context
through implementation, review, repair, selection, queue wait and reconciliation.
Selection is per objective; independent objectives may progress concurrently.

Ordinary commit, push, PR open/synchronize/reopen, Draft, Ready, labels and body
edits allocate zero GitHub Actions work, including cheap readiness wrappers.
Run development checks in the owning session. Ready remains review/UI state,
not an enqueue request or landing authorization. Native enqueue grants conditional
landing authority; do not assume a second manual approval pause after green G.

Read [the source/settings cutover](docs/ci/merge-queue-cutover.md) and live #4602
before using the route. A source candidate does not activate required checks,
queue settings or the trusted H publisher. Preserve current protections until
coordinated activation is qualified; report a missing native operation or
unsatisfiable requirement instead of inventing a command or bypassing it.

Inspect submitted reviews, inline/outdated unresolved threads and ordinary
Conversation findings. Missing/partial reads and provider quota skips are not a
clean review. Repair or refute material findings with current evidence, reply
before resolving a thread and read back both transitions. New blocking findings
withdraw readiness even at unchanged H: remove the native queue entry and read
back the result. If landing wins that race, record actual M and route the repair.
Do not claim that a Draft transition alone revoked an in-flight queue operation.

Reuse unaffected proof with its original subject/run identity; refresh affected
dimensions after a repair or real integration change. A normal wait is not a
handoff. Use supported waits/events or bounded checks, not tight polling. A real
interruption retains candidate/objective, findings, authority, run/attempt,
accessible proof bytes, owner and one next action. Before deleting unique proof
or transferring executors, verify the receiver can retrieve it. Useful warm build
state and evidence are retained according to their separate purposes.

Worker-local GitHub reads and decisions remain the default. The coordinating
owner handles dependencies, capacity and integration; a protected effect/publisher
service is not a mandatory gateway for ordinary observations.

## cargo-allow Product Boundary

`cargo-allow` is a direct source-tree exception ledger. It scans repository
files without executing project code and checks syntax-visible findings against
`policy/allow.toml`.

Do not introduce requirements for Cargo metadata, `cargo check`, rustc, Clippy,
build scripts, proc macro expansion, dependency resolution, ripr,
unsafe-review, coverage tools, network access, or GitHub API calls for
`cargo-allow`'s own scan.

Other tools can provide evidence. `cargo-allow` owns the durable source
exception ledger.

Never suppress findings just to pass CI. Never silently broaden policy. Never
auto-extend expiry. Never launder `baseline_debt` into approval. Never claim
macro-expanded, type-aware, MIR-level, build-aware, control-flow, data-flow,
unsafe-proof, test-adequacy, or coverage-proof behavior until implemented.

## Git, PR, and Review Workflow

For exploratory, review-only, or analysis-only work, stop at findings unless
the user turns it into an implementation lane.

When asked to review or re-review a pull request, validate review feedback, or
decide review currentness, follow
[`.agents/skills/review-current-head/SKILL.md`](.agents/skills/review-current-head/SKILL.md).
Bind the review to the exact live base/head pair and effective merge base,
inspect the complete changed-file set plus relevant owners and consumers, check
existing review threads before posting, and prefer one batched actionable
review. Do not mutate the branch while claiming independent review.

Reviewer identity alone is not independence. The same agent can take a fresh
pass using changed evidence and controls; another identity can repeat the same
stale assumptions. State the actual independence and correlated-failure limit.

If the reviewer pushes a repair, the prior review is stale and the new head
requires fresh affected review. If the base or merge base changes, recompute the
effective diff and rerun every affected review dimension before preserving the
verdict. A new material finding can withdraw readiness without any source
change. A new SHA does not erase unaffected evidence.

In this solo-maintainer repository, Draft is a machine-visible safety control
for unresolved blocking review findings. When a review identifies a
merge-blocking defect, the PR must be converted to draft in the same pass
(`gh pr ready --undo`); only a fresh re-review on the repaired head confirming
zero blocking findings may restore it to ready (`gh pr ready`). **Ready is
review/UI state, not a qualification request or merge authorization.** Do not
toggle it merely to start CI.

For a user-authorized swarm or PR lane, scoped commits, branch pushes, PR
creation, PR updates, repair, review, qualification observation, protected
merge, post-merge sync, and cleanup are normal once each transition's actual
prerequisites are satisfied.

Do not stage unrelated files. Treat uncommitted user changes as user-owned
unless they are clearly created by the current lane. Before git operations that
change state, inspect branch, status, and the relevant diff.

Do not hard-reset, force-push, rewrite history, or move branch refs unless
explicitly asked. Do not push directly to `main`; use the normal protected PR
merge path unless the user explicitly asks for authorized repository
maintenance.

## Ordinary Development Qualification

The selected target lifecycle is:

```text
warm implementation and focused proof in-session
-> differentiated substantive review and joined repair
-> current product/scope judgment and conditional authority for exact H
-> verified native enqueue
-> required proof on actual G, its base and queue composition
-> native protected landing M or removal/recomposition
-> actual M and objective reconciliation
```

H authorization and G proof are different obligations. Do not require a future G
result before enqueueing, manufacture a pre-G success, or treat an old H label
or sibling's green result as authority. Reconcile the observed native operation
before repeating an uncertain request. Source-only tests do not establish live
admission or protection.

Use the existing mandatory/affected portfolio (#2569/#3753); unknown impact
widens proof or blocks. Preserve MSRV, platform, package, coverage, frozen-subject
and release-sensitive requirements. A cache hit, skipped job, control
acknowledgment or candidate-authored success field is not qualification. Keep
executed, admissibly reused, evidenced inapplicable and missing obligations
separate. A changed queue composition needs a new G.

Complete active ordinary qualification targets at most 20 minutes, including
setup, required children, finalization and trusted result publication. Queue wait,
aggregate machine cost and shifted local work are measured separately. Do not
weaken proof or hide work to meet the target. Ordinary main changes launch no
routine replay; no age/count or daily health fallback is selected.

## Final Judgment and Merge

Before native enqueue, reacquire the selected H, current base/merge-base,
material findings, review/proof applicability, product fit and conditional
landing authority. Confirm a non-draft candidate and the actual supported queue
operation under the observed protection generation. Missing, stale, malformed,
cancelled, silently skipped and action-required evidence is not green.

The native queue qualifies actual G against its composed base and predecessors.
`queue-qualification` covers the mandatory ci.yml portfolio; sentinel,
frozen-subject and deterministic Changie contexts remain separate requirements
in the coordinated settings generation. No direct squash or auto-merge fallback
substitutes for native queue enforcement. A queue-qualified result may land
without another owner action.

Read back the queue/PR state and actual landed commit/tree M. Associate the
receipt with the actual composition before reusing it. A successful enqueue
response is not a merge receipt; an executed G is not automatically proof of a
different M. Reconcile the objective and any residual acceptance with one owner
and next action. A child merge does not alone close its parent objective.

Deleting local/remote branches and worktrees is allowed for task-owned or
confirmed stale state after evidence custody is secured. Do not delete ambiguous
user work or another lane's caches, evidence or unfinished work.

## Release Workflow and Authority

Publishing, tagging, install-smoke checks, public install-doc updates, and
release-record finalization require explicit release authorization. A merge
verdict does not grant it. Once release authorization is explicit, perform the
release steps directly, in dependency order, and record evidence. Do not leave a
release half-finished unless a real blocker appears.

Future releases should push a `v*` tag to trigger
[`.github/workflows/release.yml`](.github/workflows/release.yml). See
[`docs/release/README.md`](docs/release/README.md) for Trusted Publishing setup,
workflow-dispatch dry-runs, and manual fallback. Tag pushes perform real
crates.io uploads; do not push test tags without release authorization.

## Validation

Run the narrowest useful validation first, then broader checks when practical.
Use the cargo-allow no-new guard as the default final source-tree check:

```bash
cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md
```

Report checks as pass, fail, not run, unavailable, or not applicable. Include
relevant failures and validation gaps. Do not claim tests, builds, publishes,
deployments, merges, or releases succeeded without direct evidence.

After merge, verify the actual landing and reconcile the issue. Reuse still-valid
candidate proof; run affected integration checks when the landed composition
requires them, not the full suite again merely because a merge occurred. Preserve
required enforcement while the automatic-main CI migration is incomplete.
After landing, sync and inspect actual `main`; reuse applicable G proof and run
affected in-session checks when a material difference requires them. Clean
temporary local artifacts when their ownership and evidence retention are clear,
especially under actual disk/memory pressure. Inspect ownership and retain useful proof
first; clean `target/cargo-allow`, `target/package`, scratch files, stale
branches and disposable worktrees created for the lane only when they are no
longer needed. Do not delete another lane's caches, evidence or unfinished
work, and do not discard useful warm build state without a reason.

## Implementation Defaults

Use nearby project patterns before introducing new structure. Add or update
tests when behavior changes. Update docs, examples, schemas, generated artifacts,
and lockfiles only when the change requires it.

Do not add dependencies when the repo or standard library already solves the
problem cleanly. Avoid unrelated refactors.

## Active Priorities (#3768 Campaign Train)

0. Agent context, skill, and readiness controls (#3731, #3770, #3747)
1. RC.1 external reconciliation (#3759)
2. Release safety kernel (#3744, #3755, #3752, #3761, #3760)
3. Installed usability and pilots (#3771, #2466, #2467, #3151, #2485)
4. Candidate preparation, verification, and CI economy (#3750, #3773, #3774, #3753, #3751, #4602)
5. Final 0.2.0 candidate refreeze (#2501)
6. Hard STOP for separate explicit release authorization (#3760)
7. Final release execution and closeout only under #2502 authority

## Public Release Identities and Claim Boundaries

- `cargo-allow` public prerelease: `0.2.0-rc.1` (usable pilot evidence with incident lineage; not reusable as final package bytes or authorization).
- Stable rollback baseline: `0.1.11`.
- Prospective final: `0.2.0`.
- Shared substrate / `cargo-intent` / `cargo-proof` package line: `0.1.0`.
- `cargo-intent` and `cargo-proof` are independently experimental siblings and do not gate `cargo-allow`.
- Public `rc.1` is usable pilot evidence with incident lineage, not final exact-byte proof.
- Dormant contingency: `0.2.0-rc.2` is selected only if RC dogfood uncovers package-byte defects requiring an additional public prerelease before final support decisions.

## Session Lane Classification

Every work session operates in one of the following lane classes:

- `ReversibleImplementation`: Scoped code, doc, test, or config changes with proof, PR, exact-head handoff, and merge verification.
- `ReadOnlyReview`: Independent exact-head PR review and currentness verification following `.agents/skills/review-current-head/SKILL.md`.
- `ExternalObservation`: Read-only queries of external registries, CI runs, or issue states without local mutation.
- `RootDecision`: Escalation of policy, architectural, or release choices requiring human operator selection.
- `IrreversibleOperation`: Tag mutation, crates.io publish/yank, GitHub release publish/replace, or live secret/setting changes; require explicit authority and exact recovery posture.
- `BlockedOrStale`: Halting the affected route when dependencies, base refs, or prerequisites are stale or blocked while independent eligible work continues.

## Release Immutability Law

Once a release tag triggers an external CI run or any crate row is uploaded to
crates.io, that tag is immutable: never delete, move, retag, or overwrite it.
Any downstream repair requires a new version candidate under a fresh freeze and
separate authorization.

## Windows Shell

Prefer single-quoted PowerShell `-Command` strings when using `$`, `$_`, or
script blocks.

Do not assume Unix tools like `ls`, `grep`, `sed`, or `awk` are available.
Prefer `Get-ChildItem`, `Select-String`, `Get-Content`, and `Format-Table` for
local inspection.

If a command fails from quoting, fix the quoting before changing the underlying
task.

## Communication

Lead with the result. Keep summaries short and concrete. Use file paths,
commands, PR numbers, release versions, and failure modes when they matter.
