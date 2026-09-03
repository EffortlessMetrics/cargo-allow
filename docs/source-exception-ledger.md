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

## Configuration Discovery

Cargo-allow has three related configuration surfaces. They are intentionally
separate today; profile resolution and federation do not share one unified
resolver yet. This distinction matters when a repository contains more than
one `.allow/` or `policy/` file.

### Source-exception policy

Commands that operate on the source-exception ledger (`check`, `audit`, `list`,
`diff`, and related commands) use this order:

1. An explicit `--config` path.
2. A valid `.allow/config.toml` federation registry that names a canonical
   ledger for the `source-exception` lane.
3. Source-text discovery from the requested source-tree root upward. At each
   directory, cargo-allow checks a local `Cargo.toml` first, then the
   conventional paths below.

For `Cargo.toml` source text, `[package.metadata.cargo-allow].config` takes
precedence over `[workspace.metadata.cargo-allow].config` in the same manifest.
The selected metadata value must be a non-empty, existing relative path without
`..`; an unusable metadata candidate falls through to conventional discovery.
Cargo-allow reads the manifest text directly. It does not invoke the Cargo
`metadata` command or infer workspace membership.

> Command maturity is maintained in the [Support Tiers command maturity table](status/SUPPORT_TIERS.md#command-maturity).
> This reference describes current ledger behavior; it does not promote the
> source candidate to the published channel.

The conventional source-exception order is:

```text
policy/cargo-allow.toml
policy/allow.toml
.cargo/allow.toml
allow.toml
```

The native `policy/cargo-allow.toml` path may omit the `policy = "cargo-allow"`
header. A foreign dialect at another conventional path is skipped with a
diagnostic instead of being parsed or merged.

`cargo-allow doctor` exposes this source-exception resolution provenance in the
`config.provenance` JSON object and the matching human line. `source` is
`cli_override` for an explicit `--config`, `federation_registry` for a valid
canonical source-exception ledger, `package_metadata` or `workspace_metadata`
for Cargo manifest metadata, and `conventional_path` for the ordered paths
listed above. `precedence` is `cli_override`, `federation_registry`, or
`discovery_fallback`. If the resolver had to fall back after an evaluation
error, the selected discovery source remains visible while precedence is
omitted. Provenance describes how the path was selected; it does not validate
the policy contents or imply Cargo workspace metadata was executed.

`ResolvedCargoAllowConfigV1` is the versioned read-only component for making
that current selection auditable across future consumers. Its adapter retains
the selected portable policy identity and digest, candidate and
skip provenance, the higher-order error that caused a legacy fallback,
federation/profile participation, completeness, limitations, and the exact
opaque source subject supplied by the caller. Source subjects use portable
ASCII identity characters (`A-Z`, `a-z`, `0-9`, `.`, `_`, `:`, `@`, `+`, and
`-`) so they cannot carry checkout-local paths. Configuration paths carry a
`resolved_repository_root` or `discovery_ancestor` anchor, an ancestor depth,
and a safe relative path. This preserves intentional upward discovery without
serializing `..` or checkout-local absolute paths; existing symlink targets are
contained under their authorized anchor before policy bytes are read. The
portable projection uses `.` for the resolved repository root and preserves an
in-repository requested-root identity when the caller supplies one. The typed
optional root-relationship field disambiguates same, descendant, external, and
unknown relationships; its absence denotes a legacy producer. The legacy
adapter entry point passes the same root for both values and therefore
continues to emit `.` for each. Unrepresentable relationships are reported as
`unknown` or `external` rather than being mistaken for the repository root.
When a Cargo manifest cannot be read or parsed far enough to distinguish
package from workspace metadata, the skipped attempt uses the honest generic
`cargo_metadata` source rather than being mislabeled as legacy discovery.
The initial adapter intentionally reports partial completeness
because current discovery stops after its winner and still performs multiple
reads. The adapter now preserves an explicitly supplied in-repository
requested-root identity while keeping the resolved repository root at `.`;
unknown and external relationships remain explicit and portable. Candidate
completeness and current-behaviour characterization remain part of #3875;
command cutover remains #3876.

### Spec-system profile

`--profile spec-system` has its own resolver. Unless an explicit profile config
is supplied, it checks these paths in order:

```text
.allow/profiles/spec-system.toml
.allow/config.toml
policy/spec-system.toml
```

If none exists, the command uses built-in advisory roots and reports that the
profile config was not found. When an owned `.allow/` profile and the legacy
`policy/spec-system.toml` both exist, the owned path wins and the conflict is
reported for cleanup.

### Federation registry

Federation currently reads the fixed `.allow/config.toml` path. When that file
parses and validates, its canonical ledger registered for the
`source-exception` lane takes precedence over source-text policy discovery.
Doctor reports identify this source vocabulary as `fixed_allow_config` in both
human and JSON output when the fixed path is present.
The same path is also a profile fallback, but the profile and federation
parsers remain separate consumers with separate contracts; cargo-allow does not
silently merge their fields.

This is the current compatibility boundary for issue [#2828](https://github.com/EffortlessMetrics/cargo-allow/issues/2828).
The Cargo manifest metadata path is implemented, while unifying profile and
federation resolution remains planned work and should not be inferred from the
presence of shared `.allow/` paths.

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
- `[allow.last_seen]`: review hint only. It is one entry-level observation;
  for entries covering multiple findings, an anchored occurrence can suppress
  sibling location-drift advisories to avoid refresh oscillation. It does not
  establish exact per-occurrence movement; see [#2508](https://github.com/EffortlessMetrics/cargo-allow/issues/2508).

`owner = "unowned"` is reserved for generated `classification = "baseline_debt"`
entries. Reviewed retained exceptions must use a concrete owner so accountability
does not disappear after adoption.
Token-like entry metadata such as `owner`, `classification`, and `family` must
not include leading or trailing whitespace. Those values are policy identifiers,
not display strings, and exact matching keeps baseline-debt, routing, and diff
signals deterministic.
`evidence` and `links` entries must also be non-empty, whitespace-normal, and
unique inside an allow entry so receipts do not inflate proof or traceability
signals by repeating the same reference.
Local-file traceability links such as `doc:`, `spec:`, `adr:`, `ripr:`,
`unsafe-review:`, `unsafe_review:`, and `coverage:` must use source-tree-relative
exact paths. Policy loading rejects parent-directory segments, absolute paths,
and wildcard tokens in those link targets; existence checks remain a separate
source-tree inventory diagnostic.

`path` scopes are exact source-tree paths. Source-tree scopes must not include
leading or trailing whitespace because cargo-allow treats them as selectors,
not display strings. Source-tree glob syntax is intentionally small and
build-independent; canonical policy globs support `*`, `?`, and whole-segment
`**`. Bracket classes such as `[ab]` and brace alternation such as `{a,b}` are
not supported; use exact `path` entries or explicit separate globs instead. A
`**` token is recursive only as its own path segment; patterns such as
`scripts/**.sh` are rejected instead of being treated as a different wildcard
shape. Repository-wide globs such as `**`, `**/*`, and equivalent whole-tree
wildcard shapes are rejected because a retained exception must not silently
cover the entire source tree. Diff mode reports non-directional source-tree
scope retargets as review-required `scope_changed` policy changes. That covers
exact path changes or sibling glob replacements that are neither broadening nor
narrowing.

Diff mode reports owner, reason, or classification removals as policy
weakening, additions of those required metadata fields as policy improvements,
and non-empty replacements as review-required policy changes. Changing a
concrete owner to `owner = "unowned"` is also policy weakening, because retained
exceptions must not silently lose ownership. Removed traceability links are also
review-required unless they are local-file traceability links, which fail the
diff because source-tree rationale should not disappear silently. Removing only
weak traceability links is reported as an improvement when typed traceability
remains, and as review-required when no typed traceability remains. Typed
traceability link additions are reported as policy improvements, while
unstructured or unknown-prefix link additions are review-required so vague
traceability does not look like proof-quality cleanup.
Local-file traceability link additions with invalid source-tree paths fail the
diff because retained exception links must not point outside the repository
surface.
Local-file traceability link additions that are absent from the compared
source-tree inventory also fail, so PR summaries do not treat untracked or
missing local rationale as proof-quality cleanup.
Diff evidence-health counts also include retained local-file traceability links
whose targets are no longer present in the compared source-tree inventory, so
removing a linked rationale file without updating policy is not treated as a
neutral source-tree change.
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

> **Note:** `status` is an informational label tracked by `diff` for posture
> review. It does **not** affect `check --mode no-new` enforcement — a policy
> marked `status = "advisory"` still fails the no-new gate on new findings.

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
It must not include leading or trailing whitespace; cargo-allow keeps rationale
text exact so rendered policies, reviews, and policy diffs do not hide padded
receipt text.

`evidence` is the support for that rationale.
Evidence and link list entries must not include leading or trailing whitespace.
They are saved as exact ledger references, even when the current implementation
classifies some prefixes as traceability-only instead of resolving them.
Reviewed unsafe entries and reviewed high-risk process/network policy
exceptions must include at least one typed evidence reference using a recognized
non-empty `prefix:value` shape. The current high-risk policy exception families
are `policy_exception.process_spawn` and
`policy_exception.network_destination`. Generated `baseline_debt` entries for
these surfaces may retain uncomfortable placeholder evidence until a human
replaces the baseline with a reviewed receipt.

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
typed evidence as policy weakening, names local-file evidence removal
explicitly, reports weak-evidence cleanup as an improvement when typed evidence
remains, and treats weak-evidence removal without remaining typed evidence as
review-required. It reports typed evidence additions as policy improvements,
statically invalid or missing local-file evidence additions as policy weakening,
and weak additions such as unstructured or unknown-prefix evidence as
review-required posture changes.

General evidence can be required by setting `requirements.evidence_required =
true`. It is opt-in so generated `baseline_debt` ledgers can remain adoption
scaffolding until reviewed. Unsafe entries and high-risk process/network policy
exceptions keep their separate typed-evidence guard for reviewed receipts.

Repositories that require a locally checked evidence file for every unsafe
receipt can opt into the stricter nested requirement:

```toml
[requirements.unsafe]
verified_evidence_required = true
```

With that setting, at least one `doc:`, `spec:`, `adr:`, `ripr:`,
`unsafe-review:`, or `coverage:` reference is required. Traceability-only
references remain valid supplementary context, but cannot satisfy the stricter
unsafe mandate by themselves. Omitting the setting preserves the default
`false` behavior for existing policies. Diff mode treats disabling this
requirement as policy weakening and enabling it as an improvement.

When general evidence is not required, matched non-baseline entries with no
evidence references still remain visible as policy-health debt. JSON reports
may include `summary.policy_missing_evidence` and
`trend.policy_missing_evidence`, and check receipts may include
`counts.policy_missing_evidence`. This is distinct from outcome-level
`evidence_missing`, which is used when evidence requirements are enforced. The
default `cargo-allow worklist --format json` includes those retained entries as
evidence-cleanup work without pretending the current no-new check failed. Use
`cargo-allow worklist --missing-evidence --format json` to focus only that
queue. `cargo-allow explain <id>` also points matched non-baseline entries with
empty evidence toward the same evidence-cleanup queue.

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
`weak_evidence_references` so weak evidence/link quality is visible even when
the entry has some evidence or links and otherwise matches current source
findings.

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

Broken local evidence links and local-file traceability links are handled
differently by discovery commands and gating commands:

| Command path | Broken local reference behavior |
|---|---|
| `audit`, `diff`, `explain`, `why`, `list`, `worklist`, and `propose` | Emit source-tree artifacts and surface the broken-link count or diagnostics so repair work can be routed. |
| `prune --stale` dry-run | Emits a stale-cleanup preview even when the stale entry itself has broken evidence. |
| `check` | Fails closed on broken local evidence links and broken local-file traceability links. |
| `doctor` | Reports invalid policy state for broken local evidence links and broken local-file traceability links. |
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
`--force` is passed. A custom `--expires <date>` must be a `YYYY-MM-DD` date
within the same 120-day temporary-baseline window enforced by policy
validation. The command writes only TOML to stdout or the requested file, and
emits its proposal summary to stderr so generated policy remains parseable.
`--summary-format json --summary-output <path>` writes that summary as
`cargo-allow.propose.v1`, including source-tree inventory context, proposal
options, proposed `baseline_debt` count, generated unsafe-baseline count, and
the generated-entry defaults that must remain visibly temporary until reviewed.
The unsafe count is a routing signal: generated unsafe baseline entries still
need real evidence and human review before they should be treated as retained
exceptions.

`cargo-allow add --kind <kind> --path <path> --line <line>` generates a reviewed
allow entry from the nearest current finding at that location. It copies the
finding's structural selector fields, sets owner/reason/classification and
lifecycle metadata from CLI flags, fails closed on ambiguous nearest findings,
requires at least one typed `--evidence prefix:value` reference for reviewed
unsafe and high-risk process/network policy exceptions, and refuses to overwrite
an output policy without `--force`.
`--summary-format json --summary-output <path>` writes the add summary as
`cargo-allow.add.v1`, including source-tree inventory context, selected finding
details, generated allow-entry metadata, and the human-review-required boundary
for the proposed receipt. Unavailable policy metadata (`family`, `review_after`,
and `expires`) is omitted from the JSON allow-entry projection; selector
relationships such as `path` and `glob`, and matching-state fields such as
`last_seen`, remain nullable when the policy shape does not provide them.

Counted legacy baselines should also carry an `occurrence_limit`:

```toml
occurrence_limit = 3
```

The limit preserves no-new semantics during migration. Matching the same
structural selector more times than the baseline allowed becomes new debt
instead of silently broadening the exception. Policy load rejects
`occurrence_limit = 0` and values above the documented ceiling
(`OCCURRENCE_LIMIT_MAX = 10000`) so typos like `999999999` cannot disable the
cap.

Diff mode reports occurrence-limit increases or removals as policy weakening
and occurrence-limit additions or reductions as policy improvements. It also
reports an existing `baseline_debt` entry being reclassified as reviewed policy,
or a reviewed entry being reclassified as `baseline_debt`, as policy weakening,
because generated adoption debt must not be normalized or introduced without an
explicit retained-exception review.

`cargo-allow explain <id>` reports this live posture for a single entry. It
shows the policy metadata, selector, current match status, matched finding
count, outcome counts, selector precision, broad-scope status, stale state,
occurrence-limit overruns, evidence reference diagnostics, and local
traceability-link diagnostics. When the entry
needs attention, it also includes
suggested next actions and proof commands. Matched `baseline_debt` entries also
show next actions because generated debt still needs human review, and matched
entries with no evidence references show evidence-cleanup actions. Local
evidence references are shown as present, missing, or invalid; traceability
strings are identified as not executed or resolved. Local-file `links` entries
use the same source-tree diagnostics in `explain` so missing specs, ADRs, or
other linked rationale files are visible during review, and broken or weak links
route to traceability-repair next actions. Current findings include
scanner-provided `source_package` context when available; that field is not
Cargo metadata or build-membership proof. It is derived only from readable
source-tree `Cargo.toml` text with a visible `[package].name`; workspace-only,
invalid, unreadable, or non-UTF8 manifests simply provide no package context.
The command is still bounded by the normal cargo-allow claim boundary: source
syntax only, with no macro expansion, macro token-tree expression parsing, type
analysis, build output, control-flow analysis, or data-flow analysis.
`--format json` emits the same single-entry explanation as
`cargo-allow.explain.v1`, including source-tree inventory context, scanner
limitations, evidence/link diagnostics, current findings, match outcomes, and the
same suggested actions/proof commands shown in the human view.
Unavailable policy metadata (`family` and lifecycle dates) is omitted from the
allow-entry object; selector relationships and matching-state fields retain their
nullable representation.

`cargo-allow why --kind <kind> --path <path> --line <line>` is the inverse of
`explain`: given a finding location, it shows why that finding is unreceipted
(or already receipted) and lists nearby same-kind allow entries with per-gate
selector mismatch reasons. Use it when a CI failure names a path and line and
you need operator clarity before `add` or `explain`.
`--format json` emits the same explanation as `cargo-allow.why.v1`.

Lint suppression scanning includes direct `#[allow(...)]`, `#![allow(...)]`,
`#[expect(...)]`, and `#![expect(...)]` attributes, plus source-visible
`cfg_attr(..., allow(...))` and `cfg_attr(..., expect(...))` conditionals.
Conditional attributes are treated as repository text: cargo-allow records that
the suppression surface exists, but it does not evaluate cfg predicates,
compile the crate, or ask Clippy which condition is active.

Unsafe scanning includes direct unsafe source constructs such as unsafe blocks,
unsafe functions, unsafe impls, unsafe traits, unsafe extern blocks, direct
`#[unsafe(...)]` / `#![unsafe(...)]` attributes, and source-visible
`cfg_attr(..., unsafe(...))` conditionals. Conditional unsafe attributes are
also treated as repository text; cargo-allow does not evaluate whether the cfg
condition is active.

`cargo-allow list` defaults to a concise human card for each allow entry with
bounded ID, status, kind, scope, owner, match/evidence summary, and reason
lines. Cards avoid an unbounded horizontal TSV row while retaining a
deterministic count/status/evidence summary. The view distinguishes an empty
ledger, an empty filtered view, and an empty tracked-source inventory. Long
repository values are ellipsized in this concise view; `--wide` or explicit
`--columns` retains the complete human projection. JSON remains complete and
unchanged. With `--color always` (or supported terminal auto-detection), fixed
status markers in human cards and wide/explicit status columns are styled;
`--width <columns>` can explicitly tighten concise card lines for a narrow
terminal. On an interactive human terminal, the CLI uses the operating
system-reported width when it is available; redirected and captured output
keeps the deterministic non-TTY layout. If terminal sizing is unavailable or
too narrow for the safe minimum, the stable default layout is used.
the `explain` human status projection follows the same palette. JSON and
`--output` files remain ANSI-free. Worklist human risk/status labels follow the
same palette. Diff human posture, movement, and policy/finding status labels
follow the same palette; repository-controlled paths, IDs, messages, and
changed-file lists remain plain. Other human commands remain plain until their
renderers are migrated. The `why` human status line follows the same palette;
its finding details, candidate IDs, mismatch reasons, and proof commands remain
plain. The `doctor` human config and federation status labels follow the same
palette; roots, paths, diagnostics, ledger metadata, and inventory messages
remain plain.
The `propose` human summary styles its generated `baseline_debt` classification
marker; generated policy TOML, finding-derived values, and summary files remain
plain. The `refresh` human report styles its fixed `lifecycle: preserved` marker;
drift messages, paths, IDs, finding locations, and refresh artifacts remain
plain. The `prune --stale` human report styles its fixed `stale` marker;
policy rows, paths, TOML previews, JSON, and `--output` files remain plain.
The `add` human summary styles its fixed `human review` warning, and broad-add
summaries style their fixed `broad baseline` marker; generated policy TOML,
finding-derived values, JSON, and summary files remain plain.
The `migrate` human summary styles its fixed `ready` or `blocked` posture marker;
generated policy TOML, JSON, and summary files remain plain.
The full row includes current status, match count, kind, family, owner,
classification, scope, scanner-provided source package context,
evidence-reference count, broken local evidence-reference count, weak
evidence-reference count, selector precision, broad-scope status, lifecycle
dates, and reason. Broken and weak reference counts include typed local-file
`links` entries so list filters can find missing linked rationale as well as
missing evidence artifacts. It supports maintenance
filters such as `--kind`, `--family`, `--owner`, `--classification`, `--path`,
`--source-package`, `--allow-id`, `--status`, `--expired`, `--review-due`, `--stale`,
`--location-drift`, `--baseline-debt`, `--missing-evidence`, `--broken-evidence`, and
`--weak-evidence`. Status selectors `--status`, `--expired`, `--review-due`,
`--stale`, and `--location-drift` are mutually exclusive (pick one);
`--baseline-debt` is a classification filter and may still be combined with one
status selector. Governed kind
filters are validated up front, so a mistyped `--kind` fails closed instead of
producing a misleading empty list. Path filtering uses normalized
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
choice explicit. Human dry-run and write output includes a diff-style TOML
removal preview so reviewers can inspect the exact canonical `[[allow]]` blocks
before or after cleanup. `cargo-allow prune --stale --write` removes only those
stale entries from the selected policy file after revalidating the rendered policy.
`--format json` emits the stale cleanup preview or write result as
`cargo-allow.prune.v1`, including source-tree inventory context, scanner
limitations, mode flags, written path when write mode changed the policy, and
the stale entries selected for removal. Stale-entry rows omit unavailable policy
`family` metadata rather than emitting `null`; identity, ownership, scope, and
reason fields remain unchanged.

`cargo-allow doctor` validates local setup without executing repository code.
It reports source-tree root discovery, whether a policy config was found,
the loaded policy schema version, policy name, top-level owner, and policy
status when available, whether that policy parses and passes local validation,
any validation diagnostic, and the source-tree inventory source and file count.
Local policy validation includes locally referenced evidence and traceability
file existence, but it still does not execute external evidence tools or
repository code.
When a policy model can be loaded, doctor JSON also reports non-zero broken
local evidence-link and weak evidence-reference counts under `config`,
including typed local-file traceability links, so setup diagnostics can route
evidence repair before wider scans.
When no policy config is found, doctor JSON may include
`config.suggested_init_command` with the root-aware standalone initialization
command for the diagnosed source tree.
Absent optional `config` metadata is omitted rather than emitted as JSON null;
`config.found` remains the required discriminator, and `config.valid` and
`config.diagnostic` appear only when a policy was evaluated. Federation fields
retain their existing contract in this slice.
`--format json` emits the same setup diagnostics as `cargo-allow.doctor.v1` so
CI or agent runners can verify which source tree, policy contract, policy owner,
policy state, and inventory mode a command would use before running wider policy
checks.
When a valid policy declares `[[workspace.file_family]]` rules, doctor also
reports a `file_families` block: each configured rule includes its canonical
family, glob, and count of inventory files it classified, while `conflicts`
lists paths where equally specific rules remain ambiguous. These are
classification diagnostics only; they do not exclude files, approve findings,
or make claims about file contents.

For maintainer troubleshooting, `cargo-allow doctor --support-bundle <path>`
writes the separate `cargo-allow.support-bundle.v2` contract to a path inside
the source-tree root. The bundle is allowlisted and redacted: it records setup
metadata, repository-relative config identity, inventory counts, and federation
presence/validity, while excluding source contents, policy reasons and
evidence, environment variables, credentials, remotes, and unowned artifacts.
It performs no network upload and does not change the policy ledger. See the
[support-bundle schema](schemas/support-bundle.schema.json).

`cargo-allow worklist --format json` turns non-matched no-new outcomes and
matched `baseline_debt` entries into agent-safe work items. Each item includes a
kind, risk, difficulty, current status, governed exception kind, family where
available, path or allow ID, suggested actions, and proof commands. The worklist
summary rolls up risk counts, difficulty counts, and non-empty queue kind counts,
and the artifact records the source-tree inventory source, root,
`files_scanned` count, and explicit scanner limitations when available. The
worklist schema enumerates the supported scanner
limitation values so consumers can distinguish source-tree boundaries from
arbitrary annotations. `source_package` fields are scanner-provided context
only; they are not Cargo metadata or build-membership proof. Missing package
context means the scanner did not find usable source-tree manifest text for that
file, not that the file lacks Cargo package membership.
Worklist output can be filtered by governed kind, scanner family, policy owner,
policy classification, work item queue kind, match status, source-tree path,
baseline debt, broad source-tree scopes, missing evidence, risk, and
difficulty; filtered artifacts record all applied filters.
The default worklist includes matched policy entries with empty evidence, and
unsafe entries are queued as `unsafe_missing_evidence` so they keep the stronger
unsafe evidence actions while still appearing in the missing-evidence shortcut
queue. Use `--missing-evidence` when a run should include only policy-backed
items with no evidence references.
Governed kind filters are validated up front, so a mistyped `--kind` fails
closed instead of producing a misleading empty queue.
Canonical work item queue kinds use underscores, and `--item-kind` also accepts
hyphenated aliases such as `stale-allow` for artifact-local `work-*` queue IDs.
Worklist artifacts record the canonical underscore kind in the applied filter
context.
Policy-backed slices can also be filtered by durable allow ID with `--allow-id`,
and scanner-provided source-tree package context can be filtered with
`--source-package`.

The human worklist output includes the same first-step suggested actions and
proof commands so a maintainer can triage the queue without switching to JSON.
For broken or weak evidence/link work items, it also shows the exact evidence or
link reference, status, prefix, target, and diagnostic message.
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
difficulty, then queue kind, then lower selector precision when a policy entry
is tied to the item, then stable source and allow identifiers.
`work-*` IDs are artifact-local queue handles assigned after filtering and
sorting; use `allow_id` for durable policy references.
Policy-backed work items include owner, classification, and reason so the queue
can route work without requiring an immediate `explain` lookup.
They also include lifecycle dates and an evidence-reference count when tied to a
policy entry, making expiry pressure and evidence gaps visible in the queue.
When available, they also include selector precision so agents can distinguish
narrow retained receipts from broad policy entries that may need review or
narrowing. Lower scores mean fewer structural identity fields; higher scores
mean the selector is more specific, not that the exception is proven correct.
It is a routing surface, not an auto-fix plan: agents and humans should fix,
prove, narrow, or remove the exception instead of adding suppressions just to
silence cargo-allow.

The worklist command also reports evidence-health cleanup as work items. Broken
local evidence links and broken local-file traceability links become
`broken_evidence_link` items. Unstructured evidence strings, weak traceability
links, or references with unknown prefixes become `weak_evidence_reference`
items. This mode loads the policy even when local evidence or linked rationale
is missing so humans or agents can get a repair target; normal
`cargo-allow check` still fails closed on broken local evidence references and
broken local-file traceability links. Evidence repair risk follows the governed
exception kind and family, so missing evidence for unsafe or high-risk policy
exceptions is routed ahead of lower-risk repair work. Matched high-risk process
and network policy exceptions with no evidence also receive typed-evidence and
narrow-or-remove actions instead of generic evidence cleanup guidance.

The worklist may also include advisory `broad_scope` items for matched allow
entries that use wildcard source-tree scopes. These do not mean the current
policy failed; they route cleanup work toward narrower selectors or explicit
review of the broad scope. Their risk follows the governed exception kind, so a
broad unsafe selector is routed ahead of a broad low-risk documentation scope.

`cargo-allow diff --base <rev>` compares the selected policy ledger between
the base revision and the current checkout or explicit `--head` revision, then
reports policy weakening and review-required policy changes in human and
Markdown output. The default policy path is discovered from standard
source-tree locations, and `--config` can select a specific ledger path.
Current detection covers scope broadening, selector precision loss,
expiry/review extension, evidence removal with local-file removals named
explicitly, broken local evidence additions, local-file traceability link removal,
top-level policy status weakening, top-level policy owner removal/unassignment,
owner/reason/classification removal, owner unassignment, occurrence-limit
loosening, added `baseline_debt`, reviewed entries reclassified as
`baseline_debt`, existing `baseline_debt` entries reclassified as reviewed
policy, and policy requirement loosening. It also reports source-tree inventory
carveout changes for `workspace.ignored` and `workspace.generated`. This is
policy ledger comparison plus source-tree local evidence path validation; it
does not validate evidence content or execute evidence tools.

When `--kind <kind>` is supplied, diff output filters source finding posture
and allow-entry policy posture to that governed kind before comparing base and
head. Top-level policy contract changes, such as `[requirements]`,
`workspace.ignored`, `workspace.generated`, or policy status changes, remain
ledger-level posture signals because they are not owned by one allow entry.
Unknown governed kind names fail closed at CLI parsing time before cargo-allow
loads and scans the source-tree inventory.

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
JSON finding rows omit unavailable scanner metadata such as `family` and
`source_package`; navigation and identity-shape fields such as `line` and
`container` retain nullable values when those observations are unavailable.

When `--head <rev>` is supplied, diff treats the compared git trees as the
posture source. The rendered current report, evidence-health summary, policy
comparison, and source finding comparison come from the explicit head revision,
not from the working tree. If no `--config` is supplied, cargo-allow searches
the standard policy paths in the head revision first, then falls back to the
base revision. This keeps current posture tied to the head revision when a PR
moves the policy file, while still making base-only policy removals visible. If
a relative `--config` is supplied, that source-tree path must exist in the base
or head revision; missing explicit config paths fail closed instead of silently
comparing empty policies. Absolute config paths still use the existing
working-tree path validation. This remains source-tree git-object reading only;
cargo-allow does not execute repository code or ask Cargo to resolve project
metadata.

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

Repositories may declare custom file-presence families with repeated
`[[workspace.file_family]]` tables:

```toml
[[workspace.file_family]]
id = "model-artifact"
family = "ml_model"
glob = "models/**/*.onnx"
reason = "Govern versioned model artifacts."
```

The schema requires a stable lowercase rule ID, a lowercase canonical family
code, a source-tree-relative bounded glob, and a non-empty rationale. Built-in
family codes are reserved, duplicate definitions are rejected, and these rules
never ignore files or approve ledger entries. This configuration contract is
stored and rendered now; classifier application and reclassification movement
remain the follow-up seams in #2691 and #2692.

Optional `[lanes.<kind>]` tables declare per-kind enforcement posture without
splitting the ledger. Supported `mode` values are `advisory`, `shadow`, and
`blocking`. Unconfigured kinds default to `blocking`. Shadow and advisory lanes
report findings and receipt counts but do not fail `check` gate modes unless
`--deny` promotes a receipt advisory class. Blocking lanes follow the existing
`no-new`/`strict` failure rules. Receipts include `lane_posture` with the
effective mode per configured or scanned kind.

`workspace.ignored` and `workspace.generated` globs must be unique after slash
normalization so inventory and generated-code posture cannot be inflated by
repeating the same carveout.

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
the default `cargo-allow worklist` queue, can be focused with
`cargo-allow worklist --missing-evidence`, and does not, by itself, claim the
source-tree check failed.
