---
name: cargo-allow-0.2-campaign
description: Implement the currently selected cargo-allow 0.2.0 campaign issue from exact live repository and issue state, stay within one semantic owner and PR lane, validate proportionally, and hand the resulting exact head to the independent review skill.
---

# Cargo-allow 0.2 campaign implementation

Use this skill for reversible implementation work selected by the active cargo-allow `0.2.0` campaign. It routes one current issue from exact live state to a reviewable pull-request head. It does not perform independent review, make maintainer decisions, or cross an irreversible release boundary.

The independent review contract remains `.agents/skills/review-current-head/SKILL.md`. An implementation session must hand its exact current head to that skill rather than treating self-review, old review, or green CI as equivalent independent review.

## Start from exact live state

Before broad implementation, read and reconcile:

1. `AGENTS.md` and `GEMINI.md`;
2. `docs/campaigns/cargo-allow-0.2.0.md` and controller issue `#3768`;
3. the selected issue body, current comments, dependencies, acceptance criteria, non-goals, and claim boundary;
4. current `main`, the exact merge base, open pull requests, active branches where observable, and the full current repository state relevant to the issue;
5. current external state when the issue depends on a registry, GitHub setting, public release, or another repository.

Do not use an old issue title, remembered source location, prior branch, prior receipt, or latest green run as current authority. Missing, unavailable, stale, conflicting, quota-limited, or ambiguous evidence remains non-clean.

## Classify the task before editing

Assign exactly one class:

- `ReversibleImplementation`: source, tests, docs, fixtures, or workflow changes that can be reviewed and reverted before an external commitment. This skill may proceed.
- `ReadOnlyReview`: inspect and report without becoming the implementation writer. Use the appropriate review path instead of this implementation workflow.
- `ExternalObservation`: read current registry, GitHub, provider, or downstream state without mutation. Record observation time, identity, and limitations.
- `RootDecision`: a maintainer must choose product scope, support posture, platform commitment, target repository, version, recovery posture, or another non-derivable policy. Stop implementation at a bounded decision packet.
- `IrreversibleOperation`: create, move, or delete release tags; publish or yank packages; publish, replace, or delete a GitHub Release; change live repository controls; mutate an external pilot repository; mint final release authorization; or perform another externally committed action. Stop and route to the exact authorized operator issue.

Do not reclassify a `RootDecision` or `IrreversibleOperation` as implementation merely because the mechanism is available.

## Select one unblocked issue

Choose one issue only when all of the following are true:

- it is in the currently selected `#3768` denominator or is an explicitly accepted dependency;
- its prerequisites are complete or the issue defines a useful independently reviewable slice;
- no open pull request or active semantic writer owns the same contract, files, schema, workflow, or state transition;
- the work does not activate a conditional RC.2 lane, broaden final support, stabilize cargo-intent or cargo-proof, or select another product decision without an explicit maintainer ruling;
- the intended change can fit one semantic owner and one pull request.

When several issues are unblocked, prefer the one that removes a current blocker for the smallest complete downstream dependency chain. Do not bundle adjacent work merely because the same files are open.

## Post an issue execution packet

Before broad code changes, post or refresh a concise packet on the selected issue containing:

```text
classification
exact base commit and tree when available
selected issue and controlling parent
active competing pull requests or writers
semantic owner and intended files
accepted prerequisites
implementation outcome
non-goals and hard stops
focused and widened proof
independent review handoff
claim boundary
```

If live state invalidates the issue premise, update the issue with evidence and return to the controller. Do not implement the historical premise anyway.

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

A narrow foundation may remain a partial issue result only when the issue explicitly defines it as an independently useful accepted slice and the remaining acceptance is kept open.

## Validate proportionally and fail wide

Run the cheapest discriminating proof first, then widen according to changed seams.

Typical order:

1. formatter or parser checks for changed files;
2. focused unit, contract, schema, fixture, or script tests owned by the change;
3. affected package Clippy and tests;
4. affected product or compatibility lane;
5. `cargo run -p cargo-allow -- check --mode no-new` when the repository policy or source inventory can be affected;
6. package, exact-candidate, release-trust, cross-platform, or full-workspace proof when the changed contract can invalidate it.

Unknown or contradictory impact selects the wider proof set. A cache hit, skipped job, empty test selection, artifact upload, or process exit without the expected semantic result is not proof.

Record commands actually run, their exact subject, and any limitation. Do not claim local Linux proof establishes Windows, public registry, installed-candidate, or external-adoption behavior.

## Open or update one reviewable pull request

The pull request must identify:

- selected issue and controlling campaign;
- exact purpose and user or operator effect;
- changed semantic owner and important files;
- behavior preserved;
- non-goals and irreversible operations not performed;
- focused and widened proof already run;
- negative controls;
- rollback;
- exact claim boundary.

Keep the PR as draft while implementation or author self-review is incomplete. Do not mark merge-ready from issue assignment, generated prose, old review, or partially green CI.

## Hand off the exact head to independent review

After implementation and author checks:

1. synchronize the PR metadata with the actual full current diff;
2. record the exact base/head pair and current merge base;
3. invoke `.agents/skills/review-current-head/SKILL.md` for independent review;
4. provide the selected issue, execution packet, changed contracts, expected invariants, negative controls, and proof artifacts;
5. keep implementation and review verdicts distinct.

The reviewer must inspect the full current diff, not only the latest repair commit. Unavailable or quota-limited model review is `NotProven`, not approval. Pending, skipped, cancelled, stale, or non-discriminating CI is also non-clean.

## Repair and re-review law

When review or CI finds a defect:

- preserve the accepted issue scope;
- repair the smallest owning seam;
- rerun affected focused proof and any invalidated wider proof;
- update the PR body when behavior, risk, or claim boundary changed;
- invalidate the previous review for every load-bearing head change;
- request a fresh review of the new exact base/head pair and full current diff.

A writer may analyze and test a repair, but may not count that work as the independent current review required for merge readiness.

## Merge and synchronized-main closeout

Merge only when:

```text
selected acceptance is complete for the claimed slice
+ exact current independent review is clean
+ every required CI result is terminal and green
+ conversations are resolved
+ the head is current and mergeable
```

After merge:

1. observe the exact merge commit and synchronized `main`;
2. verify the selected default-branch checks or record any provider/instrument failure honestly;
3. update the child issue with merged PR, exact main identity, proof, remaining limitations, and claim boundary;
4. close the child only when its stated acceptance is actually complete;
5. update `#3768` with the lane transition and next unblocked owner;
6. remove only implementation-owned scratch branches or artifacts after retained evidence is safe.

Do not infer authorization for the next issue, tag, publication, support promotion, or external operation from a successful merge.

## Hard stops

This implementation skill never authorizes or performs:

- creation, movement, or deletion of any release tag;
- package publication, yanking, ownership change, or credential exercise;
- publication, replacement, or deletion of a GitHub Release or release asset;
- live branch, tag, ruleset, environment, secret, or repository-control changes;
- mutation of an external clean or brownfield pilot repository;
- final release authorization or recovery authorization;
- selection of RC.2, final support/platform/channel scope, maintenance policy, or another maintainer decision;
- treating cargo-intent, cargo-proof, prebuilt assets, macOS, editor integration, or repository extraction as cargo-allow core requirements without a current ruling.

Return a bounded decision or operator packet at those boundaries. Do not continue through them.

## Claim boundary

This skill governs reversible, issue-first implementation for one selected cargo-allow `0.2.0` campaign lane from exact live state through a reviewable current pull-request head, proportional proof, independent review handoff, and synchronized-main closeout. It does not independently review its own work, choose product or support policy, authorize release operations, or perform external mutation.