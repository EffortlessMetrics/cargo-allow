# Doc Artifact Ledger

`policy/allow.toml` remains the source-exception ledger. For this repository,
the governed artifact registry lives at `.allow/artifacts/doc-artifacts.toml`
(owned profile state). Legacy `policy/doc-artifacts.toml` remains a
compatibility fallback for repositories that have not migrated.

This ledger is part of the spec-system profile. cargo-allow reads it only when
a caller uses `--profile spec-system`; default source-exception checks continue
to use `policy/allow.toml`.

## Scope

The initial ledger registers accepted source-of-truth artifacts for the profile:

- `CARGO-ALLOW-PROP-0001`
- `CARGO-ALLOW-SPEC-0001`
- `CARGO-ALLOW-SUPPORT-0001`
- `CARGO-ALLOW-GOAL-0001`
- `CARGO-ALLOW-PLAN-0001`
- `CARGO-ALLOW-CLOSEOUT-0001`

Later source-of-truth artifacts should be added when they land:

- ADRs.
- policy ledgers.
- release records when they become governed artifacts.

The ledger should not replace the human-facing proposal, spec, plan, or
closeout documents. It is the durable index that lets a structural scanner build
the graph without guessing from filenames or chat history.

## Required Fields

The top-level ledger fields are:

| Field | Purpose |
| --- | --- |
| `schema_version` | Identifies the ledger format. |
| `policy` | Names this source-tree policy surface. |
| `owner` | Names the accountable owner for the registry. |
| `status` | Starts as `advisory` until dogfood promotes selected checks. |

Each `[[artifact]]` entry should include:

| Field | Purpose |
| --- | --- |
| `id` | Stable artifact ID used by graph edges. |
| `kind` | Artifact kind, such as `proposal` or `spec`. |
| `path` | Source-tree path to the artifact. |
| `status` | Artifact lifecycle state. |
| `owner` | Accountable owner for the artifact. |
| `created` | Date the artifact entered the governed stack. |

Kind-specific link fields should be added only when they describe real graph
edges. The initial spec row uses `linked_proposal` to connect
`CARGO-ALLOW-SPEC-0001` back to `CARGO-ALLOW-PROP-0001`.

## Graph Checks

The `spec-system` profile uses this ledger to validate static graph structure:

- artifact IDs are unique.
- artifact paths exist.
- artifact files contain their IDs.
- kinds and statuses are recognized.
- linked artifacts resolve.
- superseded artifacts name valid replacements.
- accepted specs link to proposals or provide standalone reasons.

These checks are structural. They do not execute proof commands, call GitHub,
run Cargo, or prove that a claim is semantically true.

## Claim Boundary

This ledger supports claims that source-tree artifacts were parsed, registered,
and linked by the opt-in profile. It must not be used to claim proof execution,
test adequacy, release readiness, GitHub state, support-tier truth, or semantic
correctness.

## Rollback

If the source-of-truth profile direction is withdrawn, remove
`policy/doc-artifacts.toml`, remove this page, and remove their
`policy/allow.toml` entries. Default cargo-allow source-exception behavior must
remain unchanged.
