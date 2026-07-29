# cargo-allow JSON Schemas

These schemas describe machine-readable cargo-allow artifacts. They are local
contracts for source-tree policy scans; they do not imply build, type,
macro-expansion, or proof-level coverage.

| Artifact | Schema ID | Producer |
|---|---|---|
| Setup diagnostics | `cargo-allow.doctor.v1` | `cargo-allow doctor --format json` |
| Audit/check/diff report | `cargo-allow.report.v1` | `cargo-allow audit --format json`, `cargo-allow check --format json`, `cargo-allow diff --format json` |
| Check/diff receipt | `cargo-allow.receipt.v1` | `cargo-allow check --receipt <path>`, `cargo-allow diff --receipt <path>` |
| Single-entry explanation | `cargo-allow.explain.v1` | `cargo-allow explain <id> --format json` |
| Unreceipted finding explanation | `cargo-allow.why.v1` | `cargo-allow why --kind <kind> --path <path> --line <line> --format json` |
| Add-finding plan | `cargo-allow.add-finding-plan.v1` | `cargo-allow why --kind <kind> --path <path> --line <line> --plan <path>` |
| Add plan application receipt | `cargo-allow.add-plan-application.v1` | `cargo-allow add --from-plan <path> --owner <owner> --reason <reason> --update` |
| Filtered ledger list | `cargo-allow.list.v1` | `cargo-allow list --format json` |
| Stale prune preview/result | `cargo-allow.prune.v1` | `cargo-allow prune --stale --format json` |
| Advisory drift refresh receipt | `cargo-allow.refresh.v1` | `cargo-allow refresh --allow-id <id> --format json` |
| Baseline proposal summary | `cargo-allow.propose.v1` | `cargo-allow propose --summary-format json --summary-output <path>` |
| Single-entry add summary | `cargo-allow.add.v1` | `cargo-allow add --summary-format json --summary-output <path>` |
| Legacy migration summary | `cargo-allow.migrate.v1` | `cargo-allow migrate --summary-format json --summary-output <path>` |
| Spec-system graph report | `cargo-allow.spec-system.v1` | `cargo-allow check --profile spec-system --format json`, `cargo-allow audit --profile spec-system --format json`, `cargo-allow worklist --profile spec-system --format json`, `cargo-allow doctor --profile spec-system --format json`, `cargo-allow explain <artifact-id> --profile spec-system --format json` |
| Agent worklist | `cargo-allow.worklist.v1` | `cargo-allow worklist --format json` |

## Self-description contract (not a governed artifact)

| Self-description | Schema ID | Producer |
|---|---|---|
| Tool identity | `cargo-allow.tool-identity.v1` | `cargo-allow tool identity --format json` |

The tool-identity contract carries `schema_id`/`schema_version` for
self-description but is **not** a governed artifact: it omits
`claim_boundary`, `scanner_limitations`, and `inventory`. Consumers should
treat it as a build-provenance and compatibility-checking envelope, not as a
source-tree scan result.

## Files

- [doctor.schema.json](doctor.schema.json)
- [report.schema.json](report.schema.json)
- [receipt.schema.json](receipt.schema.json)
- [explain.schema.json](explain.schema.json)
- [why.schema.json](why.schema.json)
- [add-finding-plan.schema.json](add-finding-plan.schema.json)
- [add-plan-application.schema.json](add-plan-application.schema.json)
- [list.schema.json](list.schema.json)
- [prune.schema.json](prune.schema.json)
- [refresh.schema.json](refresh.schema.json)
- [propose.schema.json](propose.schema.json)
- [add.schema.json](add.schema.json), [propose.schema.json](propose.schema.json), [refresh.schema.json](refresh.schema.json), [prune.schema.json](prune.schema.json), and [migrate.schema.json](migrate.schema.json) embed the shared `mutation_receipt`
  fragment (`cargo-allow.mutation-receipt.v1`): a provenance envelope
  (`operation`, `tool_version`, `repo_root`, `config_source`, `ledger_ids`,
  `changed_allow_ids`, `before_fingerprints`, `after_fingerprints`, `result`,
  `next_commands`, `claim_boundary`) per CARGO-ALLOW-SPEC-0008 "Mutation
  Receipt Envelope" (GOAL-0004 PR 5). Migration-specific counts, queues, and
  closeout semantics remain in the command-specific summary and closeout
  fields.
- [migrate.schema.json](migrate.schema.json)
- [spec-system.schema.json](spec-system.schema.json)
- [worklist.schema.json](worklist.schema.json)
- [tool-identity.schema.json](tool-identity.schema.json) self-description contract (not a governed artifact)
- [common.v1.json](common.v1.json) shared source-tree fragments used as the
  tested vocabulary source for future schema consolidation. Artifact schemas
  remain self-contained for consumer portability. The shared catalog includes
  source-tree inventory, governed source-exception kind and match-status
  vocabularies, structural identity, selector, allow-entry shapes, source
  finding rows, stale-prune rows, evidence-prefix vocabularies, evidence and
  link diagnostic row shapes used by `explain` and `worklist`, worklist filters and
  work-item rows, report inventory, summary, trend, audit remediation, finding,
  and outcome rows, receipt count rows, the diff posture object, diff-summary, finding-change,
  policy-change, selector-identity, selector-precision, exception-identity,
  scope-change, occurrence-limit, lifecycle, evidence-change, metadata-change,
  requirement-change, and policy-status fragments used by `diff`.

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

The cargo-allow contract suite also validates the checked-in producer samples
against every governed schema with the Rust `jsonschema` validator. This proves
the sample/rendering fixtures conform to the published shapes; it does not
claim that every runtime branch or arbitrary consumer payload has been
exhaustively exercised.
The `cargo-allow.add-finding-plan.v1` artifact binds an operator-reviewed add
plan to repository, inventory, policy, finding, and source-file identities. Its
`proof_plans` carry authoritative program plus ordered argv; consumers must not
shell-split or reconstruct those arguments from human text. The artifact plans
a later mutation and does not itself write policy or execute proof commands.
Its `required_fields` are derived from the active policy requirements;
`review_after_or_expires` means the later reviewed decision must supply at
least one of those lifecycle fields, and `evidence` appears only when the
active policy or finding kind requires it.
The `cargo-allow.add-plan-application.v1` artifact is the receipt emitted after
`add --from-plan` verifies an add-finding plan against a fresh scan and applies
it. Unlike the plan, this receipt records a mutation that already happened: it
binds `plan_digest`, `finding_digest`, `repository_identity`, the
`policy_before_digest`/`policy_after_digest` pair, `added_allow_id`, and the
`target_ledger` it replaced. It does not claim `policy_not_mutated`, and it is
honest that no recheck ran — `targeted_recheck` is always `not_executed` and
`full_check_argv` carries the authoritative program-plus-argv the operator must
run to confirm the full-repository posture.
Source location fields such as `span.line`, `span.column`, `line_hint`, and
`column_hint` are review and navigation hints. They are one-based source
positions; when cargo-allow can derive a column from source text, the column is
a character position in the line rather than a byte offset. Consumers must not
use those fields as durable identity.

Every `report.schema.json` `$defs` fragment is mirrored in
`common.v1.json` and covered by shared compatibility tests. Future report
fragments should be added to both files together so audit, check, and diff
consumers do not lose the shared contract catalog.

List artifacts currently emit a `filters` object with every known filter key,
but nested filter fields are optional in the schema so older `cargo-allow.list.v1`
artifacts and future additive filter fields can remain compatible.
List row evidence-health fields such as `broken_evidence_references` and
`weak_evidence_references` are also optional in the schema and emitted only when
non-zero so older `cargo-allow.list.v1` artifacts remain valid while current
renderers expose the per-entry evidence repair signal. The list filter object
also includes optional `broken_evidence` and `weak_evidence` booleans for saved
filtered evidence-health views.
List row metadata fields `family`, `source_package`, `review_after`, and
`expires` follow omit-when-unavailable semantics: current renderers omit them
when no value exists rather than emitting JSON `null`. The `filters` object is
different by design: its nullable fields use `null` to represent no filter.
Worklist policy metadata follows the same omission law: unavailable
`exception_kind`, `family`, `owner`, `classification`, `reason`, `created`,
`review_after`, `expires`, and `source_package` fields are omitted. Worklist
relationship fields such as `evidence_count`, `finding_index`, and `path`,
along with nullable filter fields, retain `null` when the relationship or
filter is not applicable.
Worklist artifacts follow the same rule: current renderers emit all known
filter keys, while the nested `filters` schema keeps those keys optional.
Non-empty worklist artifacts may also include `summary.item_kinds`, an optional
object that counts emitted work items by queue item kind so consumers can route
the queue without parsing every row. Its keys are limited to the stable
worklist item-kind vocabulary.
Worklist item fields such as `selector_precision` are optional and emitted only
for policy-backed work items where cargo-allow can score the related selector.
The score is routing metadata for narrowing and review work, not proof that the
exception is correct.
Adoption summary artifacts such as `add` and `propose` also emit all known
`options` keys today, while nested option fields remain optional in the schema
for v1 compatibility.
`propose` summary artifacts emit `unsafe_baseline_debt_entries_proposed` today,
while that summary field remains optional in the schema so older
`cargo-allow.propose.v1` artifacts without the unsafe-specific count remain
valid.
When proposal generation creates temporary baseline debt, current `propose`
JSON artifacts may also include an optional `follow_up_queues` array with stable
`signal` names, human `label` strings, machine `route_kind` values, stable
worklist `item_kind` names, optional `worklist_filter` values, routed `count`
values, and exact `cargo-allow worklist ... --format json` commands. These
queues point generated baseline debt at the existing baseline-debt and weak
unsafe-evidence worklist routes; they do not convert generated debt into
approval.
`migrate` summary artifacts may include `summary.lint_exception_entries`,
`summary.evidence_entries`, `summary.entries_with_links`,
`summary.link_entries`,
`summary.broken_evidence_links`, `summary.unsafe_broken_evidence_links`,
`summary.weak_evidence_references`, and
`summary.unsafe_weak_evidence_references` as optional migration-health counts.
The current renderer emits the lint count, emits `evidence_entries` as the total
number of `evidence` values carried into the migrated canonical policy, emits
link counts for canonical traceability links carried into the migrated policy,
emits broken local evidence links only when migrated references point to
missing or invalid local paths, emits weak evidence references only when legacy
conversion preserved unstructured or unknown-prefix evidence, and emits the
unsafe-specific evidence-health counts only when those references belong to
migrated unsafe entries. When those
baseline-debt counts are non-zero, current `migrate` JSON artifacts may include
an optional `follow_up_queues` array with stable migration `signal` names, human
`label` strings, machine `route_kind` values, stable worklist `item_kind` names,
counts, and exact `cargo-allow worklist ... --format json` commands. Current
migration follow-up queues use `route_kind = "worklist_item_kind"` for
`baseline_debt`.
When evidence-health counts are non-zero, current `migrate` JSON artifacts may
also include an optional `evidence_repair_queues` array with stable
evidence-health `signal` names, human `label` strings, machine `route_kind`
values, stable worklist `item_kind` names, total and unsafe-specific counts,
and the exact `cargo-allow worklist --item-kind ... --format json` command for
each repair queue. When `unsafe_count` is non-zero, queue rows may also include
an `unsafe_command` with the corresponding `--kind unsafe` worklist route.
Current migration repair queues use `route_kind = "worklist_item_kind"`.

The shared report schema is emitted by `audit`, `check`, and `diff`, but the
top-level `diff` posture extension is valid only on reports whose
`command = "diff"`. Audit and check reports use the same base schema without
the PR-posture extension.
The spec-system profile emits its own `cargo-allow.spec-system.v1` graph report
schema instead of the source-exception `report` schema. Its inventory scanner
is `source_tree_graph`, its claim boundary includes
`source_tree_graph_validation`, and its scanner limitations include
`proof_commands_not_executed`; this profile records structural relationships in
the source tree and does not execute proof commands or external services. When
produced by `worklist --profile spec-system`, the same schema carries graph
repair items for missing nodes, broken links, missing closeouts, and missing
claim-to-proof commands. When produced by `doctor --profile spec-system`, it
also carries setup readiness for profile config, artifact roots, the artifact
ledger, support tiers, active goals, and templates. The root `mode` field
comes from `.allow/profiles/spec-system.toml`, legacy `policy/spec-system.toml`,
or the built-in advisory default. Advisory
findings keep `status = "passed"` and `failed = false`; shadow findings set
`status = "failed"` and `failed = true` so CI and agents can see failure posture
without making this profile part of default cargo-allow behavior.
Blocking mode uses the same artifact posture fields and only fails commands for
findings with `blocking_eligible = true`. The initial blocking-eligible classes
are objective structural failures such as malformed explicit profile config,
missing or invalid doc-artifact ledgers, duplicate artifact IDs, invalid
artifact kinds or statuses, missing registered files, missing declared IDs, and
unknown link targets. Current producers include optional summary counts that
separate blocking-eligible and advisory findings and work items so reviewers can
scan the safe structural subset without losing lifecycle context. Work items
also include optional `blocking_eligible` and `blocking_reason` fields so agents
can route objective repairs separately from judgment-heavy lifecycle checks,
which remain advisory.
Diff reports may include optional `diff.summary.broken_evidence_links`,
`diff.summary.missing_evidence`, and `diff.summary.weak_evidence_references`
when the compared head policy has evidence-health signals. These duplicate the
base report evidence-health counts inside the PR-posture summary so JSON diff
consumers do not need to join across artifact sections to explain why the net
posture worsened.
Diff reports may include optional structural posture counts such as
`diff.summary.scope_broadened`, `diff.summary.scope_changed`,
`diff.summary.scope_narrowed`, `diff.summary.selector_changed`,
`diff.summary.selector_precision_decreased`, and
`diff.summary.selector_precision_increased` when the corresponding
`diff.policy_changes[].kind` rows are present. These fields summarize existing
row kinds for reviewers and automation; they do not replace row-level scope or
selector detail.
Diff reports may also include optional `diff.summary.evidence_added`,
`diff.summary.weak_evidence_added`, `diff.summary.broken_evidence_added`,
`diff.summary.evidence_removed`, severity-derived evidence-removal counts,
`diff.summary.link_added`, `diff.summary.weak_link_added`,
`diff.summary.broken_link_added`, `diff.summary.link_removed`, and
severity-derived link-removal counts when policy evidence or traceability-link
rows changed in the compared PR posture. The generic added/removed counts
summarize the corresponding `diff.policy_changes[].kind` rows; weak and broken
added counts summarize review/fail additions so consumers can flag weak or
broken evidence introductions without parsing row messages. Removal failure,
review, and improvement counts summarize the existing row severity for
evidence/link removals. The rows remain the source for severity, message, and
added/removed values.
Diff finding-change rows may include optional `diff.finding_changes[].line` and
`diff.finding_changes[].column` source locations for review/navigation only;
they are not part of stable finding identity. Rows may include optional
`diff.finding_changes[].identity` structural context showing the source-syntax
identity used by posture matching; within that object, `line_hint` and
`column_hint` remain review hints. Rows may also include optional
`diff.finding_changes[].source_package` when cargo-allow can derive package
context from source-tree `Cargo.toml` text; this is routing context, not Cargo
metadata.
Report JSON may also include an optional top-level `evidence_repair_queues`
array when audit, check, or diff reports have broken local evidence links,
missing evidence, or weak evidence references. Queue rows include a stable
`signal`, a human `label`, machine `route_kind`, a stable worklist `item_kind`
when one is available, route-specific `worklist_filter` values when applicable,
the routed `count`, and the exact `cargo-allow worklist ... --format json`
command so CI and agents can route evidence repair work without parsing human
text or shell command strings.
Audit report JSON may also include an optional top-level
`audit_remediation_roadmap` array when `command = "audit"` and the first-run
inventory has review or repair signals. Rows include a stable `signal`, human
`label`, machine `route_kind`, stable `item_kind` when one is associated with
the work, route-specific `worklist_status` or `worklist_filter` values when
applicable, the routed `count`, and an exact follow-up command such as a focused
`cargo-allow worklist ... --format json` queue or the stale-prune dry-run preview
command. This is the machine-readable counterpart to the human, Markdown, and
HTML audit remediation roadmap.
Report JSON may also include an optional `source_inventory` object when source
findings are present. This is the machine-readable counterpart to the audit
source-exception inventory, grouped by governed exception kind and
`kind.family`, so consumers do not need to re-aggregate the full `findings`
array to answer first-run inventory questions.
Check and diff receipts may also include the same optional `source_inventory` object
when the receipt is produced from a source-tree scan with findings. This lets
stored no-new evidence carry both the gate counts and the source-exception
inventory without requiring consumers to archive the full report artifact.
Check and diff receipts may also include an optional `evidence_repair_queues` array when
the saved no-new evidence has broken local evidence links, missing evidence, or
weak evidence references. These rows mirror report JSON queue routing, including
`signal`, `label`, machine `route_kind`, optional `item_kind`, optional
`worklist_filter`, `count`, and `command`, so CI artifacts can point directly at
the worklist command needed to repair retained evidence gaps.
Doctor JSON may include optional `config.evidence_repair_queues` rows when setup
diagnostics find broken local evidence links or weak evidence references in the
loaded policy. The rows include `signal`, `label`, machine `route_kind`,
`item_kind`, `count`, and `command` so root/config readiness checks stay
machine-routable while preserving the source-tree/no-code-execution claim
boundary.

## Claim Boundary Vocabulary

Every JSON artifact includes `claim_boundary`. Every source-syntax and
policy-migration artifact also includes `scanner_limitations`. The scanner
limitation vocabulary is the execution/analyzer subset of the broader claim
boundary; `source_tree_inventory` and `source_syntax_only` are claims about the
artifact's scan surface, not limitations.

| Value | Scanner limitation? | Meaning |
|---|---:|---|
| `source_tree_inventory` | No | Artifact is scoped to the scanned source-tree inventory. |
| `source_syntax_only` | No | Findings and policy posture are based on source syntax, not compiled output. |
| `cargo_metadata_not_invoked` | Yes | Cargo metadata was not invoked to produce the scan. |
| `cargo_commands_not_invoked` | Yes | Cargo commands were not invoked to produce the scan. |
| `rustc_not_invoked` | Yes | rustc was not invoked to produce the scan. |
| `clippy_not_invoked` | Yes | Clippy was not invoked to produce the scan. |
| `build_scripts_not_executed` | Yes | build scripts were not executed. |
| `proc_macros_not_executed` | Yes | proc macros were not executed. |
| `macro_expansion_not_analyzed` | Yes | macro-expanded Rust was not analyzed. |
| `macro_token_tree_contents_not_analyzed` | Yes | macro token-tree contents were not parsed as Rust expressions. |
| `type_information_not_analyzed` | Yes | type information and trait resolution were not analyzed. |
| `mir_not_analyzed` | Yes | MIR was not analyzed. |
| `build_output_not_analyzed` | Yes | build output and compiler output were not analyzed. |
| `control_flow_not_analyzed` | Yes | control-flow analysis was not performed. |
| `data_flow_not_analyzed` | Yes | data-flow analysis was not performed. |
| `external_evidence_tools_not_invoked` | Yes | External evidence tools can be referenced by policy but were not executed by the scan. |
| `repository_code_not_executed` | Yes | Repository code was not executed by cargo-allow. |
| `source_text_in_identity_fields` | No | Identity fields (symbol, callee, container, module, macro_name, lint) carry source-derived text and are emitted in CI artifacts; set `CARGO_ALLOW_REDACT_IDENTITY=1` to redact them. |

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
Conditional constraint subschemas may use `properties` only to constrain a
specific existing field, such as requiring `command = "diff"` when the report
contains a `diff` extension. Those constraint subschemas are not full object
shapes and must not reject the artifact's normal top-level fields.

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
Added local evidence references that are missing or invalid in the scanned
source tree remain `evidence_added` rows, but their severity is `fail` and their
message identifies broken local evidence. cargo-allow validates only the local
source-tree path; it does not execute or prove the referenced evidence.

Diff report `policy_changes` use `kind_changed` and `family_changed` when an
existing entry changes the governed exception identity it receipts. These are
failing policy-posture signals because the retained receipt now covers a
different source-exception class. These rows may include an optional
`exception_identity` object with the changed field and before/after values.

Diff report `policy_changes` use `owner_unassigned` when an existing reviewed
entry changes from a concrete owner to `owner = "unowned"`. This is a failing
policy-posture signal because retained exceptions must not silently lose
ownership.

Diff report `policy_changes` use `policy_owner_added`,
`policy_owner_changed`, `policy_owner_removed`, and `policy_owner_unassigned`
for top-level ledger-owner changes. These rows use the synthetic `allow_id`
value `policy.owner`. Removing a concrete policy owner or changing it to
`unowned` is failing policy weakening, adding a concrete owner is an
improvement, and changing one concrete owner to another requires review. These
rows may include an optional `metadata` object with the before/after owner.

Diff report `policy_changes` use `policy_status_weakened`,
`policy_status_tightened`, and `policy_status_changed` for top-level policy
status changes. These rows use the synthetic `allow_id` value `policy.status`.
Changing `active` to `advisory` or removing an active status is failing policy
weakening; changing `advisory` or an unset status to `active` is a policy
improvement; other status transitions require review. These rows may include an
optional `policy_status` object with before/after status values.

Diff report `policy_changes` use `requirement_loosened` and
`requirement_tightened` for policy-level `[requirements]` changes. The
`allow_id` field is a synthetic stable path such as
`requirements.owner_required` because these are ledger policy controls rather
than individual allow entries. Loosening is a failing policy-posture signal;
tightening is a policy improvement. These rows may include an optional
`requirement` object with the changed requirement and before/after boolean
values.

Diff report `policy_changes` use `workspace_ignored_added` and
`workspace_ignored_removed` for source-tree inventory exclusions. Added ignored
scopes fail because they can hide findings from the scan; removed ignored
scopes are improvements. `workspace_generated_added` and
`workspace_generated_removed` report generated-code scope changes. Added
generated scopes require review because they can reclassify non-Rust inventory,
while removed generated scopes are improvements. These rows use synthetic
`allow_id` values such as `workspace.ignored` or `workspace.generated`, and may
include an optional `scope` object with the added or removed source-tree scope.

Diff report `policy_changes` use `scope_changed` for source-tree scope retargets
that are neither broadening nor narrowing, such as exact path changes or sibling
glob replacements. Consumers should treat these as review-required changes
because the retained exception now covers a different source-tree surface.
They also use `selector_changed` for equal-precision structural selector
retargets, such as changing `container`, `callee`, `symbol`, or snippet hash
values without changing the selector precision score. Line hints are excluded
from this identity signal. Rows with `selector_changed` may also include an
optional `selector_identity` object listing the structural selector fields whose
normalized values changed.
Rows with `selector_precision_decreased` or `selector_precision_increased` may
also include an optional `selector_precision` object with before/after scores
and the selector fields added or removed. This lets consumers classify selector
weakening without parsing the human `message` string.
Rows with `scope_broadened`, `scope_narrowed`, or `scope_changed` may also
include an optional `scope` object with the changed scope carrier and normalized
before/after source-tree scopes. This lets consumers review scope movement
without parsing the human `message` string.
The same row kinds may also be summarized in
`diff.summary.scope_broadened`, `diff.summary.scope_changed`,
`diff.summary.scope_narrowed`, `diff.summary.selector_changed`,
`diff.summary.selector_precision_decreased`, and
`diff.summary.selector_precision_increased`.
Rows with `occurrence_limit_loosened` or `occurrence_limit_tightened` may also
include an optional `occurrence_limit` object with before/after count values.
This lets consumers distinguish capped baseline changes from unlimited approval
without parsing the human `message` string.
Rows with `created_added`, `created_changed`, `created_removed`,
`expiry_extended`, `expiry_shortened`, `review_after_extended`, or
`review_after_shortened` may also include an optional `lifecycle` object with
the changed lifecycle field and before/after values.
Rows with `evidence_added`, `evidence_removed`, `link_added`, or `link_removed`
may also include an optional `evidence` object with the changed collection and
added/removed values. `evidence_added` is an improvement for typed evidence
references, but review-required when the added value is weak evidence such as an
unstructured string, unknown prefix, or empty typed reference, and fails when
added local evidence is invalid, missing, or outside the compared inventory.
Weak or broken evidence additions may also be counted in
`diff.summary.weak_evidence_added` or `diff.summary.broken_evidence_added`.
`evidence_removed` fails when typed evidence is removed, improves posture when
only weak evidence is removed and typed evidence remains, and requires review
when weak evidence is removed without any remaining typed evidence.
Those severities may also be summarized in
`diff.summary.evidence_removal_failures`,
`diff.summary.evidence_removal_review_items`, and
`diff.summary.evidence_removal_improvements`.
`link_removed` fails for local traceability-link removal, improves posture when
only weak traceability links are removed and typed traceability remains, and
otherwise requires review. `link_added` follows the same
improvement/review/fail local-link rules as added evidence. Weak or broken link
additions may also be counted in `diff.summary.weak_link_added` or
`diff.summary.broken_link_added`. Link-removal severities may also be
summarized in `diff.summary.link_removal_failures`,
`diff.summary.link_removal_review_items`, and
`diff.summary.link_removal_improvements`.
Rows with `owner_added`, `owner_changed`, `owner_removed`, `owner_unassigned`,
`reason_added`, `reason_changed`, `reason_removed`, `classification_added`,
`classification_changed`, `classification_removed`,
`baseline_debt_introduced`, or `baseline_debt_normalized` may also include an
optional `metadata` object with the changed field and before/after values.

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
`trend.broken_evidence_links` when local evidence or link references such as
`doc:`, `spec:`, `adr:`, `ripr:`, `unsafe-review:`, or `coverage:` point
outside the source tree, point to a directory or symlinked path component, or
point to a missing file.
Receipts may use the same optional count under `counts.broken_evidence_links`.

Report JSON may include `summary.weak_evidence_references` and
`trend.weak_evidence_references` when retained evidence or link strings are
unstructured or use unknown prefixes. Receipts may use the same optional count
under `counts.weak_evidence_references`. These references are not broken local
links and do not, by themselves, fail `check`; they remain visible so teams can
replace weak evidence or traceability references with recognized prefixes.

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
`cargo-allow worklist --broken-evidence --format json` for the
broken-link repair queue,
`cargo-allow worklist --weak-evidence --format json` for
unstructured or unknown-prefix evidence/link cleanup, and
the default `cargo-allow worklist --format json` queue for retained entries
that still need evidence references. Use
`cargo-allow worklist --missing-evidence --format json` to focus only those
missing-evidence entries.
Worklist JSON items for broken or weak evidence/link diagnostics may include an
optional `evidence_reference` object with the original evidence string,
normalized prefix/target metadata, diagnostic status, optional diagnostic
category, and diagnostic message. The stable `status` field preserves the
machine compatibility vocabulary; the optional `category` field exposes the
human-facing repair bucket (`present`, `missing`, `invalid_local_path`,
`not_local`, `unknown_prefix`, or `untyped`) without requiring consumers to
parse diagnostic text.
This object is evidence metadata; `work_items[].path` remains the source-tree
path for source-backed work or a local evidence path for broken local evidence
links.

## Compatibility Coverage

The test suite parses the current report, receipt, diff, list, explain,
worklist, prune, propose, add, migrate, and doctor JSON renderers as JSON and
checks the shared v1 source-tree contract fields. That protects the artifact
root shape from accidental manual-rendering drift.
Doctor artifacts may include optional `config.broken_evidence_links` and
`config.weak_evidence_references` counts when setup diagnostics can load a
policy model. When a policy model is available, current doctor artifacts emit
these counts even when they are zero so healthy evidence setup is explicit. The
fields are omitted when no policy model is available.
When no policy config is found, doctor JSON may include
`config.suggested_init_command` with the standalone `cargo-allow init --root`
command for the diagnosed source tree. This field is additive and is omitted
when a config is found.
Doctor config metadata follows omit-when-absent semantics: `config.found` is
always present, while absent path, policy metadata, validity, and diagnostics
are omitted instead of represented as JSON `null`. Evidence counters retain
their existing behavior: present, including zero, when a policy model loads;
omitted when no policy model is available.

Black-box integration tests also parse saved JSON artifacts written by the
`cargo-allow` binary itself, including `--output` report-style artifacts,
`check --receipt` and `diff --receipt` receipts, and `--summary-output` add/propose/migrate
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
  `git_index_staged_candidate`, `filesystem_fallback`,
  `filesystem_include_untracked`, and `unknown`
  renderer defaults;
- shared evidence prefix, evidence-reference status, and diagnostic category vocabularies, including
  canonical prefixes, parser-recognized aliases, local-file evidence prefixes,
  traceability-only evidence prefixes, and evidence diagnostic statuses;
- artifact-local fragments mirrored in `common.v1.json` keep the same wire
  shape, including structural identity and diff posture detail fragments;
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
  commands; and
- the `add`, `propose`, `refresh`, `prune`, and `migrate` artifacts'
  `mutation_receipt` envelopes (GOAL-0004 PR 5) keep the required field set and
  `cargo-allow.mutation-receipt.v1` `schema_id`.

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
- `spec-system` for opt-in proposal/spec/ADR/plan/goal/support-tier/closeout
  graph reports.

Do not parse human, Markdown, SARIF, or HTML output as the primary contract when
a JSON artifact exists for the same workflow. Markdown and HTML are review
surfaces; JSON is the machine surface. SARIF is for code-scanning ingestion.
Its run properties may include advisory policy/evidence-health counts such as
`policy_baseline_debt`, `policy_missing_evidence`, `broken_evidence_links`, and
`weak_evidence_references`. SARIF run properties may also include an optional
`evidence_repair_queues` array with `signal`, `label`, machine `route_kind`,
optional `item_kind`, optional `worklist_filter`, `count`, and exact
`cargo-allow worklist ... --format json` command rows for those advisory
evidence-health signals, but SARIF results remain limited to non-matched
source-tree outcomes rather than synthetic policy-health rows.

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
build scripts, proc macros, external evidence tools, or repository code for
these scans. External evidence tools include dependency policy, crate audit,
test adequacy, unsafe review, and coverage tools whose outputs may be linked as
evidence but are not executed by cargo-allow.
