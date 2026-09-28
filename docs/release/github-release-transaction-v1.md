# GitHub Release transaction v1

Issue #3933 owns the crash-consistent GitHub Release mutation history for the
final release, under #2502/#2509. The [journal schema](../schemas/cargo-allow.github-release-journal.v1.schema.json),
[checkpoint schema](../schemas/cargo-allow.github-release-checkpoint.v1.schema.json),
public `allow-report` models (`github_release_journal_v1`,
`github_release_checkpoint_v1`), and canonical JSON renderers record every
draft creation, exact asset upload, actual-state reconciliation, and one-way
public finalization for one exact final-release operation and GitHub Release
identity. They do not create, edit, publish, or delete a real GitHub Release,
authorize the operation, or prove asset semantics independently (#3726 retains
provenance and actual-set semantics).

## Transaction law

```text
observe existing release/draft state
        ▼
journal + checkpoint exact draft-create intent (PreMutationDurable)
        ▼
API call (outside this model)
        ▼
unknown response → reconcile by exact observation, never by re-creation
        ▼
DraftObservedExact reuses the exact accepted draft by identity
        ▼
per asset: intent → start (once) → response → exact observation
        ▼
same-name wrong-byte actual → incident, never delete/replace under clean
        ▼
extra unmanifested assets → explicit ExtraAssetObserved, blocks reconciliation
        ▼
ActualAssetSetReconciled → CloseoutReceiptComplete → FinalizeIntentDurable
        ▼
finalize → unknown response → observe exact public/draft state
        ▼
PublicReleaseObservedExact (stable channel only) → OperationComplete
```

- Draft intent is append-once: a later run reuses the exact accepted draft
  by identity; another draft or public release for the tag is a conflict.
- Asset uploads start once per asset; unknown responses reconcile by exact
  observation of the same bytes, never by re-upload.
- Checkpoints are independently read back before each mutation and before a
  later mutation consumes the resulting state; discovery is by exact object
  ID plus producer, operation name, and canonical identity digest.
- Checkpoints never invent operation identity: every record carries the
  canonical #3940 identity digest and journal prefix, linkage pins both.
- Finalization requires the exact actual draft, all expected assets observed,
  and a complete closeout; the final stable release is `draft=false`,
  `prerelease=false` on the exact tag.
- Entries, checkpoints, and incidents are append-only; a later green run
  can never omit an earlier unknown, conflict, or incident.

## Consumers

PR B/C composition applies unchanged: the journal and checkpoints bind the
canonical #3940 operation identity digest, and #2502 plus incident/recovery
paths consume the same journal/checkpoint identity. Pre-tag authority (#3790)
and live-control evidence compose in later lanes.

## Claim boundary

Append-only local and remote transaction evidence for GitHub draft creation,
exact asset uploads, actual-state reconciliation, and one-way public
finalization. It prevents runner loss or ambiguous responses from causing
duplicate, replaced, or prematurely public release state; it does not
authorize the operation or prove asset semantics independently.

Focused proof:

```text
cargo test -p cargo-allow --test github_release_journal --locked
cargo test -p cargo-allow --test github_release_checkpoint --locked
cargo test -p cargo-allow --test github_release_unknown_response --locked
cargo run -p cargo-allow -- check --mode no-new
git diff --check
```
