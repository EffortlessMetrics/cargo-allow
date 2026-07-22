# repo-protocol

Human projection of the shared protocol crate (#2582).

## Claim boundary

Provider-neutral repository identity and transport envelopes only. The first
migrated envelope is [`RepositorySnapshotV1`](../../crates/repo-protocol/src/repository_snapshot.rs),
with field-parity fixtures mirroring `allow-diff::RepositorySnapshotIdentity` (production adapter wiring lands in #2583).

No Git access, filesystem IO, or product-domain semantics live in this crate.
