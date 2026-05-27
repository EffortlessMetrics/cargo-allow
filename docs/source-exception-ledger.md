# Source Exception Ledger

The ledger is the central cargo-allow artifact. It records exceptions that are
allowed to remain in a repository and the conditions under which they remain
acceptable.

The canonical MVP path is:

```text
policy/allow.toml
```

The primary command form is `cargo-allow ...`. `cargo allow ...` is accepted as
Cargo external subcommand compatibility, but cargo-allow's own scan is a direct
source-tree policy scan.

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

Diff mode reports owner, reason, or classification removals as policy weakening
and additions of those required metadata fields as policy improvements.

## Reason And Evidence

`reason` is the human rationale.

`evidence` is the support for that rationale.

Example:

```toml
reason = "Parser validates the range before slicing."
evidence = [
  "test:parser_rejects_invalid_text_range",
  "ripr:target/ripr/reports/parser-span-gap.json",
  "spec:docs/specs/parser-span-invariants.md",
]
```

The presence of evidence is not proof that the exception is correct. It is a
traceable claim that reviewers and tools can inspect. Diff mode reports removed
evidence as policy weakening and newly added evidence references as policy
improvements.

Known local evidence prefixes are parsed when a policy is loaded from a source
tree. `doc:`, `spec:`, `adr:`, `ripr:`, `unsafe-review:`, and `coverage:`
references must point to source-tree-relative paths that exist. `test:`,
`cargo:`, `issue:`, and `pr:` references are treated as traceability strings in
the current implementation and are not executed or resolved over the network.

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

## Selector Precision

Selectors should be as narrow as practical. Strong selectors include:

- exact path.
- kind and family.
- AST kind.
- container.
- callee, macro name, or lint.
- symbol or target fingerprint.
- normalized snippet hash.

Weak selectors include:

- broad globs.
- kind-only matching.
- missing container.
- missing callee, macro name, or lint where applicable.
- line-only matching.

Diff mode reports precision loss as policy weakening and precision increases as
policy improvements. The current precision score rewards exact paths and
structural selector fields such as AST kind, container, callee, macro name,
lint, symbol/fingerprints, snippet hash, and occurrence limits. It deliberately
does not reward line hints, because line and column are review hints rather than
identity.

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
from `created`, empty or parent-directory scopes, and selectors that contain no
structural identity beyond line hints.

Diff mode reports expiry or review-date extensions/removals as policy weakening
and added or earlier lifecycle dates as policy improvements.

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

Baseline debt must carry a short expiry. In the MVP validator, that means an
`expires` date no more than 120 days after `created` or, when `created` is
absent, the tool's deterministic fixture date.

`cargo-allow propose --write <path>` refuses to overwrite an existing file unless
`--force` is passed. The command writes only TOML to stdout or the requested
file, and emits its proposal summary to stderr so generated policy remains
parseable.

`cargo-allow add --kind <kind> --path <path> --line <line>` generates a reviewed
allow entry from the nearest current finding at that location. It copies the
finding's structural selector fields, sets owner/reason/classification and
lifecycle metadata from CLI flags, fails closed on ambiguous nearest findings,
and refuses to overwrite an output policy without `--force`.

Counted legacy baselines should also carry an `occurrence_limit`:

```toml
occurrence_limit = 3
```

The limit preserves no-new semantics during migration. Matching the same
structural selector more times than the baseline allowed becomes new debt
instead of silently broadening the exception.

Diff mode reports occurrence-limit increases or removals as policy weakening
and occurrence-limit additions or reductions as policy improvements.

`cargo-allow explain <id>` reports this live posture for a single entry. It
shows the policy metadata, selector, current match status, matched finding
count, outcome counts, stale state, occurrence-limit overruns, and evidence
reference diagnostics. Local evidence references are shown as present, missing,
or invalid; traceability strings are identified as not executed or resolved. The
command is still bounded by the normal cargo-allow claim boundary: source syntax
only, with no macro expansion, macro token-tree expression parsing, type
analysis, build output, control-flow analysis, or data-flow analysis.

`cargo-allow list` shows allow entries with current status, match count, kind,
family, owner, classification, scope, lifecycle dates, and reason. It supports
maintenance filters such as `--kind`, `--owner`, `--expired`, `--review-due`,
`--stale`, and `--baseline-debt`. Stale status is computed from current
source-syntax findings; line and column hints are not identity.

`cargo-allow prune --stale --dry-run` previews stale allow entries that no
current source-syntax finding matched. The command is dry-run only in the
current implementation: it does not write `policy/allow.toml`, and humans should
confirm the exception is gone before deleting the entry.

`cargo-allow worklist --format json` turns non-matched no-new outcomes into
agent-safe work items. Each item includes a kind, risk, difficulty, current
status, path or allow ID where available, suggested actions, and proof commands.
The worklist summary rolls up both risk and difficulty counts, and the artifact
records the source-tree inventory source, root, and `files_scanned` count when
available. `source_package` fields are scanner-provided context only; they are
not Cargo metadata or build-membership proof.
It is a routing surface, not an auto-fix plan: agents and humans should fix,
prove, narrow, or remove the exception instead of adding suppressions just to
silence cargo-allow.

The worklist command also reports broken local evidence links as
`broken_evidence_link` work items. This mode loads the policy even when local
evidence is missing so humans or agents can get a repair target; normal
`cargo-allow check` still fails closed on broken local evidence references.

`cargo-allow diff --base <rev>` compares the current policy ledger with the
base revision's `policy/allow.toml` and reports policy weakening in human and
Markdown output. Current detection covers scope broadening, selector precision
loss, expiry/review extension, evidence removal, owner/reason/classification
removal, occurrence-limit loosening, and added `baseline_debt`. This is policy
ledger comparison only.

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

Markdown diff output starts with a PR summary. The summary reports net posture,
reviewer action, current no-new failures, new and removed source findings,
policy failures, policy review items, and policy improvements. `worse` means
the diff has a failing no-new or policy-weakening signal. `review-required`
means posture changed without a failing signal, such as a receipted new source
finding or a new allow entry. `improved` means source findings or allow entries
were removed without new failures or review-required policy changes.

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

The current non-Rust family vocabulary is intentionally explicit:
`ci_declarative`, `documentation`, `release_script`, `test_fixture`,
`generated_code`, `editor_extension`, `package_metadata`, `shell_script`,
`python_tool`, `javascript_tool`, `configuration`, and `unknown_non_rust`.

Human and Markdown audit reports summarize the non-Rust file inventory by
status and family, then list the scanned file paths. This is a review surface,
not a separate schema contract; the stable machine-readable report remains the
versioned JSON output.
