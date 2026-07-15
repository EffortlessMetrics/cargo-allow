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
- nearby same-kind allow entries with per-gate selector mismatch reasons
- suggested `add` / `explain` / `check` next steps

## Claim Boundary

`why` reports source-tree / source-syntax matching posture only. It does not
prove that an exception is safe or that tests are adequate.

Reference: [Source exception ledger](../source-exception-ledger.md),
[Explain an allow entry](explain-an-allow.md).
