# Error Codes

`allow_core::CargoAllowError` exposes a stable `code()` in addition to its
structured `kind()` and human-readable message. Downstream tooling should
branch on the code or kind, not on rendered message text.

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
