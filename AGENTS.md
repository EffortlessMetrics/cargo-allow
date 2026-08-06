# cargo-allow Agent Instructions

## Operating Model

Start from live state. Check branch, status, open PRs, relevant CI, release
state, repo guidance, and dirty worktree state before choosing a lane.

Work one coherent lane at a time. Keep each change narrow, reviewable, and tied
to a clear proof obligation. Prefer one PR per product or maintenance slice.

When the user says to proceed in an active swarm, PR, or release lane, treat
that as authorization to carry the lane through its normal lifecycle unless
they narrow the scope.

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
Bind the review to the exact live head, inspect the complete changed-file set
plus relevant owners and consumers, check existing review threads before
posting, and prefer one batched actionable review. Do not mutate the branch
while claiming independent review. If the reviewer pushes a repair, the prior
review is stale and the new head requires fresh affected review.

For a user-authorized swarm, PR, or release lane, scoped commits, branch pushes,
PR creation, PR updates, PR merge, post-merge sync, and cleanup are normal once
the diff has been inspected and validation is recorded.

Do not stage unrelated files. Treat uncommitted user changes as user-owned
unless they are clearly created by the current lane. Before git operations that
change state, inspect branch, status, and the relevant diff.

Do not hard-reset, force-push, rewrite history, or move branch refs unless
explicitly asked. Do not push directly to `main`; use the normal PR merge path
unless the user explicitly asks for direct repository maintenance.

Before merge, verify that the live PR head still equals the final reviewed
head, the PR is non-draft and mergeable, substantive conversations are
resolved with current-head evidence, and every required check is terminal and
green or explicitly not applicable under repository policy. Pending,
cancelled, stale, malformed, action-required, and silently skipped evidence are
not green. Green CI alone is not merge readiness. Prefer a head-pinned merge
when the platform supports it.

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

After a green merge, sync `main`, rerun the required guard, and clean
`target/cargo-allow`, `target/package`, scratch files, stale branches, and
disposable worktrees created for the lane.

## Implementation Defaults

Use nearby project patterns before introducing new structure. Add or update
tests when behavior changes. Update docs, examples, schemas, generated
artifacts, and lockfiles only when the change requires it.

Do not add dependencies when the repo or standard library already solves the
problem cleanly. Avoid unrelated refactors.

Current priority order:

1. Publish or close the current patch-release loop when authorized.
2. Strengthen migration and evidence parity for `0.2.0`.
3. Improve PR posture weakening and improvement detection for `0.3.0`.
4. Continue small, fixture-backed scanner identity hardening for `0.4.0`.
5. Stabilize the governance surface for `1.0`.

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
