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
Bind the review to the exact live base/head pair and effective merge base,
inspect the complete changed-file set plus relevant owners and consumers, check
existing review threads before posting, and prefer one batched actionable
review. Do not mutate the branch while claiming independent review. If the
reviewer pushes a repair, the prior review is stale and the new head requires
fresh affected review. If the base or merge base changes, recompute the
effective diff and rerun every affected review dimension before preserving the
verdict.

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

Publishing, tagging, install-smoke checks against public bytes, public
install-doc updates, and release-record finalization require explicit release
authorization. Once release authorization is explicit, perform the release
steps directly, in dependency order, and record evidence. Do not leave a
release half-finished unless a real blocker appears.

Future releases should push a `v*` tag to trigger
[`.github/workflows/release.yml`](.github/workflows/release.yml). See
[`docs/release/README.md`](docs/release/README.md) for Trusted Publishing setup,
workflow dispatch dry-runs, and manual fallback. Tag pushes perform real
crates.io uploads; do not push test tags without release authorization.

## Current `v0.2.0-rc.1` Campaign

This is the current release campaign until #3691 is reconciled after the public
RC. GitHub issues and PRs remain the live work authority; this section tells an
agent how to enter the graph without reconstructing it.

### Fixed identities and denominators

Keep these four denominators separate:

```text
retained workspace architecture       22 packages
new shared/intent/proof namespace     12 packages at 0.1.0
cargo-allow RC resolved closure       13 rows = 10 RC + 3 shared 0.1.0
cargo-allow RC upload set             10 packages at 0.2.0-rc.1
```

Release/version law:

```text
cargo-allow 0.1.11       final ordinary Rust 1.85 rollback baseline
cargo-allow 0.2.0-rc.1   current RC target, Rust 1.95
shared packages 0.1.0    independent experimental namespace line
cargo-intent 0.1.0       independent experimental product line
cargo-proof 0.1.0        independent experimental product line
final cargo-allow 0.2.0  later fresh refreeze, not an alias of the RC
```

Do not turn all 22 workspace packages into the cargo-allow candidate. Do not
change the twelve sibling namespace packages to the RC version. Public Cargo
package identities are `intent-compiler` and `proof-orchestrator`; retained
`intent-engine` / `proof-engine` source or Rust-library identities are not
alternate crates.io package names.

### Choose work from the live graph

Before starting implementation for an issue:

1. read the issue, its named parent/controller, and any predecessor receipt;
2. search open PRs/branches for that exact issue or seam;
3. inspect the live PR head, comments, review threads, and current CI before
   deciding whether the lane is free;
4. start from current `main` only when there is no viable in-flight lane.

Use six hours without a material branch/PR update as a coordination heuristic,
not as proof of abandonment:

- **updated within six hours:** default to review, coordination, or a different
  unowned issue; do not fork competing implementation;
- **untouched for six or more hours:** inspect the current branch and evidence,
  post or retain a takeover/handoff note, then carry the lane forward;
- replace an existing branch from current `main` only when the branch is
  demonstrably unusable or superseded. Preserve useful commits and receipts.

Do not use issue age alone. A fresh comment with no code movement does not make
stale candidate bytes current, and an old branch can still contain valid work.

### Immediate namespace state gate

#3729 merged commit `3942873eb18b9f88dfdfcc013ca013452fdb23b1`,
adding the legacy `release/authorize-v0.2.0.json` artifact with the stated intent
to trigger the twelve-package `0.1.0` namespace publisher. A release-safety
review had already identified that this cargo-allow stable-release identity was
the wrong authority for the independent namespace operation.

The path matched the then-current `release-authorized.yml` push trigger. Until
#3733 observes the actual GitHub Actions run and all twelve crates.io rows,
namespace state is **unknown external state**:

```text
#3733 read-only reconciliation
  -> NoPublication
  |  Partial / ReleaseIncident
  |  Complete with exact checksum reconciliation
  |  Conflict / InstrumentFailure / ProviderUnavailable
```

Do not infer publication success from the authorization source, and do not
repeat/recover the publication operation without the exact authorization
required by #3708/#3390 for the observed state.

`release/authorize-v0.2.0.json` is also currently an unreceipted governed file
on `main`. Do **not** delete, move, or receipt it merely to make the no-new guard
green before #3733 retains the #3729 run/registry state and artifact identity.
After observation, #3733 owns the reviewed historical/removal disposition.

### Current lane order

Current foundation state:

```text
#3718  merged: typed release-version preflight is on main
#3733  immediate read-only namespace external-state reconciliation
#3730  active durable namespace-authority repair; repair/re-review, do not fork
```

Writer closure:

```text
#3717 reconnaissance snapshot
  -> #3719 generated/classified/drift-checked denominator
  -> #3720 receipt/report/general-output convergence
  -> #3721 bootstrap/backup/restore/cleanup + Windows semantics
  -> #3692 final writer reconciliation
```

Do not treat #3717's Markdown inventory as writer closure by itself.

Sibling package and namespace rail depends on #3733's observed result:

```text
#3703 shared package qualification
#3704 cargo-intent ownership/package qualification
#3706 cargo-proof ownership/package qualification
    (these may proceed in parallel when they do not edit one authority)

if #3733 = NoPublication:
  -> #3722 exact 12-row candidate + zero-upload rehearsal
  -> #3708 / #3390 fresh exact namespace authorization + publication
  -> #3705 and #3707 exact crates.io-installed product proofs

if #3733 = Complete with all exact checksums:
  -> V2/docs/support reconciliation
  -> #3705 and #3707 exact crates.io-installed product proofs

if #3733 = Partial / Conflict:
  -> #3708 / #3390 incident-preserving recovery or version/name decision
  -> do not rebuild from moving main or erase the first irreversible run
```

A namespace recovery/upload remains an irreversible operation and requires the
separately named exact authorization. A prior broad release instruction does
not authorize altered bytes or a new recovery candidate.

RC identity and workflow rail:

```text
#3718 merged typed preflight
  -> #3723 exact ten-package 0.2.0-rc.1 candidate cut
  -> #3725 Changie prerelease-history proof/live corpus update
  -> #3724 typed RC/stable propagation through release.yml
  -> #3694 RC identity reconciliation
```

Delay #3723 until load-bearing source/package-byte changes have converged enough
that a version cut will not churn on every repair. Package README/docs included
in a `.crate` are package bytes; changing them after a digest freeze invalidates
that candidate.

Release evidence and qualification can proceed in bounded parallel lanes:

```text
#3693 -> #3726  V2 evidence + actual GitHub draft-asset closeout
#3727           exact 0.1.11 -> RC -> 0.1.11 upgrade/rollback
#2263           current-head blocker ledger
#2283/#2284     source/live GitHub release controls
#3151           exact installed release experience
```

After those and namespace/RC prerequisites are current:

```text
#3728  compile CargoAllowRcFreezeReceiptV1 = Complete
```

**#3728 is the final reversible handoff. Stop there.** A Complete freeze receipt
identifies bytes that *could* be authorized; it is not authorization. The next
step belongs to #3695 and requires a separate maintainer authorization copying
the exact freeze identities and digests before tag creation, crates.io upload,
or public GitHub Release state changes.

After actual public package publication, #3709 proves bounded three-product
interop and #3710 reconciles public package/docs.rs/support/announcement truth.
Do not use those post-publication facts to retroactively justify prepublication
claims.

### Implementation-lane packet

For every implementation PR, make the PR body sufficient for another agent to
review without conversation history. Include:

```text
controlling issue and parent/controller
exact problem/slice being closed
source-of-truth owner changed by this PR
explicit non-goals and irreversible actions not performed
changed files/surfaces
negative controls added or exercised
commands actually run and their result
hosted CI/receipt identity when available
remaining limitations or accepted follow-up
claim boundary
```

If implementation uncovers a separate defect, file/link the smallest bounded
issue and keep the current PR on its original seam. Do not turn a useful finding
into opportunistic scope growth.

After the author finishes a head, hand it to
`review-current-head`. Author-side inspection is useful but does not count as
independent review evidence. The reviewer reloads the live pair and may return a
repair packet; after repairs, re-review the new head before merge.

### Agent stop conditions

Stop and return evidence rather than guessing when:

- another active lane owns the same source-of-truth surface;
- the issue requires a product/support/root decision not already made;
- exact external registry or GitHub settings evidence is unavailable;
- a proposed repair would change the package architecture outside the issue;
- candidate identity moved and retained proof is stale;
- the next action is an authorization artifact, tag, upload, yank, recovery
  upload, or public release operation without the separately named
  authorization.

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

1. Complete #3733's read-only #3729 run/registry reconciliation; until then,
   treat namespace external state as unknown and do not clean up the triggering
   artifact merely for CI.
2. Repair and re-review active #3730; typed release preflight #3718 is already
   merged.
3. Drive writer closure (#3719 -> #3720 -> #3721) and sibling family
   qualification (#3703/#3704/#3706) in parallel where source ownership allows;
   route namespace candidate/public-install work from #3733's observed result.
4. Cut and prove the RC identity (#3723/#3725/#3724) after load-bearing source
   convergence; finish #3693/#3726 in parallel.
5. Close RC qualification inputs (#3727, #2263, #2283/#2284, #3151) and any
   required public sibling install proof.
6. Produce #3728's exact reversible freeze and stop for the separate #3695
   authorization before any RC tag/upload/public-release action.
7. After the public RC, reconcile #3709/#3710 and use RC findings to drive the
   later fresh final `0.2.0` refreeze rather than treating RC success as final
   authorization.

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
