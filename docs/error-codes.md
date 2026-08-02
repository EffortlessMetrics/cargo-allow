# Error Codes

`allow_core::CargoAllowError` exposes a stable `code()` in addition to its
structured `kind()` and human-readable message. Downstream tooling should
branch on the code or kind, not on rendered message text.

`CargoAllowError` intentionally does not implement `PartialEq` or `Eq`.
Human-readable messages are operator-facing; the current rendering remains
available for existing callers but is not a durable machine contract. Consumers
should compare `kind()`, `code()`, and structured fields instead.

## Process exit codes

The `cargo-allow` binary maps failures to process exit codes as follows:

| Exit | Meaning |
| --- | --- |
| `0` | Successful command. |
| `1` | Policy/check/diff gate failure, or runtime / configuration / validation / IO / invariant failure. |
| `2` | Operator invocation / usage failure: Clap parse errors, or structured `CargoAllowErrorKind::Usage` (`E0001_USAGE`). |

Exit `2` means the invocation was wrong (bad flags, conflicting options, missing
required arguments). It does **not** mean a policy violation or an internal
instrument problem. Exit codes are chosen from the structured error kind (or by
Clap before `main`), never by matching message text.

Policy-gate failures in `check` / `diff` still exit `1` from those command
handlers. Structured usage errors that reach `main` share exit `2` with Clap.

## Registry

| Code | Kind | Meaning |
| --- | --- | --- |
| `E0001_USAGE` | `usage` | Invalid command-line usage or incompatible options. |
| `E0002_INVALID_CONFIG` | `invalid_config` | Missing or invalid runtime configuration. |
| `E0003_INVALID_POLICY` | `invalid_policy` | Invalid policy ledger, schema, or policy value. |
| `E0004_INVENTORY` | `inventory` | Source inventory discovery or access failure. |
| `E0005_SCAN` | `scan` | Source scanning or source-file read failure. |
| `E0006_POLICY_VIOLATION` | `policy_violation` | The selected policy mode rejected findings or posture. |
| `E0007_ARTIFACT` | `artifact` | Artifact rendering, receipt, or policy-write failure. |
| `E0008_INTERNAL` | `internal` | An internal invariant or implementation failure. |
| `E0009_UNKNOWN` | `unknown` | A legacy or not-yet-classified error. |

Codes are append-only and are never reused for a different failure class.
Adding a new code is compatible with consumers that handle the existing
`#[non_exhaustive]` error-kind enum conservatively. The `unknown` code remains
for `CargoAllowError::new` and other compatibility paths until those call sites
can be assigned a more specific kind.

## Located diagnostics

When a parser has source text and a TOML byte span, `CargoAllowError::location()`
returns an optional `CargoAllowErrorLocation` with the source path and one-based
line and character column. The human-readable message remains operator-facing
for existing callers; machine consumers should use `code()`, `kind()`, and
`location()` instead of parsing `Display` output. Errors created without a
source span continue to return `None`.

## Validation diagnostics

Policy validation errors expose `CargoAllowError::diagnostics()` as a stable
machine-readable list. Each `CargoAllowDiagnostic` carries its error code,
category, severity, optional source path/span, allow-entry ID, validation field,
message, help text, and causes. Aggregated validation keeps one diagnostic per
independent failure; the existing `Display` text remains an operator-oriented
summary for compatibility.
