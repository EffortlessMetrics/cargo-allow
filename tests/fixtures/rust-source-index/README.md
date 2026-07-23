# rust-source-index parity fixtures (#2587-A)

Report-only contracts for structural test-subject extraction parity. Tests replay
`allow-rust::test_subjects` APIs against these fixtures until `rust-source-index`
owns the implementations.

## Fixtures

| File | Parity case | Move ledger |
| --- | --- | --- |
| `parity-test-subjects-v1.toml` | `parity-rust-source-index-test-subjects-v1` | `move-allow-rust-test-subjects` |

Packet 2587-B moves subject/selector/result DTOs. Packet 2587-C moves discovery
and `allow-rust` compatibility re-exports.
