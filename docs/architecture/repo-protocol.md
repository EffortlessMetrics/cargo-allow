# repo-protocol

Human projection of the shared protocol crate (#2582).

## Claim boundary

Provider-neutral repository identity and transport envelopes only. The first
migrated envelope is [`RepositorySnapshotV1`](../../crates/repo-protocol/src/repository_snapshot.rs),
adapted from `allow-diff::RepositorySnapshotIdentity` via
`allow-diff::repository_snapshot_v1_from_identity`.

No Git access, filesystem IO, or product-domain semantics live in this crate.
