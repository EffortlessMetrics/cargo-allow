# Source Exception Ledger

The ledger is the central cargo-allow artifact. It records exceptions that are
allowed to remain in a repository and the conditions under which they remain
acceptable.

The canonical policy path is:

```text
policy/allow.toml
```

The primary command form is `cargo-allow ...`. `cargo allow ...` is accepted as
Cargo external subcommand compatibility, but cargo-allow's own scan is a direct
source-tree policy scan.

Top-level policy header values such as `schema_version`, `policy`, `owner`, and
`status` are exact ledger tokens. They must not include leading or trailing
whitespace.

## Entry Contract

Each retained exception should have:

- `id`: stable allow ID.
- `kind`: governed surface, such as `panic`, `unsafe`, `lint_exception`,
  `non_rust_file`, or `generated_code`.
- `family`: more specific category when available.
- `path` or `glob`: file scope.
- `owner`: owning team or maintainer.
- `classification`: why this class of exception exists.
- `reason`: why this exact exception is acceptable.
- `evidence`: supporting artifacts or references.
- `links`: optional specs, ADRs, issues, or PRs.
- `created`: when the entry was introduced.
- `review_after` or `expires`: lifecycle pressure.
- `[allow.selector]`: structural selector.
- `[allow.last_seen]`: review hint only.

`owner = "unowned"` is reserved for generated `classification = "baseline_debt"`
entries. Reviewed retained exceptions must use a concrete owner so accountability
does not disappear after adoption.
Token-like entry metadata such as `owner`, `classification`, and `family` must
not include leading or trailing whitespace. Those values are policy identifiers,
not display strings, and exact matching keeps baseline-debt, routing, and diff
signals deterministic.

`path` scopes are exact source-tree paths. Source-tree glob syntax is
intentionally small and build-independent; canonical policy globs support `*`,
`?`, and whole-segment `**`. Bracket classes such as `[ab]` and brace
alternation such as `{a,b}` are not supported; use exact `path` entries or
explicit separate globs instead. A `**` token is recursive only as its own path
segment; patterns such as `scripts/**.sh` are rejected instead of being treated
as a different wildcard shape. Diff mode reports non-directional source-tree
scope retargets as review-required `scope_changed` policy changes. That covers
exact path changes or sibling glob replacements that are neither broadening nor
narrowing.

Diff mode reports owner, reason, or classification removals as policy
weakening, additions of those required metadata fields as policy improvements,
and non-empty replacements as review-required policy changes. Changing a
concrete owner to `owner = "unowned"` is also policy weakening, because retained
exceptions must not silently lose ownership. Removed traceability links are also
review-required, while added links are reported as policy improvements.
`created` date removal fails, changes require review, and adding a missing
`created` date is reported as an improvement so exception provenance cannot
drift silently.

Diff mode reports top-level policy `owner` changes with the synthetic
`policy.owner` ID. Removing a concrete ledger owner or changing it to `unowned`
is policy weakening, adding a concrete owner is a policy improvement, and
changing one concrete owner to another requires review.
The top-level policy `owner` value must not include leading or trailing
whitespace; it is a ledger-routing identifier.

Diff mode reports top-level policy `status` changes with the synthetic
`policy.status` ID. Changing `active` to `advisory` or removing an active status
is policy weakening, changing `advisory` or an unset status to `active` is a
policy improvement, and other status transitions require review.

Diff mode also compares policy-level `[requirements]` booleans. Loosening a
requirement is policy weakening, tightening one is a policy improvement, and the
reported policy-change `allow_id` uses a synthetic stable path such as
`requirements.owner_required` because the change belongs to the ledger contract
rather than one allow entry.

Diff mode reports source-tree inventory carveout changes. Adding a
`workspace.ignored` scope is policy weakening because it can hide findings from
the scan; removing one is an improvement. Adding a `workspace.generated` scope
requires review because it can reclassify non-Rust inventory as generated code;
removing one is an improvement. These rows use synthetic stable `allow_id`
values such as `workspace.ignored` and `workspace.generated`.

## Reason And Evidence

`reason` is the human rationale.

`evidence` is the support for that rationale.
Evidence and link list entries must not include leading or trailing whitespace.
They are saved as exact ledger references, even when the current implementation
classifies some prefixes as traceability-only instead of resolving them.

Example:

```toml
reason = "Parser validates the range before slicing."
evidence = [
  "test:parser_rejects_invalid_text_range",
  "ripr:target/ripr/reports/parser-span-gap.json",
  "spec:docs/specs/parser-span-invariants.md",
  "legacy-policy:no-panic-baseline",
]
```

The presence of evidence is not proof that the exception is correct. It is a
traceable claim that reviewers and tools can inspect. Diff mode reports removed
evidence as policy weakening, typed evidence additions as policy improvements,
and weak additions such as unstructured or unknown-prefix evidence as
review-required posture changes.

General evidence can be required by setting `requirements.evidence_required =
true`. It is opt-in so generated `baseline_debt` ledgers can remain adoption
scaffolding until reviewed. Unsafe entries keep their separate
`requirements.unsafe.evidence_required` guard.

When general evidence is not required, matched non-baseline entries with no
evidence references still remain visible as policy-health debt. JSON reports
may include `summary.policy_missing_evidence` and
`trend.policy_missing_evidence`, and check receipts may include
`counts.policy_missing_evidence`. This is distinct from outcome-level
`evidence_missing`, which is used when evidence requirements are enforced. Use
`cargo-allow worklist --missing-evidence --format json` to route those retained
entries for evidence cleanup without pretending the current no-new check
failed.

Known local evidence prefixes are parsed when a policy is loaded from a source
tree. `doc:`, `spec:`, `adr:`, `ripr:`, `unsafe-review:`, and `coverage:`
references must point to source-tree-relative regular files that exist.
Symlinked evidence paths, including symlinked parent directories, are rejected
so the local evidence check does not silently follow a reference outside the
scanned source tree. `test:`, `cargo:`, `issue:`, `pr:`, and `legacy-policy:`
references are treated as traceability strings in the current implementation
and are not executed or resolved over the network.
For compatibility with migrated policies, `unsafe_review:` is accepted as an
alias for `unsafe-review:`, and `legacy_policy:` is accepted as an alias for
`legacy-policy:`.

Evidence strings with no `prefix:value` shape or with an unknown prefix are
retained as weak traceability, but they are not recognized as typed evidence.
Recognized traceability prefixes with an empty value, such as `test:` or
`issue:`, are also treated as weak evidence because they do not identify an
artifact or review target.
Audit, check, diff, and receipt artifacts may report these under
`weak_evidence_references` so weak evidence quality is visible even when the
entry has some evidence and otherwise matches current source findings.

Illustrative local evidence references:

```toml
evidence = [
  "unsafe-review:docs/evidence/unsafe-review/ffi-read-buffer.json",
  "test:ffi_read_buffer_rejects_null_pointer",
  "doc:docs/safety/ffi-read-buffer.md",
]
```

```toml
evidence = [
  "ripr:docs/evidence/ripr/parser-span-gap.json",
  "test:parser_rejects_invalid_text_range",
  "spec:docs/specs/parser-span-invariants.md",
]
```

```toml
evidence = [
  "coverage:docs/evidence/coverage/parser-span.lcov.info",
  "cargo:cargo llvm-cov --package parser",
]
```

When these appear in a real policy, the local files must exist before
`cargo-allow check` runs. cargo-allow validates the reference shape and local
path existence only; it does not execute ripr, unsafe-review, coverage tools,
Cargo commands, tests, or repository code, and it does not interpret those
receipt formats as proof.

Broken local evidence links are handled differently by discovery commands and
gating commands:

| Command path | Broken local evidence behavior |
|---|---|
| `audit`, `diff`, `explain`, `list`, `worklist`, and `propose` | Emit source-tree artifacts and surface the broken-link count or diagnostics so repair work can be routed. |
| `prune --stale` dry-run | Emits a stale-cleanup preview even when the stale entry itself has broken evidence. |
| `check` | Fails closed on broken local evidence links. |
| `doctor` | Reports invalid policy state for broken local evidence links. |
| `add` | Validates local evidence references before writing a reviewed policy entry. |
| `prune --stale --write` | Revalidates the remaining policy before writing; stale broken-evidence entries can be removed, but broken references that remain still block the write. |

This split is intentional. Read-only reporting and adoption commands should
help users find evidence repair work, while CI gates and reviewed policy writes
must not normalize broken evidence.

## Selector Precision

Selectors should be as narrow as practical. Strong selectors include:

- exact path.
- kind and family.
- AST kind.
- container.
- callee, macro name, or lint.
- symbol or target fingerprint.
- normalized snippet hash.

Selector identity values are exact source-syntax identifiers or fingerprints.
They must not include leading or trailing whitespace, because cargo-allow does
not trim them during structural matching.

Weak selectors include:

- broad globs.
- kind-only matching.
- missing container.
- missing callee, macro name, or lint where applicable.
- line-only matching.

For source-code exception kinds (`panic`, `unsafe`, and `lint_exception`),
`path`, `glob`, `line_hint`, and `[allow.last_seen]` are scope and review hints,
not structural identity. Those entries must include at least one structural
selector field such as `ast_kind`, `container`, `callee`, `macro_name`, `lint`,
`symbol`, a fingerprint, or `normalized_snippet_hash`. File-policy entries such
as `non_rust_file` and `generated_code` may remain scope-centric when the
source-tree file itself is the governed surface.
Line and column hints are one-based source positions for review. When a scanner
can compute a column from source text, that column should be a character
position in the source line rather than a byte offset.

Diff mode reports precision loss as policy weakening and precision increases as
policy improvements. The current precision score rewards exact paths and
structural selector fields such as AST kind, container, callee, macro name,
lint, symbol/fingerprints, snippet hash, and occurrence limits. It deliberately
does not reward line hints, because line and column are review hints rather than
identity. Equal-precision structural selector retargets, such as changing
`container`, `callee`, `macro_name`, `lint`, `symbol`, fingerprints, or snippet
hash values without changing the precision score, are reported as
review-required `selector_changed` policy changes. Line and column hint changes
do not trigger this signal.

Diff mode also reports scope broadening as policy weakening and scope narrowing
as a policy improvement when an allow entry moves from a broader glob to a
narrower glob or exact path.

## Lifecycle

Lifecycle fields prevent exceptions from becoming invisible permanent debt.

Use `review_after` when the exception may remain valid but needs periodic
review.

Use `expires` when the exception should fail after a known date unless it is
removed or re-approved.

Do not auto-extend expiry. Extending expiry is a policy decision and should be
visible in review.

The validator rejects invalid calendar dates, lifecycle dates that move backward
from `created`, `review_after` dates later than `expires`, empty or
parent-directory scopes, source-code selectors that contain no structural
identity beyond path/glob scope and line hints, and non-source selectors that
contain no structural identity or selector glob.

Diff mode reports expiry or review-date extensions/removals as review-required
lifecycle changes, and added or earlier lifecycle dates as policy improvements.

## Baseline Debt

`cargo-allow propose` may generate temporary adoption entries. Those entries
should use:

```toml
owner = "unowned"
classification = "baseline_debt"
reason = "Generated by cargo-allow propose; requires human review."
```

Baseline debt is allowed to make adoption possible. It must not be treated as a
clean final state.

Baseline debt must carry a short expiry. In the current validator, that means an
`expires` date no more than 120 days after `created` or, when `created` is
absent, the tool's deterministic fixture date.

`cargo-allow propose --write <path>` refuses to overwrite an existing file unless
`--force` is passed. The command writes only TOML to stdout or the requested
file, and emits its proposal summary to stderr so generated policy remains
parseable. `--summary-format json --summary-output <path>` writes that summary
as `cargo-allow.propose.v1`, including source-tree inventory context, proposal
options, proposed `baseline_debt` count, generated unsafe-baseline count, and
the generated-entry defaults that must remain visibly temporary until reviewed.
The unsafe count is a routing signal: generated unsafe baseline entries still
need real evidence and human review before they should be treated as retained
exceptions.

`cargo-allow add --kind <kind> --path <path> --line <line>` generates a reviewed
allow entry from the nearest current finding at that location. It copies the
finding's structural selector fields, sets owner/reason/classification and
lifecycle metadata from CLI flags, fails closed on ambiguous nearest findings,
and refuses to overwrite an output policy without `--force`. `--summary-format
json --summary-output <path>` writes the add summary as `cargo-allow.add.v1`,
including source-tree inventory context, selected finding details, generated
allow-entry metadata, and the human-review-required boundary for the proposed
receipt.

Counted legacy baselines should also carry an `occurrence_limit`:

```toml
occurrence_limit = 3
```

The limit preserves no-new semantics during migration. Matching the same
structural selector more times than the baseline allowed becomes new debt
instead of silently broadening the exception.

Diff mode reports occurrence-limit increases or removals as policy weakening
and occurrence-limit additions or reductions as policy improvements. It also
reports an existing `baseline_debt` entry being reclassified as reviewed policy,
or a reviewed entry being reclassified as `baseline_debt`, as policy weakening,
because generated adoption debt must not be normalized or introduced without an
explicit retained-exception review.

`cargo-allow explain <id>` reports this live posture for a single entry. It
shows the policy metadata, selector, current match status, matched finding
count, outcome counts, selector precision, broad-scope status, stale state,
occurrence-limit overruns, and evidence reference diagnostics. When the entry
needs attention, it also includes
suggested next actions and proof commands. Matched `baseline_debt` entries also
show next actions because generated debt still needs human review. Local
evidence references are shown as present, missing, or invalid; traceability
strings are identified as not executed or resolved. Current findings include
scanner-provided `source_package` context when available; that field is not
Cargo metadata or build-membership proof. It is derived only from readable
source-tree `Cargo.toml` text with a visible `[package].name`; workspace-only,
invalid, unreadable, or non-UTF8 manifests simply provide no package context.
The command is still bounded by the normal cargo-allow claim boundary: source
syntax only, with no macro expansion, macro token-tree expression parsing, type
analysis, build output, control-flow analysis, or data-flow analysis.
`--format json` emits the same single-entry explanation as
`cargo-allow.explain.v1`, including source-tree inventory context, scanner
limitations, evidence diagnostics, current findings, match outcomes, and the
same suggested actions/proof commands shown in the human view.

`cargo-allow list` shows allow entries with current status, match count, kind,
family, owner, classification, scope, scanner-provided source package context,
evidence-reference count, broken local evidence-reference count, weak
evidence-reference count, selector precision, broad-scope status, lifecycle
dates, and reason. It supports maintenance
filters such as `--kind`, `--family`, `--owner`, `--classification`, `--path`,
`--source-package`, `--allow-id`, `--status`, `--expired`, `--review-due`, `--stale`,
`--baseline-debt`, and `--missing-evidence`. Path filtering uses normalized
source-tree paths and includes broad glob scopes that cover the selected path.
Stale status is computed from current source-syntax findings; line and column
hints are not identity. `--broad-scope` lists entries whose source-tree scope
uses wildcard syntax, which is useful for reviewing intentionally wide policy
receipts before they become normalized debt. `--format json` emits the same
filtered ledger rows as `cargo-allow.list.v1` with source-tree inventory
context, scanner limitations, and applied filters so saved artifacts do not
require parsing the human table.

`cargo-allow prune --stale` previews stale allow entries that no current
source-syntax finding matched. Dry-run is the default; `--dry-run` makes that
choice explicit. `cargo-allow prune --stale --write` removes only those stale
entries from the selected policy file after revalidating the rendered policy.
`--format json` emits the stale cleanup preview or write result as
`cargo-allow.prune.v1`, including source-tree inventory context, scanner
limitations, mode flags, written path when write mode changed the policy, and
the stale entries selected for removal.

`cargo-allow doctor` validates local setup without executing repository code.
It reports source-tree root discovery, whether a policy config was found,
the loaded policy schema version, policy name, top-level owner, and policy
status when available, whether that policy parses and passes local validation,
any validation diagnostic, and the source-tree inventory source and file count.
Local policy validation includes locally referenced evidence file existence, but
it still does not execute external evidence tools or repository code.
`--format json` emits the same setup diagnostics as `cargo-allow.doctor.v1` so
CI or agent runners can verify which source tree, policy contract, policy owner,
policy state, and inventory mode a command would use before running wider policy
checks.

`cargo-allow worklist --format json` turns non-matched no-new outcomes and
matched `baseline_debt` entries into agent-safe work items. Each item includes a
kind, risk, difficulty, current status, governed exception kind, family where
available, path or allow ID, suggested actions, and proof commands. The worklist
summary rolls up both risk and difficulty counts, and the artifact records the
source-tree inventory source, root, `files_scanned` count, and explicit scanner
limitations when available. The worklist schema enumerates the supported scanner
limitation values so consumers can distinguish source-tree boundaries from
arbitrary annotations. `source_package` fields are scanner-provided context
only; they are not Cargo metadata or build-membership proof. Missing package
context means the scanner did not find usable source-tree manifest text for that
file, not that the file lacks Cargo package membership.
Worklist output can be filtered by governed kind, scanner family, policy owner,
policy classification, work item queue kind, match status, source-tree path,
baseline debt, broad source-tree scopes, missing evidence, risk, and
difficulty; filtered artifacts record all applied filters.
Policy-backed slices can also be filtered by durable allow ID with `--allow-id`,
and scanner-provided source-tree package context can be filtered with
`--source-package`.

The human worklist output includes the same first-step suggested actions and
proof commands so a maintainer can triage the queue without switching to JSON.
For broken or weak evidence work items, it also shows the exact evidence
reference, status, prefix, target, and diagnostic message.
Generated proof commands include `explain`, `list --allow-id`, and the durable
allow-ID worklist queue for policy-backed items; broad-scope and baseline-debt
advisory items also point back to the matching shortcut queues.
When the human view is truncated, it says how many work items were omitted and
points to JSON for the full queue. Filtered worklist output records the applied
filters so saved artifacts are not mistaken for the full ledger queue. The
doctor, report, receipt, explain, list, prune, propose, and worklist schemas
enumerate scanner limitation values rather than treating claim-boundary facts
as open-ended prose.
Work items are ordered for routing: high risk first, then lower estimated
difficulty, then stable source and allow identifiers.
`work-*` IDs are artifact-local queue handles assigned after filtering and
sorting; use `allow_id` for durable policy references.
Policy-backed work items include owner, classification, and reason so the queue
can route work without requiring an immediate `explain` lookup.
They also include lifecycle dates and an evidence-reference count when tied to a
policy entry, making expiry pressure and evidence gaps visible in the queue.
It is a routing surface, not an auto-fix plan: agents and humans should fix,
prove, narrow, or remove the exception instead of adding suppressions just to
silence cargo-allow.

The worklist command also reports evidence-health cleanup as work items. Broken
local evidence links become `broken_evidence_link` items. Unstructured evidence
strings or references with unknown prefixes become `weak_evidence_reference`
items. This mode loads the policy even when local evidence is missing so humans
or agents can get a repair target; normal `cargo-allow check` still fails
closed on broken local evidence references.

The worklist may also include advisory `broad_scope` items for matched allow
entries that use wildcard source-tree scopes. These do not mean the current
policy failed; they route cleanup work toward narrower selectors or explicit
review of the broad scope.

`cargo-allow diff --base <rev>` compares the current policy ledger with the
base revision's `policy/allow.toml` and reports policy weakening and
review-required policy changes in human and Markdown output. Current detection
covers scope broadening, selector precision loss, expiry/review extension,
evidence removal, top-level policy status weakening, top-level policy owner
removal/unassignment, owner/reason/classification removal, owner unassignment,
occurrence-limit loosening, added `baseline_debt`, reviewed entries reclassified
as `baseline_debt`, existing `baseline_debt` entries reclassified as reviewed
policy, and policy requirement loosening. It also reports source-tree inventory
carveout changes for `workspace.ignored` and `workspace.generated`. This is
policy ledger comparison only.

The same command also compares source finding posture between the base git tree
and the current checkout, or the optional `--head` git tree when provided. It
uses source-syntax finding keys built from kind, family, path, AST kind,
language, module/container/callee/macro/lint fields, symbols, fingerprints, and
normalized snippet hash. Line and column remain hints, not identity: moving a
finding without changing its structural source surface should not appear as a
new exception. The diff reports new and removed syntax-visible exception
findings, including count changes for repeated matching finding keys; it still
does not claim macro expansion, macro token-tree expression parsing, type
information, build awareness, proof adequacy, control-flow analysis, or
data-flow analysis.

Saved `check` receipts may include a `source_inventory` object with the same
kind and kind.family breakdown as JSON reports. This keeps the durable CI
receipt useful for both gate status and source-exception inventory without
requiring consumers to archive or re-aggregate the full finding list.

Markdown diff output starts with a PR summary. The summary reports net posture,
reviewer action, current check failures, new and removed source findings,
policy failures, policy review items, policy improvements, and non-zero
evidence-health counts such as broken local evidence links or weak evidence
references. `worse` means the diff has a failing no-new, broken local evidence,
or policy-weakening signal. `review-required` means posture changed without a
failing signal, such as a receipted new source finding or a new allow entry.
`improved` means source findings or allow entries were removed without new
failures or review-required policy changes.

JSON diff output includes the same posture signals under the optional `diff`
object. That object contains `net_posture`, a summary of current failures and
posture-change counts, `finding_changes`, and `policy_changes`, so automated PR
consumers do not need to parse human or Markdown text.

## Non-Rust Files

Source trees often contain non-Rust operational surface:

- CI workflows.
- shell scripts.
- Python or JavaScript tools.
- release helpers.
- documentation.
- generated files.
- configuration.

cargo-allow treats those files as governed source surface when they are in the
scanned inventory. Each retained non-Rust file should have an owner, reason,
classification, selector, and lifecycle.

By default, repository inventory is based on `git ls-files`. Use
`--include-untracked` for local discovery runs that should include untracked
files. The policy section currently named `[workspace]` contains source-tree
inventory settings; it is not a Cargo workspace requirement. `workspace.ignored`
removes matching inventory paths before scanning, and `workspace.generated`
marks matching non-Rust file findings as `generated_code`.
The `workspace.inventory` policy value is canonically `git-tracked`; the parser
also accepts the artifact-style alias `git_tracked`. This field does not disable
the normal source-tree fallback behavior when git-tracked inventory is
unavailable.
`workspace.default_mode` is the default gate mode used by `cargo-allow check`
when `--mode` is omitted. Passing `--mode audit`, `--mode no-new`,
`--mode strict`, or `--mode release` still overrides the policy default for that
invocation.
`workspace.inventory` and `workspace.default_mode` are exact policy tokens and
must not include leading or trailing whitespace.

The current non-Rust family vocabulary is intentionally explicit:
`ci_declarative`, `documentation`, `release_script`, `test_fixture`,
`generated_code`, `editor_extension`, `package_metadata`, `shell_script`,
`python_tool`, `javascript_tool`, `configuration`, and `unknown_non_rust`.

Human and Markdown audit reports summarize the non-Rust file inventory by
status and family, then list the scanned file paths. This is a review surface,
not a separate schema contract; the stable machine-readable report remains the
versioned JSON output. Audit summaries also count policy-level
`baseline_debt` entries so generated adoption debt remains visible even when the
underlying findings currently match and `check --mode no-new` passes.
Audit reports can also count broken local evidence links. `audit` treats these
as evidence-health findings so first-run inventory can finish and route cleanup
to `cargo-allow worklist --item-kind broken_evidence_link`; `check` still fails
closed on broken local evidence references.
Audit, check, and diff artifacts can also report policy-level missing evidence
when retained non-baseline allow entries have no evidence references but
otherwise match the current source tree. That advisory count routes to
`cargo-allow worklist --missing-evidence` and does not, by itself, claim the
source-tree check failed.
