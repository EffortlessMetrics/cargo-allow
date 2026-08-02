# Explain Why a Finding Is Unreceipted

Use `why` when a check failure points at a path and line, and you need the
inverse of `explain`: why this finding is not covered by an allow entry.

## Human Output

```bash
cargo-allow why --kind panic --path src/lib.rs --line 42
```

`--kind` is required so the command can disambiguate when multiple finding
kinds appear near the same line. The human view shows:

- the selected finding and structural identity
- current match posture (`new`, matched, ambiguous, …)
- evaluation result class (`exact_scoped`, `exact_after_full_fallback`,
  `target_scanner_partial`, or `full_fallback_unavailable`)
- nearby same-kind allow entries with per-gate selector mismatch reasons
- suggested actions, proof commands, and claim boundary

## JSON Output

```bash
cargo-allow why --kind panic --path src/lib.rs --line 42 \
  --format json \
  --output target/cargo-allow/why.json
```

JSON emits `cargo-allow.why.v1` with the same finding, outcome, candidate
entries, and next-step fields for agents and CI evidence. Candidate `family` is
omitted when the nearby policy entry has no family; selector relationship
fields such as `path`, `glob`, and `selector_glob` remain `null` when that
relationship is unavailable. The additive `evaluation.result_class` field
identifies whether the result came from a proven one-file path, an exact
full-world fallback, a partial target scan, or an unavailable full-world
fallback. A scoped result can remain `exact_scoped` while the wider repository
inventory is partial when the target file itself was scanned completely; the
inventory and scanner completeness are disclosed separately. Add-finding plans
are emitted only for the two exact classes. Older `why.v1` artifacts without
this optional field remain valid.

When a finding has a unique stronger candidate that is expired or missing
required evidence, matching may use a strictly weaker live candidate for
finding coverage. The stronger entry is still reported as stale maintenance
debt with the lifecycle or evidence reason; this does not renew, delete, or
approve that entry. Invalid selectors and equal-strength ambiguity remain
fail-closed, so a fallback cannot turn malformed policy into coverage.

## Claim Boundary

`why` reports source-tree / source-syntax matching posture only. It does not
prove that an exception is safe or that tests are adequate.

Reference: [Source exception ledger](../source-exception-ledger.md),
[Explain an allow entry](explain-an-allow.md).
