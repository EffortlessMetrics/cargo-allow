---
name: cargo-allow-0.2-campaign
description: Implement the currently selected cargo-allow 0.2.0 campaign issue from exact live repository and issue state, stay within one semantic owner and PR lane, validate proportionally, and hand the resulting exact head to the independent review skill.
---

# Cargo-allow 0.2 campaign implementation

Use this skill for reversible implementation work selected by the active cargo-allow `0.2.0` campaign. It routes one current issue from exact live state to a reviewable pull-request head. It does not perform independent review, make maintainer decisions, or cross an irreversible release boundary.

The independent review contract remains `.agents/skills/review-current-head/SKILL.md`. An implementation session must hand its exact current head to that skill rather than treating self-review, old review, or green CI as equivalent independent review.

## Start from exact live state

Before broad implementation, read and reconcile:

1. `AGENTS.md`, `GEMINI.md`, and `CLAUDE.md`;
2. `docs/campaigns/cargo-allow-0.2.0.md` and controller issue `#3768`;
3. the selected issue body, current comments, dependencies, acceptance criteria, non-goals, and claim boundary;
4. current `main`, the exact merge base, open pull requests, active branches where observable, and the full current repository state relevant to the issue;
5. current CI state, current review state, and required check identities for the selected branch or pull request;
6. local branch, status, diff, worktrees, and generated or untracked state when operating in a checkout;
7. current external state when the issue depends on a registry, GitHub setting, public release, or another repository.

Conversation memory, a prior issue summary, an old task-run narrative, a remembered source location, a prior branch, a prior receipt, and the latest green run are not current authority. Missing, unavailable, stale, conflicting, quota-limited, or ambiguous evidence remains non-clean.

## Classify the task before editing

Assign exactly one class:

- `ReversibleImplementation`: source, tests, docs, schemas, workflows, fixtures, or repository policy changes that can be reviewed and reverted before an external commitment. This skill may proceed.
- `ReadOnlyReview`: inspect and report without becoming the implementation writer. Activate `.agents/skills/review-current-head/SKILL.md`; do not mutate the branch.
- `ExternalObservation`: read current registry, GitHub, provider, release, ruleset, or downstream state without mutation. Record observation time, identity, result, and limitations. Observation never authorizes mutation.
- `RootDecision`: a maintainer must choose product scope, support posture, platform commitment, target repository, version, recovery posture, accepted limitation, or another non-derivable policy. Produce the narrowest decision packet with a recommendation, deciding criterion, consequences, and what changes the call, then stop.
- `IrreversibleOperation`: create, move, or delete release tags; publish or yank packages; publish, replace, or delete a GitHub Release; change live repository controls; mutate an external pilot repository; mint final release authorization; or perform another externally committed action. Stop unless the exact operation and controlling typed authority have been explicitly authorized.
- `BlockedOrStale`: the issue is blocked, already owned by another viable writer, superseded, or based on a stale premise. Comment with the exact blocker or changed fact and route it to the current owner. Do not implement the historical body anyway.

Do not reclassify a `RootDecision`, `IrreversibleOperation`, or `BlockedOrStale` task as implementation merely because the mechanism is available or the old issue is labeled agent-ready.

## Select one unblocked issue

Choose one issue only when all of the following are true:

- it is in the currently selected `#3768` denominator or has an explicit current reclassification into that denominator;
- its prerequisites are complete or the issue defines a useful independently reviewable slice;
- no open pull request or active semantic writer owns the same contract, files, schema, workflow, or state transition;
- the work does not activate a conditional RC.2 lane, broaden final support, stabilize cargo-intent or cargo-proof, or select another product decision without an explicit maintainer ruling;
- the intended change can fit one semantic owner and one pull request.

When an active viable pull request already owns the lane, coordinate with or review that pull request rather than opening a duplicate writer. When several issues are unblocked, prefer the smallest issue whose completion unblocks multiple later rails. Do not use issue count, label age, or nearby files as priority signals, and do not bundle adjacent work merely because capacity is available.

## Post an issue execution packet

Before broad code changes, post or refresh a concise packet on the selected issue containing:

```text
controller and selected child issue
classification
exact base commit, tree, head, and merge base when available
active competing pull requests or writers
semantic owner and intended files or consumers
predecessor evidence consumed
implementation purpose and exact owned seam
highest-risk invariant or false-green route
required negative controls
scope, non-goals, and hard stops
focused and widened proof
independent review handoff
external or irreversible actions = none
claim boundary
```

If live state invalidates the issue premise, classify it `BlockedOrStale`, update the issue with evidence, and return to the controller. Do not implement the historical premise anyway.

## Keep one semantic owner and one writer

- Use one fresh branch from current `main` for one issue-sized semantic change.
- Keep one writer for every shared type, schema, workflow, package identity, generated authority, or mutation path.
- Let the issue that owns a downstream consumer perform its consumer migration after the producer contract lands; do not create competing edits across stacked branches.
- Preserve accepted behavior and unrelated work. Delete or supersede only what the selected issue owns.
- Do not mix release repair, product UX, CI economy, package topology, documentation policy, and agent-control changes merely because they share a repository.
- If another current PR begins touching the same semantic owner, stop, reconcile ownership, and choose one merge candidate.

## Implement the smallest complete change

A complete implementation should:

- satisfy the selected issue's actual acceptance criteria rather than only add scaffolding;
- preserve fact, inference, decision, support claim, publication state, and authorization as separate fields where the domain requires them;
- fail closed for missing, partial, stale, unsupported, malformed, conflicting, skipped, cancelled, or instrument-failure state;
- use typed or repository-owned authorities instead of adding another shell, prose, path-list, version-list, or package-list truth table;
- retain deterministic ordering and portable identities;
- include discriminating negative controls for the defect or boundary being changed;
- update checked projections, fixtures, documentation, policy receipts, and generated artifacts only when the semantic owner requires them;
- leave no hidden fallback to the old path after a claimed cutover;
- perform no tag, registry, GitHub Release, live-setting, authorization, or external-repository mutation.

A narrow foundation may remain a partial issue result only when the issue explicitly defines it as an independently useful accepted slice and the remaining acceptance stays open.

## Validate proportionally and fail wide

Run the cheapest discriminating proof first, then widen according to changed seams.

Typical order:

1. formatter, parser, frontmatter, link, or static contract checks for changed files;
2. focused unit, contract, schema, fixture, or script tests owned by the change;
3. affected package Clippy and tests;
4. affected product or compatibility lane;
5. `cargo run -p cargo-allow -- check --mode no-new` when repository policy or source inventory can be affected;
6. package, exact-candidate, release-trust, cross-platform, or full-workspace proof when the changed contract can invalidate it.

Unknown or contradictory impact selects the wider proof set. A cache hit, skipped job, empty test selection, artifact upload, or process exit without the expected semantic result is not proof.

Record commands actually run, their exact subject, semantic result, and any limitation. Do not claim local Linux proof establishes Windows, public registry, installed-candidate, model-discovery, or external-adoption behavior.

## Open or update one reviewable pull request

The pull request must identify:

- selected issue and controlling campaign;
- exact purpose and user or operator effect;
- changed semantic owner and important files;
- behavior preserved;
- non-goals and irreversible operations not performed;
- focused and widened proof actually run;
- negative controls;
- rollback;
- exact claim boundary.

Keep the PR as draft while implementation or author self-review is incomplete. Do not mark merge-ready from issue assignment, generated prose, an author-side review, an old review, a generic comment review, or partially green CI.

## Hand off the exact head to independent review

After implementation and author checks:

1. synchronize the PR metadata with the actual full current diff;
2. record the exact base SHA, head SHA, and merge base;
3. invoke `.agents/skills/review-current-head/SKILL.md` for independent review;
4. provide the selected issue, execution packet, changed contracts, expected invariants, negative controls, and proof artifacts;
5. keep implementation, CI, external observation, and independent review verdicts distinct.

The reviewer must inspect the full current diff, not only the latest repair commit. Reviewer identity alone does not create independence. Unavailable or quota-limited Gemini, CodeRabbit, or other model review is `NotProven`, not approval. Pending, skipped, cancelled, stale, or non-discriminating CI is also non-clean.

## Repair and re-review law

When review or CI finds a defect:

- preserve the accepted issue scope;
- return a bounded repair packet to the one writer;
- repair the smallest owning seam;
- rerun affected focused proof and any invalidated wider proof;
- update the PR body when behavior, risk, or claim boundary changed;
- invalidate the previous review for every load-bearing head, base, or merge-base change;
- request a fresh review of the new exact base/head pair and full current diff.

A writer may analyze and test a repair, but may not count that work as the independent current review required for merge readiness.

## Merge and synchronized-main closeout

Merge only when:

```text
selected acceptance is complete for the claimed slice
+ exact current independent review is clean
+ every required CI result is terminal and green
+ conversations are resolved
+ the head, base, and merge base are current and mergeable
```

After merge:

1. observe the exact merge commit and synchronized `main` tree;
2. verify the selected default-branch checks or record any provider or instrument failure honestly;
3. update the child issue with merged PR, exact main identity, proof, retained receipts, remaining limitations, external actions performed, and claim boundary;
4. close the child only when its stated acceptance is actually complete against merged-main/current evidence;
5. update `#3768` with the lane transition and next unblocked owner or `RootDecision`;
6. remove only implementation-owned scratch branches, worktrees, or artifacts after retained evidence is safe.

Do not infer authorization for the next issue, tag, publication, support promotion, or external operation from a successful merge.

## RC.1 and final-release immutability

Normal implementation work must preserve all of the following:

- never delete, recreate, or move `v0.2.0-rc.1` again;
- never publish another package row at `0.2.0-rc.1`;
- never treat a tag push, green workflow, issue assignment, or freeze receipt as release authorization;
- never continue or recover a partial release from moving `main` or rebuilt package bytes;
- never reuse RC.1 package bytes, checksums, freeze, authorization, or release receipt as final `0.2.0` identity;
- never create `v0.2.0`, read the publication token, publish packages, or finalize the final GitHub Release from an ordinary implementation session;
- after `#2501` produces a Complete exact final freeze, stop for the separate exact `#3760` authorization;
- after an externally observed final tag or immutable package row, any byte-changing repair requires a new version rather than tag movement or replacement.

Installed RC.1 observations may inform usability or compatibility work only when final-candidate facts are separately requalified. A later passing run cannot erase the earlier incident lineage.

## Hard stops

This implementation skill never authorizes or performs:

- creation, movement, or deletion of any release tag;
- package publication, yanking, ownership change, or credential exercise;
- publication, replacement, or deletion of a GitHub Release or release asset;
- live branch, tag, ruleset, environment, secret, or repository-control changes;
- mutation of an external clean or brownfield pilot repository;
- final release authorization or recovery authorization;
- selection of RC.2, final support/platform/channel scope, maintenance policy, or another maintainer decision;
- treating cargo-intent, cargo-proof, prebuilt assets, macOS, editor integration, or repository extraction as cargo-allow final-release blockers without a current ruling.

Return a bounded decision or operator packet at those boundaries. Do not continue through them.

## Claim boundary

This skill governs reversible, issue-first implementation for one selected cargo-allow `0.2.0` campaign lane from exact live state through a reviewable current pull-request head, proportional proof, independent review handoff, and synchronized-main closeout. It does not independently review its own work, choose product or support policy, authorize release operations, prove model discovery by itself, or perform external mutation.