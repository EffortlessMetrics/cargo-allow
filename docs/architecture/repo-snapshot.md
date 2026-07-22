# repo-snapshot

Human projection of the shared snapshot crate (#2583).

## Claim boundary

Exact repository source views and Git snapshot reads. PR1 (#2583-A) lands the crate
skeleton and parity fixtures over current `allow-diff` revision/staged APIs.

Parity fixtures live under `tests/fixtures/repo-snapshot/`. Implementation moves and
the staged-deletion negative fixture land in packets 2583-B through 2583-D.

## Module surfaces

- `repo-snapshot::revision_identity` — committed revision/tree identity (moves from `allow-diff`)
- `repo-snapshot::staged_index` — staged Git index snapshot (moves from `allow-diff`)

Transport envelopes use `repo-protocol::RepositorySnapshotV1` via the PR1 adapter.
