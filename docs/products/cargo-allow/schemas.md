# cargo-allow core schema and artifact catalog

This page is a core subset for quick orientation, not an exhaustive artifact inventory.
The complete [cargo-allow schema catalog](../../schemas/README.md)
is canonical and includes release-control, supporting-harness, and other
non-governed contracts that are intentionally not repeated here.

The canonical machine-readable artifact contracts live in the complete catalog
linked above. The core cargo-allow artifacts are:

| Artifact | Producer |
| --- | --- |
| `cargo-allow.doctor.v1` | `doctor --format json` |
| `cargo-allow.report.v1` | `audit`, `check`, and `diff` |
| `cargo-allow.receipt.v1` | `check` and `diff` receipt output |
| `cargo-allow.explain.v1` / `cargo-allow.why.v1` | `explain` and `why` |
| `cargo-allow.worklist.v1` | `worklist --format json` |
| `cargo-allow.core-adoption-plan.v1` | `adopt --format json` |

Consumers should validate the schema ID and version, preserve the commit and
tree identity where present, and treat incomplete or stale evidence as not
proven. The catalog is the source of truth; examples are characterization,
not approval.

## Receipt integrity binding (check receipts only)

The `cargo-allow.receipt.v1` artifact produced by `check` records the policy
bytes actually evaluated and the contextual repository `HEAD` resolved for the
requested root. These unsigned observations do not authenticate the source
bytes or invocation. Diff receipts are intentionally out of scope for this
binding follow-up.

- `git_sha`: the contextual `HEAD` commit resolved for the requested root.
  Absent when that root is not a Git repository or the commit cannot be
  resolved.
- `policy_digest`: a versioned SHA-256 (`sha256:v1:<hex>`) of the active ledger
  bytes loaded and evaluated for a successful check. It is also absent from
  generic error receipts because the error writer intentionally does not retain
  evaluated provenance; absence is not evidence that evaluation never began.
- `started_at` / `run_id`: wall-clock start time (RFC 3339 UTC) and a
  process-unique invocation id correlating a receipt to one run.

Trust assumption: these fields are unsigned, best-effort observations recorded by
the tool itself. An external verifier must independently resolve the expected
repository HEAD and policy bytes before using them to detect mismatches; the
fields do not authenticate the invocation, `started_at`, or `run_id`, and do not
defend against an attacker who controls the writing machine. Receipts with
timestamps are not byte-stable across runs.

Claim boundary: a valid schema proves artifact shape and declared evidence
identity only. It does not promote support, publication, or semantic coverage.
