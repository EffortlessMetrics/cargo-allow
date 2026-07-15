# Agent Operating Model

The source-of-truth stack is intended to give Codex and other agents bounded,
repo-native work without relying on chat history.

## Agent Inputs

An agent should be able to start from source-tree artifacts:

- proposal for why the work exists.
- spec for the required behavior.
- ADR for durable design constraints.
- implementation plan for PR order.
- accepted requirements and PR-local implementation slices for current scope.
- support tiers for user-facing claim boundaries.
- policy ledgers for governed exception and profile state.
- closeout for what already landed.

If an artifact is missing or a link is broken, the right output is a repair item
or a narrow PR, not an invented claim.

## Execution Boundary

The `spec-system` profile is a structural scanner. It may read files, parse
TOML, parse simple Markdown structures, compare IDs and paths, and emit reports,
receipts, and worklists.

It must not:

- execute proof commands.
- run tests.
- call GitHub APIs.
- use network access for cargo-allow's own scan.
- run ripr, unsafe-review, coverage, Cargo, rustc, Clippy, build scripts, or
  proc macros.
- claim semantic correctness.
- claim proof execution.

Agents may run validation commands as part of a PR workflow, but that is
separate evidence collected by the agent. It is not a capability of
cargo-allow's source-tree scan.

## Work Item Discipline

`cargo-allow worklist --profile spec-system --format json` output should
produce bounded source-of-truth repairs.

Good work items:

- identify one broken link, missing field, or missing artifact.
- name the artifact ID and path.
- name the owner when known.
- include suggested actions.
- include proof commands that a human or agent should run after the repair.

Bad work items:

- ask an agent to implement an entire release.
- mix unrelated artifact families.
- claim proof passed without evidence.
- suppress or broaden policy to make a report green.
- auto-extend expiry or launder baseline debt into approval.

## PR Shape

Source-of-truth work should land in narrow PRs. Each PR should state:

- purpose.
- non-goals.
- validation.
- claim boundary.
- rollback path.

For example, a source-of-truth PR can update one artifact family, close one
broken graph edge, or repair one worklist class. The implemented profile has
templates, proposals, specs, a doc artifact ledger, support tiers,
implementation plans, profile config, and CLI support. Historical goal
artifacts remain available for navigation but are not current coordination
state.

## Current Work and Historical Goals

Current work is identified by GitHub issues and PRs, accepted requirements, and
one PR-local implementation slice or claim. One writer per branch/worktree is a
local collision rule, not a repository-global goal selector. Local agent or
session focus is disposable and uncommitted.

Legacy goal files may still be parsed when explicitly requested for historical
navigation. They cannot authorize mutations, select current work, or promote
implementation or support status. The completed records are archived under
`.allow/goals/archive/`.

Example PR-local claim:

```toml
requirement = "REQ-EXAMPLE"
slice = "SLICE-EXAMPLE"
implementation_seam = "crates/example/src/lib.rs::feature"
tests = ["crates/example/tests/feature.rs::accepts_valid_input"]
proof_obligations = ["exact-head receipt", "negative regression"]
```

The current repository profile has no live goal manifest. Completed GOAL-0003
and GOAL-0004 records live under
[`.allow/goals/archive/`](../../.allow/goals/archive/) as historical evidence.
Current work is located through GitHub issues and PRs, accepted requirements,
and PR-local implementation slices; local session focus is disposable.
