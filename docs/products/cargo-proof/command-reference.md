# cargo-proof core command reference

This is the experimental product's current command surface, verified
against the workspace binary. The surface is intentionally small;
commands may change before the first published release.

| Command | Purpose |
| --- | --- |
| `identity` | Report the exact binary identity and capability surface. |
| `providers` | Report the deterministic selected-provider/capability projection and explicit availability posture. |
| `plan --obligation-plan <json>` | Project a proof plan from an `intent.obligation-plan.v1` file. |
| `dry-run --proof-plan <toml>` | Validate a `proof.plan.v1` file with structured argv only. |

Global options: `--root <path>` (repository root, default `.`) and the
shared format conventions.

`providers --format json` emits the `cargo-proof.provider-registry.v1`
projection. It includes stable IDs for selected providers and known
feature-disabled providers, whose disposition is `provider_unavailable`.
The command is read-only and does not resolve or execute provider processes.

`dry-run` is fail-closed by construction: it never spawns the planned
commands, so a malformed plan or an unrecognized provider is a bounded
error rather than an execution. There is no live-execution command in
the current surface.

Claim boundary: these commands plan and validate evidence structure.
They do not execute proof commands, mutate policy, or release anything.
