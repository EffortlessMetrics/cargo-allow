# repo-snapshot parity fixtures (#2583-A)

Report-only contracts for revision/staged extraction parity. Tests replay `allow-diff`
APIs against these fixtures until `repo-snapshot` owns the implementations.

## Fixtures

| File | Parity case | Move ledger |
| --- | --- | --- |
| `parity-committed-head-v1.toml` | `parity-repo-snapshot-revision-identity-v1` | `move-allow-diff-revision-identity` |
| `parity-staged-index-v1.toml` | `parity-repo-snapshot-staged-index-v1` | `move-allow-diff-staged-index` |

Packet 2583-B adds the staged-deletion negative fixture.
