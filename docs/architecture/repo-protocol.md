# repo-protocol

Human projection of the shared protocol crate (#2582).

## Claim boundary

Provider-neutral repository identity and transport envelopes only. The first
migrated envelope is [`RepositorySnapshotV1`](../../crates/repo-protocol/src/repository_snapshot.rs),
adapted from `allow-diff::RepositorySnapshotIdentity` via the PR1 parity adapter in
`cargo-allow` tests (production wiring lands with `repo-snapshot` / #2583).

No Git access, filesystem IO, or product-domain semantics live in this crate.
