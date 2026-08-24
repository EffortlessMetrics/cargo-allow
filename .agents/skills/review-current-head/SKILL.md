---
name: review-current-head
description: Review or re-review an open pull request's exact current base/head pair, disposition existing feedback, and verify merge readiness without mutating the branch. Use when asked to review a PR, re-review after repairs, validate review threads, synthesize a repair packet, or decide whether a PR is merge-ready. Do not use as the primary implementation or repair skill.
---

# Review Current Head

## Trigger

Use this skill when the task is to:

- review an open pull request;
- re-review a PR after the author, a bot, or CI-driven repair changed its head;
- verify whether existing review comments remain valid on current code;
- consolidate review findings into one repair packet;
- decide whether an exact PR base/head pair is merge-ready.

Do not use this skill as the primary implementation workflow, for broad issue
research before a PR exists, or to fix the branch while claiming independent
review. A reviewer who mutates the branch becomes an author of the new head;
prior independent-review status is stale.

## Independence and Correlated Failure

Reviewer identity alone does not create independence. A different agent can
share the same stale assumptions, evidence, or gate blind spots; the same agent
can take a genuinely fresh pass when explicitly redirected to reload live state,
discard prior verdicts, and review without mutating the branch.

State the independence posture honestly:

- identify whether the reviewer authored or repaired the current head;
- reload the current base/head pair, issue/spec, diff, consumers, threads, and
  checks rather than relying on conversation memory;
- use repository tests, receipts, branch protection, and retained artifacts as
  external controls;
- disclose shared-prior or correlated-failure limits;
- do not count an author-side self-review as separate independent evidence.

Independence is designed into fresh evidence and controls, not assumed from
separate role names.

## Inputs

Bind the review to live, retrievable identities:

```text
repository and pull-request number
current head SHA
current base ref and base SHA
current merge-base SHA when distinct or relevant
synthetic merge/check commit identity when the provider evaluates one
PR draft and mergeability state
controlling issue/spec/requirement/implementation slice, when present
PR purpose, non-goals, claim boundary, and rollback posture
complete changed-file list and effective diff
current file contents and relevant callers/consumers
existing review threads, submissions, and top-level feedback
required and current checks for the exact base/head pair
retained proof or receipt identities claimed by the PR
```

Refresh these inputs before reviewing. If the live head changes during the
review, stop. If the base SHA or merge-base changes, recompute the effective
diff and invalidate every affected review dimension before continuing. Findings
and check results for the prior base/head pair remain historical evidence; they
do not review the new pair.

## Review Procedure

### 1. Reconcile live state

- Confirm the repository, PR number, exact current head, base ref, base SHA, and
  effective merge base.
- Confirm whether the PR is draft, mergeable, conflicted, closed, or superseded.
- Inspect the controlling issue and any accepted specification or PR-local
  implementation slice.
- Compare the PR body to the actual diff. Treat unsupported or stale PR-body
  claims as review findings.
- Inspect existing review threads before posting so the review does not create
  duplicate conversations.

### 2. Inventory the complete change

Review more than the unified patch:

- enumerate every changed path;
- open the current full file around each changed seam;
- inspect relevant callers, consumers, sibling implementations, schemas,
  fixtures, docs, package metadata, generated outputs, policy records, and
  release surfaces;
- determine whether a moved or copied implementation leaves duplicate current
  authority behind;
- distinguish current behavior from historical comments, stale branches, and
  superseded issue text.

A patch-only review is insufficient when correctness depends on unchanged
context or downstream consumers.

### 3. Select proportionate review passes

Use only passes that can change the verdict. Record why a pass is not
applicable rather than manufacturing ceremony.

## Review Passes

### Correctness and invariant preservation

Inspect failure paths, state transitions, stale data, identity, concurrency,
bounded I/O, process lifecycle, rollback, partial completion, platform behavior,
and false-success routes. Ask what happens when the input is missing, malformed,
ambiguous, too large, changed between plan and apply, or only partially
available.

### Architecture and source-of-truth ownership

Verify dependency direction, semantic ownership, package/module boundaries,
public API placement, compatibility posture, duplicate authority, and alignment
with the controlling proposal/spec/ADR. A type or module name is not evidence
that it belongs in a separate package.

### Integration and consumer coverage

Inspect callers and downstream artifacts. Check schemas, machine/human
projections, documentation, examples, package manifests, features, target- and
dev-dependency edges, platform-specific behavior, installation, migration,
release, and support claims where relevant.

### Test and oracle grip

Determine whether tests discriminate the claimed behavior and the observed
failure. Require negative, adversarial, stale-head, changed-base, replay,
collision, partial, and instrument-failure cases when those can create a false
green. Process exit success, field resemblance, or one happy-path fixture is
not semantic parity.

### Security, privacy, release, and claim boundaries

Check untrusted input, path and underlying-target containment, symlink/alias
behavior, private absolute paths, credentials, process invocation, publication
authority, irreversible actions, and claims that exceed the instrumented
surface.

### Simplification

Look for duplicate types, redundant adapters, dead compatibility, speculative
abstractions, unnecessary public surface, duplicated parsing/evaluation, and
machinery that can be a private module instead of a package or protocol.

## CI and External Evidence

CI is evidence, not the review authority.

- Bind every check, receipt, and external result to the exact reviewed
  base/head pair and relevant tool/configuration identity.
- When a provider evaluates a synthetic merge or queue commit, record that
  commit and verify its parents correspond to the reviewed base/head pair. Do
  not present synthetic-merge evidence as bare-head evidence.
- Distinguish `failed`, `pending`, `cancelled`, `skipped`, `not applicable`,
  `stale`, `malformed`, `not proven`, and `action required`.
- Classify failures as product, test/oracle, policy, instrument, infrastructure,
  flaky, or stale-head before deciding whether to edit or rerun.
- One clearly external failure may be rerun. Recurrence is a harness-reliability
  problem, not permission for unlimited retries.
- Green CI does not prove an invariant or edge case that the tests and tools did
  not exercise.
- A quota-limited or unavailable model review is not a clean review result.

Use `cargo-allow diff` and related receipts as the source-exception posture
input only. They do not prove compilation, runtime behavior, test adequacy,
architecture, or release readiness.

## Finding Contract

Post only findings that are current, actionable, and supported by the reviewed
base/head pair. Each finding should state:

```text
posture or severity
current-head file and line, when available
violated contract or invariant
observable failure, false green, or false claim
smallest required repair or proof
whether it blocks this PR or is a bounded follow-up
```

Use these dispositions when reconciling feedback:

```text
Blocking
NonBlocking
RefutedWithEvidence
DuplicateOrSuperseded
StaleAfterHeadChange
StaleAfterBaseChange
AcceptedFollowUp
RootDecisionRequired
```

Do not post generic approval filler, speculative rewrites, or a second thread
for a current finding already represented by an existing conversation. Verify
bot claims against current code and primary contracts before adopting them.

## Posting Discipline

- Prefer one batched review over streaming individual comments.
- State the exact reviewed base SHA, merge-base SHA when relevant, and head SHA
  in the review body.
- Re-fetch those identities immediately before submitting the review. If the
  pair changed, do not post the stale verdict; restart the affected review.
- Anchor inline comments to current lines when possible.
- Put cross-file or missing-location findings in the review body.
- Do not resolve a thread merely because the author replied; verify the fix or
  refutation on current code.
- Keep prior-pair comments as historical evidence instead of pretending they
  reviewed the repaired or rebased PR.
- Consolidate valid findings into one repair packet for one writer. Several
  reviewers must not push competing fixes.

A clean review should state the reviewed base/head pair, review dimensions
exercised, check/evidence posture, and absence of actionable findings. Do not
use `LGTM` as an evidence substitute.

## Re-review After Repairs

Any repair that changes the head invalidates the affected review evidence. A
base or merge-base change can also change the effective patch and invalidates
the affected dimensions even when the head SHA is unchanged.

Perform a fresh affected review:

```text
fetch the new exact base/head pair
→ compare it with the previously reviewed pair
→ verify every prior disposition against current code
→ rerun the affected review passes
→ inspect edge cases introduced by the repair or new base
→ inspect exact-pair CI and receipts again
→ post or record the fresh verdict
```

New defects can be introduced by a correct repair or by interaction with newly
merged base changes. Do not limit re-review to checking that old comments were
mechanically addressed.

## Draft-State Law and Blocking Disposition

In a solo-maintainer workflow, same-account review comments do not trigger GitHub branch protection change-request blocking. To make blocking review findings machine-visible and prevent premature merge:

When `review-current-head` identifies a merge-blocking defect:
- submit the review findings with exact base/head and merge-base identities;
- **convert the PR to draft immediately in the same review pass** (`gh pr ready --undo`);
- post the smallest actionable repair packet for the author/writer;
- do not mark the PR ready merely because CI turns green or the author claims the defect is resolved;
- any subsequent author commit or base movement invalidates the prior review;
- only a fresh exact-head review confirming zero unresolved blocking findings may restore the PR to `Ready for review` (`gh pr ready`).

A blocking review retains real control authority through draft state even when API restrictions prevent a formal `REQUEST_CHANGES` submission.

## Merge Readiness

A PR is merge-ready only when all applicable statements are true:

- the reviewed head, base SHA/ref, and effective merge base still match the live
  PR state used for the verdict;
- the PR is non-draft and mergeable against the intended base;
- every substantive inline thread and top-level review conversation is resolved
  with current-pair evidence;
- all required checks are terminal and green, or explicitly accepted as not
  applicable under repository policy;
- pending, cancelled, stale, malformed, action-required, and silently skipped
  evidence are not treated as green;
- claimed tests and receipts belong to the exact base/head pair;
- no author commit or material base change landed after the final current-pair
  review;
- source-exception, schema, documentation, changelog, package, support, and
  release impacts are reconciled where applicable;
- the merge is head-pinned where the platform permits;
- post-merge main verification and issue/branch/worktree reconciliation remain
  scheduled.

Green CI alone is not merge readiness. Merge is not closeout.

## Failure Conditions

Return a blocked or not-proven review instead of guessing when:

- the PR, diff, controlling issue, current head, base identity, merge base, or
  required checks cannot be read;
- the head changes during review;
- the base or merge base changed and the effective diff was not reconciled;
- the source-of-truth owner or accepted contract is materially ambiguous;
- a required external receipt is absent, stale, malformed, or for another
  subject;
- existing feedback cannot be inspected well enough to avoid duplicates;
- a branch mutation would be required to complete the review.

## Output

Return a compact review packet:

```text
repository / PR / reviewed base SHA / merge base / reviewed head SHA
verdict
review dimensions exercised or skipped with reason
actionable findings and dispositions
exact-pair check and receipt posture
unresolved contradiction or root decision
one recommended next action
claim boundary
```

Keep raw logs and broad archaeology retrievable by reference rather than
copying them into every review summary.

## Claim Boundary

This skill standardizes read-only, exact-base/head pull-request review,
feedback disposition, re-review after repair or base movement, and
merge-readiness verification. It does not execute tests, certify semantic
correctness, replace deterministic CI or branch protection, authorize
publication, or make model output repository authority.
