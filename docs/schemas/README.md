# cargo-allow JSON Schemas

These schemas describe machine-readable cargo-allow artifacts. They are local
contracts for source-tree policy scans; they do not imply build, type,
macro-expansion, or proof-level coverage.

| Artifact | Schema ID | Producer |
|---|---|---|
| Setup diagnostics | `cargo-allow.doctor.v1` | `cargo-allow doctor --format json` |
| Audit/check/diff report | `cargo-allow.report.v1` | `cargo-allow audit --format json`, `cargo-allow check --format json`, `cargo-allow diff --format json` |
| Check receipt | `cargo-allow.receipt.v1` | `cargo-allow check --receipt <path>` |
| Single-entry explanation | `cargo-allow.explain.v1` | `cargo-allow explain <id> --format json` |
| Filtered ledger list | `cargo-allow.list.v1` | `cargo-allow list --format json` |
| Stale prune preview/result | `cargo-allow.prune.v1` | `cargo-allow prune --stale --format json` |
| Baseline proposal summary | `cargo-allow.propose.v1` | `cargo-allow propose --summary-format json` |
| Single-entry add summary | `cargo-allow.add.v1` | `cargo-allow add --summary-format json` |
| Legacy migration summary | `cargo-allow.migrate.v1` | `cargo-allow migrate --summary-format json` |
| Agent worklist | `cargo-allow.worklist.v1` | `cargo-allow worklist --format json` |

## Files

- [doctor.schema.json](doctor.schema.json)
- [report.schema.json](report.schema.json)
- [receipt.schema.json](receipt.schema.json)
- [explain.schema.json](explain.schema.json)
- [list.schema.json](list.schema.json)
- [prune.schema.json](prune.schema.json)
- [propose.schema.json](propose.schema.json)
- [add.schema.json](add.schema.json)
- [migrate.schema.json](migrate.schema.json)
- [worklist.schema.json](worklist.schema.json)
- [common.v1.json](common.v1.json) shared source-tree fragments used as the
  tested vocabulary source for future schema consolidation. Artifact schemas
  remain self-contained for consumer portability. The shared catalog includes
  source-tree inventory, evidence-prefix vocabularies, and evidence diagnostic
  row shapes used by `explain` and `worklist`, plus selector-precision
  posture fragments used by `diff`.

## Contract Status

The `*.v1` schema IDs are the current machine-readable contract names. They are
intended for CI, review automation, and agent routing that need stable artifact
identity and source-tree claim-boundary fields.

Consumers may rely on these common root fields across all current cargo-allow
JSON artifacts:

- `schema_version`
- `schema_id`
- `tool`
- `command`
- `claim_boundary`
- `scanner_limitations`
- `inventory.scope`
- `inventory.scanner`
- `inventory.source`

When available, artifacts also include `inventory.root` and
`inventory.files_scanned`. Those values describe the local source-tree scan that
produced the artifact; `root` may be an absolute local path and should not be
treated as portable identity.

Artifact-specific fields such as `diff`, `summary`, `allow_entries`,
`work_items`, `stale_entries`, `allow_entry`, and evidence diagnostics are
covered by their individual schema files. Consumers should branch on
`schema_id`, not on command-line spelling or filenames.

List artifacts currently emit a `filters` object with every known filter key,
but nested filter fields are optional in the schema so older `cargo-allow.list.v1`
artifacts and future additive filter fields can remain compatible.
Worklist artifacts follow the same rule: current renderers emit all known
filter keys, while the nested `filters` schema keeps those keys optional.
Adoption summary artifacts such as `add` and `propose` also emit all known
`options` keys today, while nested option fields remain optional in the schema
for v1 compatibility.

The shared report schema is emitted by `audit`, `check`, and `diff`, but the
top-level `diff` posture extension is valid only on reports whose
`command = "diff"`. Audit and check reports use the same base schema without
the PR-posture extension.

## Contract Change Rules

Treat the JSON artifacts as producer-consumer contracts. A PR that changes an
artifact shape should name the affected producers, likely consumers, validation
commands, and compatibility story in its PR body.

Compatible `*.v1` changes include:

- adding optional fields that are safe for consumers to ignore;
- documenting a field more precisely without changing its wire shape;
- tightening renderer tests so existing fields stay stable;
- adding non-breaking examples or schema compatibility coverage.

Use extra care for any field additions because schemas enforce
`additionalProperties = false` at both the root and nested levels. Prefer
optional fields that are omitted when they have no signal, such as zero-count
summary fields, so older strict consumers are less likely to see unexpected
properties in ordinary outputs.

Breaking changes require a new schema ID or explicit migration note. Examples
include:

- removing, renaming, or changing the type of an existing field;
- making an optional field required;
- changing the meaning of an existing status, count, or posture field;
- removing enum values or reusing an enum value for a different meaning;
- changing source-tree claim-boundary or scanner-limitation semantics.

Enum additions are reviewed contract changes even when they are additive. Update
the schema file, renderer or parser, focused schema tests, and this index
together so agent and CI consumers can adapt deliberately.

## Baseline Debt Counts

Baseline debt can appear in two related but distinct places:

- `baseline_debt` is an outcome-status count. It reflects current check results
  that were classified as baseline debt outcomes.
- `policy_baseline_debt` is a policy-ledger count. It reflects retained
  `classification = "baseline_debt"` entries that remain in `policy/allow.toml`
  even when the current findings match and `check --mode no-new` passes.

Report JSON may include `summary.policy_baseline_debt` when the policy-level
count is higher than the outcome-level `summary.baseline_debt`. Check receipts
use the same distinction under `counts.policy_baseline_debt`. Consumers should
use the policy-level field for debt-burn-down metrics and the outcome-level
field for current check-status accounting.

Diff report `policy_changes` use `baseline_debt_added` when a PR adds generated
baseline debt, `baseline_debt_introduced` when an existing reviewed entry is
reclassified as generated baseline debt, and `baseline_debt_normalized` when an
existing `classification = "baseline_debt"` entry is reclassified as reviewed
policy. These are failing policy-posture signals so generated adoption debt
cannot be silently laundered into approval or introduced into reviewed entries.

Diff report `policy_changes` also distinguish traceability and provenance
changes. Removed policy `links` emit `link_removed` review items, added links
emit `link_added` improvements, removed `created` dates emit `created_removed`
failures, changed `created` dates emit `created_changed` review items, and
added `created` dates emit `created_added` improvements. These are posture
signals for the exception ledger only; they do not imply build-aware or
proof-level validation.

Diff report `policy_changes` use `owner_unassigned` when an existing reviewed
entry changes from a concrete owner to `owner = "unowned"`. This is a failing
policy-posture signal because retained exceptions must not silently lose
ownership.

Diff report `policy_changes` use `policy_owner_added`,
`policy_owner_changed`, `policy_owner_removed`, and `policy_owner_unassigned`
for top-level ledger-owner changes. These rows use the synthetic `allow_id`
value `policy.owner`. Removing a concrete policy owner or changing it to
`unowned` is failing policy weakening, adding a concrete owner is an
improvement, and changing one concrete owner to another requires review.

Diff report `policy_changes` use `policy_status_weakened`,
`policy_status_tightened`, and `policy_status_changed` for top-level policy
status changes. These rows use the synthetic `allow_id` value `policy.status`.
Changing `active` to `advisory` or removing an active status is failing policy
weakening; changing `advisory` or an unset status to `active` is a policy
improvement; other status transitions require review.

Diff report `policy_changes` use `requirement_loosened` and
`requirement_tightened` for policy-level `[requirements]` changes. The
`allow_id` field is a synthetic stable path such as
`requirements.owner_required` because these are ledger policy controls rather
than individual allow entries. Loosening is a failing policy-posture signal;
tightening is a policy improvement.

Diff report `policy_changes` use `workspace_ignored_added` and
`workspace_ignored_removed` for source-tree inventory exclusions. Added ignored
scopes fail because they can hide findings from the scan; removed ignored
scopes are improvements. `workspace_generated_added` and
`workspace_generated_removed` report generated-code scope changes. Added
generated scopes require review because they can reclassify non-Rust inventory,
while removed generated scopes are improvements. These rows use synthetic
`allow_id` values such as `workspace.ignored` or `workspace.generated`.

Diff report `policy_changes` use `scope_changed` for source-tree scope retargets
that are neither broadening nor narrowing, such as exact path changes or sibling
glob replacements. Consumers should treat these as review-required changes
because the retained exception now covers a different source-tree surface.
They also use `selector_changed` for equal-precision structural selector
retargets, such as changing `container`, `callee`, `symbol`, or snippet hash
values without changing the selector precision score. Line hints are excluded
from this identity signal.
Rows with `selector_precision_decreased` or `selector_precision_increased` may
also include an optional `selector_precision` object with before/after scores
and the selector fields added or removed. This lets consumers classify selector
weakening without parsing the human `message` string.

## Evidence Prefix Vocabulary

Evidence strings may use typed prefixes so cargo-allow can distinguish local
source-tree evidence from offline traceability. The policy parser owns this
vocabulary; [common.v1.json](common.v1.json) mirrors it for schema consumers.

| Prefix | Aliases | Treatment |
|---|---|---|
| `doc:` | | Local source-tree file reference |
| `spec:` | | Local source-tree file reference |
| `adr:` | | Local source-tree file reference |
| `ripr:` | | Local source-tree file reference |
| `unsafe-review:` | `unsafe_review:` | Local source-tree file reference |
| `coverage:` | | Local source-tree file reference |
| `test:` | | Traceability only; not executed or resolved |
| `cargo:` | | Traceability only; not executed or resolved |
| `issue:` | | Traceability only; not resolved over the network |
| `pr:` | | Traceability only; not resolved over the network |
| `legacy-policy:` | `legacy_policy:` | Traceability only |

Unknown prefixes and unstructured strings are reported as weak evidence, not as
broken local links. Empty traceability targets are also weak evidence.

## Broken Evidence Links

Report JSON may include `summary.broken_evidence_links` and
`trend.broken_evidence_links` when local evidence references such as `doc:`,
`spec:`, `adr:`, `ripr:`, `unsafe-review:`, or `coverage:` point outside the
source tree, point to a directory or symlinked path component, or point to a
missing file.
Receipts may use the same optional count under `counts.broken_evidence_links`.

Report JSON may include `summary.weak_evidence_references` and
`trend.weak_evidence_references` when retained evidence strings are
unstructured or use unknown prefixes. Receipts may use the same optional count
under `counts.weak_evidence_references`. These references are not broken local
links and do not, by themselves, fail `check`; they remain visible so teams can
replace weak traceability with typed evidence prefixes.

Report JSON may also include `summary.policy_missing_evidence` and
`trend.policy_missing_evidence` when retained non-baseline policy entries have
no evidence references even though they otherwise match current findings.
Receipts may use the same optional count under
`counts.policy_missing_evidence`. This is distinct from outcome-level
`evidence_missing`, which reflects enforced evidence requirements.

Read-only reporting and adoption producers treat these as evidence-health
signals so inventory and repair queues can still be emitted. That includes
`audit`, `diff`, `explain`, `list`, `worklist`, `propose`, and `prune --stale`
dry-run artifacts. `check` fails closed on broken local evidence links while
still including the count in saved report and receipt artifacts when those
outputs are requested. `doctor` reports invalid policy state, `add` validates
evidence before writing a reviewed entry, and `prune --stale --write`
revalidates the remaining policy before writing. Use
`cargo-allow worklist --item-kind broken_evidence_link --format json` for the
broken-link repair queue,
`cargo-allow worklist --item-kind weak_evidence_reference --format json` for
unstructured or unknown-prefix evidence cleanup, and
`cargo-allow worklist --missing-evidence --format json` for retained entries
that still need evidence references.
Worklist JSON items for broken or weak evidence diagnostics may include an
optional `evidence_reference` object with the original evidence string,
normalized prefix/target metadata, diagnostic status, and diagnostic message.
This object is evidence metadata; `work_items[].path` remains the source-tree
path for source-backed work or a local evidence path for broken local evidence
links.

## Compatibility Coverage

The test suite parses the current report, receipt, diff, list, explain,
worklist, prune, propose, add, migrate, and doctor JSON renderers as JSON and
checks the shared v1 source-tree contract fields. That protects the artifact
root shape from accidental manual-rendering drift.

Black-box integration tests also parse saved JSON artifacts written by the
`cargo-allow` binary itself, including `--output` report-style artifacts,
`check --receipt` receipts, and `--summary-output` add/propose/migrate
summaries. That protects the CLI file output boundary from drifting away from
the renderer-level schema contract.

Schema compatibility tests also lock:

- schema document metadata, including `$schema`, `$id`, and `title`;
- the exact top-level property set for each schema;
- the exact top-level required-field set for each schema;
- rendered sample artifacts against their registered schema top-level fields;
- `additionalProperties = false` at the artifact root and nested object schema
  nodes;
- shared source-tree inventory source values, including `git_tracked`,
  `filesystem_fallback`, `filesystem_include_untracked`, and `unknown`
  renderer defaults;
- shared evidence prefix and evidence-reference status vocabularies, including
  canonical prefixes, parser-recognized aliases, local-file evidence prefixes,
  traceability-only evidence prefixes, and evidence diagnostic statuses;
- report and receipt command producer values;
- shared claim-boundary and scanner-limitation vocabularies;
- report and receipt top-level status values;
- match-status vocabularies where artifacts expose current ledger status;
- governed source-exception kind vocabularies where artifacts expose finding or
  allow-entry kinds;
- structural identity fields where artifacts expose finding identity;
- `report.diff` posture, severity, finding-change, and policy-change
  vocabularies for PR posture weakening and improvement signals; and
- the `worklist` artifact's queue item kind, risk, and difficulty
  vocabularies; and
- `worklist` and `explain` proof commands remain standalone `cargo-allow`
  commands.

This is not a promise that every field is permanently frozen. Breaking changes
should either preserve the existing `*.v1` contract or introduce a new schema ID
and update the schema file, tests, and this index together.

## Consumer Guidance

Use JSON artifacts for automation:

- `report` and `receipt` for CI gates and stored no-new evidence.
- `diff` for PR posture review.
- `worklist` for agent routing.
- `list` and `explain` for policy lookup and interactive tooling.
- `doctor` for setup diagnostics before wider scans, including root discovery,
  inventory mode, config discovery, policy validation, and local evidence-file
  diagnostics.
- `propose` for generated-baseline review.
- `add` for one finding-to-policy-entry proposal summary.
- `migrate` for legacy-policy conversion receipts.
- `prune` for stale-entry cleanup previews or write receipts.

Do not parse human, Markdown, SARIF, or HTML output as the primary contract when
a JSON artifact exists for the same workflow. Markdown and HTML are review
surfaces; JSON is the machine surface.

## Boundary

Every schema carries explicit source-tree claim-boundary and scanner-limitation
fields. Current cargo-allow artifacts mean:

```text
No new unreceipted findings were found in scanned source-tree inventory.
```

They do not mean:

```text
No unsafe, panic, lint suppression, or policy exception exists outside the
syntax-visible inventory that cargo-allow scanned.
```

cargo-allow does not invoke Cargo metadata, Cargo commands, rustc, Clippy,
build scripts, proc macros, or repository code for these scans.
