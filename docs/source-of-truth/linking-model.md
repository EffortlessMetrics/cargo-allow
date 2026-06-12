# Linking Model

The source-of-truth stack is a linked graph, not a loose docs folder. Links
should make artifact ownership and drift visible.

## Profile Roots

The `spec-system` profile uses these source-tree roots:

```text
docs/proposals/
docs/specs/
docs/adr/
plans/
.codex/goals/
docs/status/SUPPORT_TIERS.md
policy/doc-artifacts.toml
policy/spec-system.toml
policy/allow.toml
plans/*/closeout.md
```

These roots belong to the opt-in profile. They are not required by the default
cargo-allow source-exception scan.

## Registry

`policy/doc-artifacts.toml` registers governed source-of-truth artifacts:

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

[[artifact]]
id = "CARGO-ALLOW-SUPPORT-0001"
kind = "support_tier"
path = "docs/status/SUPPORT_TIERS.md"
status = "active"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"

[[artifact]]
id = "CARGO-ALLOW-GOAL-0001"
kind = "active_goal"
path = ".codex/goals/active.toml"
status = "done"
owner = "codex"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"
linked_support_tier = "CARGO-ALLOW-SUPPORT-0001"
linked_plan = "plans/spec-system/implementation-plan.md"
linked_plan_status = "done"

[[artifact]]
id = "CARGO-ALLOW-PLAN-0001"
kind = "implementation_plan"
path = "plans/spec-system/implementation-plan.md"
status = "done"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"
linked_support_tier = "CARGO-ALLOW-SUPPORT-0001"
linked_goal = "CARGO-ALLOW-GOAL-0001"
linked_closeout = "CARGO-ALLOW-CLOSEOUT-0001"

[[artifact]]
id = "CARGO-ALLOW-CLOSEOUT-0001"
kind = "closeout"
path = "plans/spec-system/closeout.md"
status = "done"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"
linked_support_tier = "CARGO-ALLOW-SUPPORT-0001"
linked_goal = "CARGO-ALLOW-GOAL-0001"
linked_plan = "CARGO-ALLOW-PLAN-0001"
```

The registry is the machine-readable index. Markdown documents remain the human
explanation. The current advisory registry starts with the accepted
`CARGO-ALLOW-PROP-0001` proposal, `CARGO-ALLOW-SPEC-0001` spec, and
`CARGO-ALLOW-SUPPORT-0001` support-tier map, plus the completed
`CARGO-ALLOW-GOAL-0001` manifest, `CARGO-ALLOW-PLAN-0001` implementation plan,
and `CARGO-ALLOW-CLOSEOUT-0001`. Future policy-ledger or release-record
artifacts should be registered only as those governed artifacts land.

## Link Rules

The `spec-system` profile checks structural rules such as:

- `schema_version` exists.
- artifact IDs are unique.
- artifact paths exist.
- the artifact ID appears in the file.
- artifact kind matches the expected root.
- status is recognized.
- owner exists.
- accepted specs link to a proposal or provide a standalone reason.
- superseded artifacts link to a valid replacement.
- active plans link to at least one proposal or spec.
- active goal manifests link to known proposal, spec, and plan artifacts.
- done work items link to a closeout.

These rules are structural. They do not prove that the linked evidence is
correct or that the implementation satisfies the spec.

## Header Blocks

Proposal, spec, ADR, plan, and closeout files should use lightweight front
matter or an equivalent visible header block:

```yaml
---
id: CARGO-ALLOW-SPEC-0001
kind: spec
status: accepted
owner: repo-infra
created: 2026-06-12
linked_proposal: CARGO-ALLOW-PROP-0001
support_tier_impact: advisory
---
```

The profile can compare machine-readable fields with the registry as those
checks are implemented. This helps catch copy/paste drift, renamed files,
unknown links, and stale statuses.

## Support-Tier Links

Support tiers own user-facing claim to proof-command mapping. Specs should point
to the support-tier surface they affect instead of duplicating the full claim
map.

Structural checks include:

- `docs/status/SUPPORT_TIERS.md` exists when the profile requires it.
- stable and stabilizing claims have non-empty proof commands.
- specs with support-tier impact point to the support-tier map.
- stable README claims are represented where practical.

The profile must not execute proof commands or claim the proof passed.

## Failure Shape

Broken links should become worklist items, not vague prose. Work item kinds
include:

- `missing_doc_artifact`
- `artifact_file_missing`
- `artifact_id_not_in_file`
- `invalid_artifact_status`
- `missing_linked_proposal`
- `unknown_linked_artifact`
- `orphan_spec`
- `missing_support_tier`
- `missing_proof_command`
- `stale_active_goal`
- `missing_closeout`
- `superseded_target_missing`

Each item should include the artifact ID, path, kind, owner, status, message,
suggested actions, and proof commands when known.
