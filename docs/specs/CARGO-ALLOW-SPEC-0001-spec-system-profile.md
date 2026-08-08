---
id: CARGO-ALLOW-SPEC-0001
kind: spec
status: accepted
owner: repo-infra
created: 2026-06-12
linked_proposal: CARGO-ALLOW-PROP-0001
standalone_reason:
linked_adrs: []
support_tier_impact: advisory
policy_impact:
  - policy/doc-artifacts.toml
  - policy/spec-system.toml
  - policy/allow.toml
---

# Spec: Spec-System Profile

## Summary

The `spec-system` profile is an explicit cargo-allow profile for static
relationship linting of a repository's source-of-truth graph. It validates
artifact identity, links, statuses, required fields, support-tier references,
active-goal references, and closeout links without executing proof commands,
checking prose style, or changing default source-exception behavior.

## Behavior Contract

The system must:

- Keep default cargo-allow behavior unchanged unless `--profile spec-system` is
  selected for a command that supports profiles.
- Treat `spec-system` as opt-in profile behavior for source-of-truth graph
  governance, not as a generic Markdown linter.
- Build a graph from ledgers, machine-readable artifact headers, and configured
  source-tree roots, then validate node identity and edge integrity.
- Read repository source-tree files and policy ledgers directly.
- Support a machine-readable artifact registry at `policy/doc-artifacts.toml`.
- Support a profile configuration file at `policy/spec-system.toml`.
- Validate artifact IDs, kinds, paths, owners, statuses, linked artifacts,
  support-tier references, active-goal references, and closeout references.
- Emit source-tree claim boundaries and scanner limitations in profile reports
  and receipts.
- Emit worklist items that name the broken artifact, path, owner when known,
  message, suggested actions, and proof commands when known.
- Keep adoption advisory by default until a repo explicitly promotes selected
  checks.

The system must not:

- Run tests or proof commands as part of spec-system scanning.
- Invoke Cargo, Cargo metadata, rustc, Clippy, build scripts, proc macros,
  ripr, unsafe-review, coverage tooling, network calls, or GitHub APIs for
  cargo-allow's own scan.
- Claim semantic correctness, proof execution, release readiness, unsafe
  soundness, test adequacy, coverage, or support-tier truth from structural
  linting.
- Make `spec-system` checks part of default `cargo-allow check`, `audit`, or
  `diff` behavior.
- Treat generated adoption debt, missing evidence, or broadened policy as
  approval.
- Lint prose quality, heading capitalization, line length, exact section order,
  Oxford commas, or other Markdown style preferences unless required fields are
  no longer machine-readable.

## Inputs

| Input | Required | Notes |
| --- | --- | --- |
| `--profile spec-system` | yes | Explicit profile selector for supported commands. |
| `policy/spec-system.toml` | profile-dependent | Defines roots, requirements, and advisory/shadow/blocking posture. |
| `policy/doc-artifacts.toml` | profile-dependent | Registers governed proposal, spec, ADR, plan, active-goal, support-tier, policy, and closeout artifacts. |
| Artifact Markdown files | profile-dependent | Proposal, spec, ADR, implementation plan, plan item, closeout, and related source-of-truth docs. |
| `.allow/goals/active.toml` | profile-dependent | Active goal manifest when profile requirements enable active-goal checks. |
| `docs/status/SUPPORT_TIERS.md` | profile-dependent | Support-tier claim to proof-command map when profile requirements enable support-tier checks. |

## Outputs

| Output | Required | Notes |
| --- | --- | --- |
| Human or Markdown report | yes | Explains current source-of-truth posture for humans. |
| JSON report or receipt | yes | Uses the `cargo-allow.spec-system.v1` schema and records claim boundary and scanner limitations. |
| Worklist JSON | yes for worklist command | Emits bounded repair items for humans and agents. |
| Doctor readiness report | yes for doctor command | Reports missing profile config, ledger, roots, support tiers, templates, and active-goal files. |
| Exit status | yes | Respects advisory, shadow, and blocking posture without changing default non-profile behavior. |

## Command Surface

Profile-aware commands should use the explicit profile selector:

```bash
cargo-allow check --profile spec-system
cargo-allow audit --profile spec-system
cargo-allow worklist --profile spec-system --format json
cargo-allow doctor --profile spec-system
cargo-allow init --profile spec-system
cargo-allow explain CARGO-ALLOW-SPEC-0001 --profile spec-system
```

`check --profile spec-system` validates the source-of-truth graph structurally.
`audit --profile spec-system` explains current posture. `worklist --profile
spec-system` emits repair items. `doctor --profile spec-system` reports setup
readiness. `init --profile spec-system` bootstraps the profile files. `explain
<artifact-id> --profile spec-system` explains one registered artifact's
metadata, incoming and outgoing links, current findings, work items, proof
commands, and claim boundary.

Commands without `--profile spec-system` must keep their source-exception
behavior and must not require spec-system files.

## Graph Model

The profile should model source-of-truth artifacts as a static graph.
Formatting rules are secondary and should exist only where they make this graph
parseable.

Initial node kinds should include:

- `proposal`
- `spec`
- `adr`
- `implementation_plan`
- `plan_item`
- `active_goal`
- `support_tier`
- `policy_ledger`
- `closeout`
- `release_record`

Initial edge kinds should include:

- `linked_proposal`
- `linked_spec`
- `linked_adr`
- `linked_plan`
- `linked_goal`
- `linked_pr`
- `linked_issue`
- `proof_command`
- `support_tier_surface`
- `closeout_for`
- `supersedes`
- `superseded_by`
- `replaces`

Initial graph findings should include:

- `missing_node`
- `duplicate_id`
- `unknown_link_target`
- `missing_required_edge`
- `invalid_status`
- `artifact_file_missing`
- `artifact_id_not_in_file`
- `orphan_spec`
- `stale_active_goal`
- `missing_closeout`
- `missing_proof_command`
- `claim_without_support_tier`

The profile may check front matter parsing, required fields, support-tier table
parsing, and ID discoverability because those checks make graph construction
reliable. It must not lint prose quality or general Markdown style.

## Accepted States

- Default `cargo-allow check --mode no-new` ignores spec-system roots unless
  those files are ordinary source-tree findings governed by `policy/allow.toml`.
- `cargo-allow check --profile spec-system` runs only structural graph checks
  for the selected profile.
- Advisory profile findings are reported without becoming default source
  exception failures.
- Artifact files are registered, readable, under configured roots, and contain
  their visible IDs.
- Accepted specs link to an accepted proposal or provide an explicit standalone
  reason.
- Linked proposals, specs, ADRs, plans, active goals, support-tier surfaces,
  policy ledgers, and closeouts resolve by ID or configured path.
- Stable or stabilizing support-tier claims have non-empty proof-command fields.
- Done work items link to a closeout.

## Rejected States

- Duplicate artifact IDs in `policy/doc-artifacts.toml`.
- Missing artifact files for registered artifacts.
- Artifact IDs absent from their files.
- Unknown artifact kinds or statuses.
- Artifact paths outside the configured source-tree roots.
- Required graph nodes missing from the artifact registry or explicitly
  configured exemptions.
- Required graph edges missing for accepted, active, done, stable, or
  stabilizing artifacts.
- Accepted specs without `linked_proposal` or `standalone_reason`.
- Accepted ADRs without `linked_spec` or `standalone_reason`.
- Active plans without at least one linked proposal or spec.
- Unknown linked proposal, spec, ADR, plan, active-goal, support-tier, policy, or
  closeout references.
- Stable or stabilizing support-tier claims with empty proof-command fields.
- Done active-goal work items without closeout links when closeout requirements
  are enabled.
- Any attempt to satisfy the profile by suppressing findings, silently
  broadening policy, auto-extending expiry, or laundering `baseline_debt` into
  approval.

## Artifact Links

- Linked proposal:
  [CARGO-ALLOW-PROP-0001](../proposals/CARGO-ALLOW-PROP-0001-spec-system-profile.md)
- Linked ADR: none yet.
- Linked implementation plan:
  [CARGO-ALLOW-PLAN-0001](../../plans/spec-system/implementation-plan.md).
- Linked support-tier surface:
  [CARGO-ALLOW-SUPPORT-0001](../status/SUPPORT_TIERS.md).
- Linked active goal:
  [CARGO-ALLOW-GOAL-0001](../../.allow/goals/active.toml).
- Linked closeout:
  [CARGO-ALLOW-CLOSEOUT-0001](../../plans/spec-system/closeout.md).
- Linked policy ledgers: this spec declares policy impact for
  `policy/doc-artifacts.toml`, `policy/spec-system.toml`, and
  `policy/allow.toml`.

## Profile Config

The profile configuration should be TOML at `policy/spec-system.toml`.

Minimum shape:

```toml
schema_version = "1.0"
profile = "spec-system"
mode = "advisory"

[roots]
proposals = "docs/proposals"
specs = "docs/specs"
adrs = "docs/adr"
plans = "plans"
goals = ".allow/goals"
support_tiers = "docs/status/SUPPORT_TIERS.md"
artifact_ledger = "policy/doc-artifacts.toml"

[requirements]
ledger_required = true
templates_required = true
support_tiers_required = true
active_goal_required = true
closeout_required_for_done_items = true
```

Recognized modes are:

- `advisory`: report findings without blocking the command.
- `shadow`: report failure posture without blocking merge or default checks.
- `blocking`: fail selected structural checks.

Repos should start with `advisory`. Promotion to `shadow` or `blocking` should
be deliberate and limited to checks that have burned in.

## Doc Artifact Ledger

The artifact registry should be TOML at `policy/doc-artifacts.toml`.

Minimum shape:

```toml
schema_version = "1.0"
policy = "cargo-allow-doc-artifacts"
owner = "repo-infra"
status = "advisory"

[[artifact]]
id = "CARGO-ALLOW-PROP-0001"
kind = "proposal"
path = "docs/proposals/CARGO-ALLOW-PROP-0001-spec-system-profile.md"
status = "accepted"
owner = "repo-infra"
created = "2026-06-12"

[[artifact]]
id = "CARGO-ALLOW-SPEC-0001"
kind = "spec"
path = "docs/specs/CARGO-ALLOW-SPEC-0001-spec-system-profile.md"
status = "accepted"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
```

Recognized artifact kinds should include:

- `proposal`
- `spec`
- `adr`
- `implementation_plan`
- `plan_item`
- `active_goal`
- `support_tier`
- `policy_ledger`
- `closeout`

Recognized artifact statuses should include:

- `draft`
- `proposed`
- `accepted`
- `active`
- `done`
- `superseded`

Superseded artifacts must link to a valid replacement.

## Header Fields

Proposal, spec, ADR, plan, plan item, and closeout files should have lightweight
front matter or an equivalent visible header block.

The profile should compare header fields with the artifact ledger when both are
present:

- `id`
- `kind`
- `status`
- `owner`
- `created`
- linked proposal, spec, ADR, plan, support-tier, policy, and closeout fields
  relevant to the artifact kind
- `support_tier_impact`
- `policy_impact`

Header mismatches are structural findings. They are not proof failures.
Other Markdown formatting differences are out of scope unless they prevent the
profile from parsing identity, status, links, proof-command fields, or
support-tier rows.

## Support-Tier Impact

The profile affects the advisory support-tier surface for
source-of-truth graph linting:
[CARGO-ALLOW-SUPPORT-0001](../status/SUPPORT_TIERS.md).

Support tiers own user-facing claim to proof-command mapping. This spec defines
that the profile can structurally check the support-tier file and proof-command
fields. It does not duplicate support-tier rows or execute proof commands.

## Active Goal Impact

The profile should support
[`CARGO-ALLOW-GOAL-0001`](../../.allow/goals/active.toml) as an active
execution manifest when profile requirements enable active-goal checks.

The active goal manifest should link to known proposal, spec, and plan
artifacts. Work items should have IDs, titles, statuses, and proof-command
fields. Done work items should link to closeouts when closeout requirements are
enabled.

The active goal manifest is agent execution state. It is not product runtime
state, release state, or a replacement for proposals, specs, plans, support
tiers, or policy ledgers.

## Worklist Items

The profile should emit repair work items for structural graph findings.

Required work item fields:

- `kind`
- `artifact_id` when known
- `path` when known
- `owner` when known
- `status` when known
- `message`
- `suggested_actions`
- `proof_commands`

Initial work item kinds should include:

- `missing_node`
- `missing_doc_artifact`
- `artifact_file_missing`
- `artifact_id_not_in_file`
- `invalid_artifact_status`
- `missing_required_edge`
- `missing_linked_proposal`
- `unknown_link_target`
- `unknown_linked_artifact`
- `orphan_spec`
- `missing_support_tier`
- `missing_proof_command`
- `claim_without_support_tier`
- `stale_active_goal`
- `missing_closeout`
- `superseded_target_missing`

Worklist output is a routing surface. It must not auto-fix policy, suppress
findings, extend lifecycle dates, or claim proof execution.

## Policy Impact

The spec uses these policy surfaces:

- `policy/doc-artifacts.toml` for artifact registration.
- `policy/spec-system.toml` for profile roots and requirements.
- `policy/allow.toml` entries for tracked docs, TOML, JSON schema, config, or
  support files introduced while dogfooding.

This spec does not change current `policy/allow.toml` semantics for source
exceptions.

## Required Evidence

Profile implementation and ongoing maintenance evidence should include:

- config parsing tests for `policy/spec-system.toml`.
- artifact ledger parsing tests for `policy/doc-artifacts.toml`.
- validation tests for duplicate IDs, missing files, missing IDs in files,
  invalid kinds, invalid statuses, unknown links, and superseded replacements.
- support-tier structural tests for non-empty proof-command fields.
- active-goal structural tests for linked artifacts and done-item closeouts.
- CLI tests proving default cargo-allow behavior is unchanged without
  `--profile spec-system`.
- report, receipt, and worklist tests that include source-tree-only claim
  boundaries and scanner limitations.
- cargo-allow dogfood evidence in advisory and shadow modes before blocking
  promotion.

## Acceptance Examples

### Example: Accepted

Input:

```toml
schema_version = "1.0"
policy = "cargo-allow-doc-artifacts"
owner = "repo-infra"
status = "advisory"

[[artifact]]
id = "CARGO-ALLOW-PROP-0001"
kind = "proposal"
path = "docs/proposals/CARGO-ALLOW-PROP-0001-spec-system-profile.md"
status = "accepted"
owner = "repo-infra"
created = "2026-06-12"

[[artifact]]
id = "CARGO-ALLOW-SPEC-0001"
kind = "spec"
path = "docs/specs/CARGO-ALLOW-SPEC-0001-spec-system-profile.md"
status = "accepted"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
```

Expected output:

```text
accepted: artifact IDs are unique, files exist, visible IDs are present, and linked proposal resolves
```

### Example: Rejected

Input:

```toml
[[artifact]]
id = "CARGO-ALLOW-SPEC-0001"
kind = "spec"
path = "docs/specs/CARGO-ALLOW-SPEC-0001-spec-system-profile.md"
status = "accepted"
owner = "repo-infra"
created = "2026-06-12"
```

Expected output:

```text
rejected: accepted spec requires linked_proposal or standalone_reason
```

### Example: Out Of Scope

Input:

```text
proof_commands = ["cargo test --workspace"]
```

Expected output:

```text
the profile may verify that the field is non-empty; it must not run the command or claim it passed
```

## Non-Goals

- Do not make spec-system checks default.
- Do not execute proof commands.
- Do not call GitHub APIs, network services, ripr, unsafe-review, coverage, or
  other external evidence tools.
- Do not invoke Cargo, rustc, Clippy, build scripts, proc macros, or dependency
  resolution during profile scans.
- Do not prove semantic correctness, release readiness, unsafe soundness, test
  adequacy, coverage, or support-tier truth.
- Do not lint prose quality, heading style, line length, capitalization, or
  exact Markdown section order.
- Do not duplicate support-tier claim truth inside specs.
- Do not put full PR queues inside specs.
- Do not create a public `allow-spec` crate before the internal profile model is
  stable.

## Claim Boundary

This spec defines structural source-tree behavior for an opt-in profile. It does
not prove semantic correctness by itself.

Spec-system reports may claim:

- source-tree files were parsed.
- configured ledgers and roots were read.
- artifact IDs, paths, kinds, statuses, owners, and links were checked.
- support-tier proof-command fields were present or missing.
- active-goal and closeout references were present or missing.
- worklist items were emitted for structural repair.

Spec-system reports must not claim:

- proof commands ran.
- tests passed.
- GitHub or network state was checked.
- implementation behavior satisfies a spec.
- support-tier product claims are true.
- source exceptions are safer or semantically correct.

## Rollback Or Compatibility

The profile must remain removable or disabled by deleting or disabling
`policy/spec-system.toml`. Default cargo-allow source-exception behavior must
continue to work without spec-system profile files.

If this spec is superseded, the replacement spec should link back to
`CARGO-ALLOW-SPEC-0001`, update the doc artifact ledger, and preserve the
non-execution boundary unless a separate accepted proposal changes it.
