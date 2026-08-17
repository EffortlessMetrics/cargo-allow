# cargo-intent core command reference

This is the experimental product's current command surface, verified
against the workspace binary. The surface is intentionally small;
commands may change before the first published release.

| Command | Purpose |
| --- | --- |
| `identity` | Report the exact binary identity and capability surface. |
| `change status` | Staged change status for a lifecycle phase. |
| `governance` | Compile the governance authority into a validation receipt (#2942). |

Global options: `--root <path>` (repository root, default `.`),
`--config <path>` (default `.allow/intent.toml`), and
`--format human|json` for machine-readable output.

The `governance` command takes `--receipt <path>` and writes a
`cargo-intent.governance-receipt.v1` artifact. It is the product's
load-bearing output today: the repository's CI consumes it as the
governance-cutover evidence chain, and a failed or partial compile
fails closed.

Claim boundary: these commands evaluate authored intent and governance
declarations. They do not scan for source exceptions, execute proof
commands, mutate policy, or release anything.
