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

### Receipt integrity binding

The `cargo-allow.receipt.v1` artifact binds each run to what it observed:

- `git_sha`: the resolved `HEAD` commit of the scanned source tree. Absent when
  the tree is not a Git repository or the commit cannot be resolved.
- `policy_digest`: a versioned SHA-256 (`sha256:v1:<hex>`) of the active ledger
  file bytes, read when the receipt is rendered. Absent when no ledger path
  resolves.
- `started_at` / `run_id`: wall-clock start time (RFC 3339 UTC) and a
  process-unique invocation id correlating a receipt to one run.

Trust assumption: these fields are unsigned, best-effort provenance recorded by
the tool itself. They make tampering or cross-run receipt substitution
detectable by re-computing the digest at the named commit; they do not defend
against an attacker who controls the machine writing the receipt. Receipts with
timestamps are not byte-stable across runs.

Claim boundary: a valid schema proves artifact shape and declared evidence
identity only. It does not promote support, publication, or semantic coverage.
