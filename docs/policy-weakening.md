# Policy Weakening

Policy weakening is a change that makes a retained source exception easier to
hide, harder to review, or less strongly evidenced.

Many governance failures are not new `unsafe` blocks or new panic-family calls.
They are policy edits that silently broaden approval. cargo-allow treats those
as PR posture changes.

## Failing Weakening Signals

These changes are failing policy-posture signals:

- `scope_broadened`: exact paths or narrow globs become broader source-tree
  scopes.
- `selector_precision_decreased`: structural selector fields such as
  `container`, `callee`, `macro_name`, `lint`, `symbol`, or snippet identity are
  removed.
- `occurrence_limit_loosened`: a capped baseline allows more occurrences or
  becomes unlimited.
- `evidence_removed`: typed evidence is removed.
- `owner_removed` or `owner_unassigned`: concrete ownership is removed.
- `reason_removed`: rationale is removed.
- `classification_removed`: classification is removed.
- `baseline_debt_introduced`: reviewed policy becomes generated baseline debt.
- `baseline_debt_normalized`: generated baseline debt is made to look reviewed
  without the expected review work.
- `requirement_loosened`: policy requirements such as owner, reason,
  classification, lifecycle enforcement, or
  `requirements.unsafe.verified_evidence_required` are relaxed.
- `workspace_ignored_added`: source-tree inventory scopes are hidden from the
  scan.

## Review-Required Changes

These changes are not always worse, but they require review:

- `scope_changed`: the retained exception is retargeted to a different exact
  source-tree surface.
- `selector_changed`: equal-precision structural selector identity changes.
- `owner_changed`, `reason_changed`, or `classification_changed`.
- `created_changed`.
- `expiry_extended`: `expires` is pushed out or removed.
- `review_after_extended`: review is pushed out or removed.
- weak evidence or traceability changes that cannot be validated locally.
- generated scope changes.

## Improvement Signals

These changes usually improve posture:

- stale allow removed.
- source finding removed.
- source-tree scope narrowed.
- selector precision increased.
- occurrence limit tightened.
- expiry or review date shortened.
- typed evidence added.
- concrete owner, reason, classification, or lifecycle added.
- ignored inventory scope removed.
- generated `baseline_debt` reduced by removing or reviewing entries.

## Why Selector Precision Matters

Line and column are hints only. Durable receipts should match source exceptions
by structural identity where possible:

```text
kind + path + AST kind + container + callee/macro/lint/symbol + snippet identity
```

Removing structural fields makes a receipt easier to match accidentally, so
cargo-allow reports precision loss separately from ordinary text edits.

## Evidence Boundary

cargo-allow validates local evidence references it can see, such as `doc:`,
`spec:`, `adr:`, `ripr:`, `unsafe-review:`, and `coverage:` paths. It does not
run tests, ripr, unsafe-review, coverage tools, GitHub APIs, or network checks.
The optional `[requirements.unsafe] verified_evidence_required = true` setting
requires at least one such local-file reference for unsafe entries;
traceability-only references remain supplementary and unresolved.

## Related Docs

- [PR posture](pr-posture.md)
- [Source exception ledger](source-exception-ledger.md)
- [Claim boundaries](claim-boundaries.md)
- [JSON schemas](schemas/README.md)
