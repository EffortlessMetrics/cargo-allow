# repo-snapshot

Human projection of the shared snapshot crate (#2583).

## Claim boundary

Exact repository source views and Git snapshot reads. PR1 (#2583-A) lands the crate
skeleton and parity fixtures over current `allow-diff` revision/staged APIs.

Parity fixtures live under `tests/fixtures/repo-snapshot/`. Packet 2583-B adds the
staged-deletion negative fixture (staged delete + dirty worktree replacement → path absent).
Packet 2583-C moves generic `RepositorySourceView` into `repo-snapshot::source_view`;
the cargo-allow self-hosted graph consumer now imports that canonical surface (3146-B).
Remaining intent/proof consumers and compatibility-dependency removal are tracked as follow-up
work rather than duplicated here.
Packet 2583-D moves `git`, `revision_identity`, and `staged_index` implementations into `repo-snapshot`;
`allow-diff` retains publish-safe ModuleFacade shims via `snapshot_package/` copies.

## Module surfaces

- `repo-snapshot::revision_identity` — committed revision/tree identity (moves from `allow-diff`)
- `repo-snapshot::staged_index` — staged Git index snapshot (moves from `allow-diff`)
- `repo-snapshot::source_view` — generic filesystem/staged/committed source views (#2583-C)

Transport envelopes use `repo-protocol::RepositorySnapshotV1` via the PR1 adapter.
