# Agent Operating Model

The source-of-truth stack is intended to give Codex and other agents bounded,
repo-native work without relying on chat history.

## Agent Inputs

An agent should be able to start from source-tree artifacts:

- proposal for why the work exists.
- spec for the required behavior.
- ADR for durable design constraints.
- implementation plan for PR order.
- active goal manifest for current execution state.
- support tiers for user-facing claim boundaries.
- policy ledgers for governed exception and profile state.
- closeout for what already landed.

If an artifact is missing or a link is broken, the right output is a repair item
or a narrow PR, not an invented claim.

## Execution Boundary

The planned `spec-system` profile is a structural scanner. It may read files,
parse TOML, parse simple Markdown structures, compare IDs and paths, and emit
reports, receipts, and worklists.

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

Future `cargo-allow worklist --profile spec-system --format json` output should
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

For example, the first source-of-truth PR can add only this documentation front
door. Later PRs can add templates, a proposal, a spec, a doc artifact ledger,
support tiers, an active goal manifest, an implementation plan, profile config,
and eventually CLI support.

## Active Goal Manifest

The active goal manifest should describe the current agent lane, not product
runtime state.

Planned shape:

```toml
schema_version = "1.0"

id = "spec-system-profile"
title = "Spec-system profile"
status = "active"
owner = "codex"
created = "2026-06-12"

linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"
linked_plan = "plans/spec-system/implementation-plan.md"

[[work_item]]
id = "spec-system-pr-001"
status = "ready"
title = "Add doc artifact ledger parser"
proof_commands = [
  "cargo test -p allow-policy spec_system",
  "cargo run -p cargo-allow -- check --mode no-new"
]
```

This manifest becomes useful only after the proposal, spec, and implementation
plan exist. It is not part of this documentation-only PR.
