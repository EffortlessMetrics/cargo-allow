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
that as authorization to carry the lane through its normal lifecycle unless
they narrow the scope.

## Selected-candidate CI and continuous ownership

Develop and review cheaply with warm focused proof. Compare alternatives before
PR publication while opening a Draft still triggers hosted CI. After the explicit
request cutover, useful alternative Drafts can stay cheap. Expensive pre-merge
qualification is for the candidate selected to land, not each alternative. Selection is per objective, never a
repository-wide one-PR queue. Do not duplicate an existing implementation merely
to consume agent capacity.

The intended lifecycle is explicit late qualification, normally at most 20 minutes
of complete CI execution, followed by a distinct final current-repository/product
judgment and protected expected-head squash. Queue time is separate. The same
useful lead normally retains context through the CI wait, result inspection,
merge and landing readback. Use supported waits/events or bounded checks, not
tight polling or a compulsory new agent/human handoff. A real interruption retains
the exact candidate, run/attempt, findings, owner and next action.

Use the actual installed interface and inspect its triggers. Current automatic
PR/push workflows are not changed by these instructions; coordinate their existing
CI-economy owners rather than silently skipping proof or inventing a command.
`premerge-ci-request` / `request-premerge-ci` are proposals, not installed names.
No routine auto-merge or merge queue. A retained label, sibling's green result or
successful source review cannot authorize a different head. Reconcile an existing
run before requesting another; classify a failure before repair or justified retry.

Inspect submitted reviews, inline threads including outdated unresolved threads,
and ordinary Conversation findings. Missing/partial reads and provider quota skips
are not a clean review. Repair or refute material findings with current evidence,
reply before resolving a thread, and read back both transitions. An earlier clean
review does not suppress a later valid finding. Preserve unaffected proof while
refreshing the dimensions changed by a repair or actual integration interaction.

When the task delegates ordinary integration and the final conditions below hold,
finish the protected merge without another routine approval. Explicit read-only,
Draft-only, release, settings and credential boundaries remain unchanged.

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

## Git, PR, and Release Workflow

For exploratory, review-only, or analysis-only work, stop at findings unless
the user turns it into an implementation lane.

When asked to review or re-review a pull request, validate review feedback, or
decide merge readiness, follow
[`.agents/skills/review-current-head/SKILL.md`](.agents/skills/review-current-head/SKILL.md).
Bind the review to the exact live base/head pair and effective merge base,
inspect the complete changed-file set plus relevant owners and consumers, check
existing review threads before posting, and prefer one batched actionable
review. Do not mutate the branch while claiming independent review. If the
reviewer pushes a repair, the prior review is stale and the new head requires
fresh affected review. If the base or merge base changes, recompute the
effective diff and rerun every affected review dimension before preserving the
verdict. When a review identifies a merge-blocking defect, the PR must be
converted to draft in the same pass (`gh pr ready --undo`); only a fresh
re-review on the repaired head confirming zero blocking findings may restore it
to ready (`gh pr ready`).

For a user-authorized swarm, PR, or release lane, scoped commits, branch pushes,
PR creation, PR updates, PR merge, post-merge sync, and cleanup are normal once
the diff has been inspected and validation is recorded.

Do not stage unrelated files. Treat uncommitted user changes as user-owned
unless they are clearly created by the current lane. Before git operations that
change state, inspect branch, status, and the relevant diff.

Do not hard-reset, force-push, rewrite history, or move branch refs unless
explicitly asked. Do not push directly to `main`; use the normal PR merge path
unless the user explicitly asks for direct repository maintenance.

Before merge, verify that the live PR head, base SHA/ref, and effective merge
base still equal the final reviewed pair, the PR is non-draft and mergeable,
substantive conversations are resolved with current-pair evidence, and every
required check is terminal and green or explicitly not applicable under
repository policy. Pending, cancelled, stale, malformed, action-required, and
silently skipped evidence are not green. Green CI alone is not merge readiness.
Prefer a head-pinned merge when the platform supports it.

Deleting local or remote branches and worktrees is allowed for branches and
worktrees created by the current lane, or confirmed stale after inspection. Do
not delete ambiguous user work.

Publishing, tagging, install-smoke checks, public install-doc updates, and
release-record finalization require explicit release authorization. Once
release authorization is explicit, perform the release steps directly, in
dependency order, and record evidence. Do not leave a release half-finished
unless a real blocker appears.

Future releases should push a `v*` tag to trigger
[`.github/workflows/release.yml`](../.github/workflows/release.yml). See
[docs/release/README.md](../docs/release/README.md) for Trusted Publishing
setup, workflow dispatch dry-runs, and manual fallback. Tag pushes perform real
crates.io uploads; do not push test tags without release authorization.

## Validation

Run the narrowest useful validation first, then broader checks when practical.

Use the cargo-allow no-new guard as the default final source-tree check:

```bash
cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md
```

Report checks as pass, fail, or not run. Include relevant failures and
validation gaps. Do not claim tests, builds, publishes, deployments, merges, or
releases succeeded without direct evidence.

After merge, verify the actual landing and reconcile the issue. Reuse still-valid
candidate proof; run affected integration checks when the landed composition
requires them, not the full suite again merely because a merge occurred. Preserve
required enforcement while the automatic-main CI migration is incomplete.
After a green merge, sync `main` and perform post-merge cleanup of temporary local
artifacts, especially when disk/memory pressure is present. Inspect ownership and
retain useful proof first; clean `target/cargo-allow`, `target/package`, scratch
files, stale branches and disposable worktrees created for the lane only when
they are no longer needed. Do not delete another lane's caches, evidence or
unfinished work, and do not discard useful warm build state without a reason.

## Implementation Defaults

Use nearby project patterns before introducing new structure. Add or update
tests when behavior changes. Update docs, examples, schemas, generated
artifacts, and lockfiles only when the change requires it.

Do not add dependencies when the repo or standard library already solves the
problem cleanly. Avoid unrelated refactors.

## Active Priorities (#3768 Campaign Train)

0. Agent context, skill, and readiness controls (#3731, #3770, #3747)
1. RC.1 external reconciliation (#3759)
2. Release safety kernel (#3744, #3755, #3752, #3761, #3760)
3. Installed usability and pilots (#3771, #2466, #2467, #3151, #2485)
4. Candidate preparation, verification, and CI economy (#3750, #3773, #3774, #3753, #3751)
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
- `ReadOnlyReview`: Independent exact-head PR review and merge-readiness verification following `.agents/skills/review-current-head/SKILL.md`.
- `ExternalObservation`: Read-only queries of external registries, CI runs, or issue states without local mutation.
- `RootDecision`: Escalation of policy, architectural, or release choices requiring human operator selection.
- `IrreversibleOperation`: Prohibited for autonomous sessions (tag mutation, crates.io publish/yank, GitHub release publish/replace, live branch/secret changes).
- `BlockedOrStale`: Halting when dependencies, base refs, or prerequisites are stale or blocked.

## Release Immutability Law

Once a release tag triggers an external CI run or any crate row is uploaded to crates.io, that tag is immutable: never delete, move, retag, or overwrite it. Any downstream repair requires a new version candidate under a fresh freeze and separate authorization.

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
