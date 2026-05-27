# cargo-allow JSON Schemas

These schemas describe machine-readable cargo-allow artifacts. They are local
contracts for source-tree policy scans; they do not imply build, type,
macro-expansion, or proof-level coverage.

| Artifact | Schema ID | Producer |
|---|---|---|
| Audit/check/diff report | `cargo-allow.report.v1` | `cargo-allow audit --format json`, `cargo-allow check --format json`, `cargo-allow diff --format json` |
| Check receipt | `cargo-allow.receipt.v1` | `cargo-allow check --receipt <path>` |
| Single-entry explanation | `cargo-allow.explain.v1` | `cargo-allow explain <id> --format json` |
| Filtered ledger list | `cargo-allow.list.v1` | `cargo-allow list --format json` |
| Stale prune preview/result | `cargo-allow.prune.v1` | `cargo-allow prune --stale --format json` |
| Agent worklist | `cargo-allow.worklist.v1` | `cargo-allow worklist --format json` |

## Files

- [report.schema.json](report.schema.json)
- [receipt.schema.json](receipt.schema.json)
- [explain.schema.json](explain.schema.json)
- [list.schema.json](list.schema.json)
- [prune.schema.json](prune.schema.json)
- [worklist.schema.json](worklist.schema.json)

## Boundary

Every schema carries explicit source-tree claim-boundary and scanner-limitation
fields. Current cargo-allow artifacts mean:

```text
No new unreceipted findings were found in scanned source-tree inventory.
```

They do not mean:

```text
No unsafe, panic, lint suppression, or policy exception exists outside the
syntax-visible inventory that cargo-allow scanned.
```

cargo-allow does not invoke Cargo metadata, Cargo commands, rustc, Clippy,
build scripts, proc macros, or repository code for these scans.
