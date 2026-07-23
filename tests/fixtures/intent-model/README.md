# intent-model parity fixtures (#2584-A)

Report-only contracts for spec-system domain extraction parity. Tests replay
`allow-policy::spec_system` APIs against these fixtures until `intent-model`
owns the implementations.

## Fixtures

| File | Parity case | Move ledger |
| --- | --- | --- |
| `parity-spec-system-v1.toml` | `parity-intent-model-spec-system-v1` | `move-allow-policy-spec-system` |

Packet 2584-B moves domain DTOs into `intent-model::spec_system`; `allow-policy`
keeps a publish-safe snapshot copy in sync. Packet 2584-C moves parsing helpers
and compatibility re-exports.
