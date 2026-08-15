# cargo-allow core command reference

This is a core quick-reference, not an exhaustive command inventory. The
[published command registry](../../dogfood/fixtures/getting-started/published-command-registry.toml)
is the canonical, complete list for the published `0.1.11` channel; consult it
before treating a command as part of the supported install surface.

The supported product command surface is the source-tree ledger and its
review/evidence helpers:

| Command | Purpose |
| --- | --- |
| `doctor` | Check repository and policy setup. |
| `audit` | Inventory syntax-visible findings and policy health. |
| `check` | Validate policy, including the `no-new` gate. |
| `diff --base <ref>` | Report posture movement against an exact base. |
| `list`, `explain`, `why` | Inspect retained entries and unreceipted findings. |
| `init`, `propose`, `add`, `refresh`, `prune` | Plan or apply bounded ledger changes. |
| `worklist` | Emit actionable repair items. |

Report commands such as `audit`, `check`, `diff`, `list`, `explain`, `why`, and
`worklist` use `--format json` for machine-readable output. Mutation commands
use their command-specific summary flags, for example
`propose --summary-format json --summary-output <path>` and
`add --summary-format json --summary-output <path>`; they write receipts and
should be run from a clean, reviewable lane.

The published command set is pinned by that registry; source-candidate commands
are not automatically published commands.

Claim boundary: command output describes source-tree observations and ledger
state. It is not a compiler, semantic analyzer, test runner, or release proof.
