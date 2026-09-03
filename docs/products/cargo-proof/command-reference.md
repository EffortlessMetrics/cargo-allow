# cargo-proof core command reference

This is the experimental product's current command surface, verified
against the workspace binary. The surface is intentionally small;
commands may change before the first published release.

| Command | Purpose |
| --- | --- |
| `identity` | Report the exact binary identity and capability surface. |
| `providers` | Report the deterministic selected-provider/capability projection and explicit availability posture. |
| `plan --obligation-plan <json>` | Project the legacy proof plan from an `intent.obligation-plan.v1` file. |
| `plan --obligation-plan <json> --receipt-inventory <json> --output <json>` | Generate a V2 plan from the selected provider registry and captured receipt inventory. |
| `dry-run --proof-plan <toml>` | Validate a `proof.plan.v1` file with structured argv only. |
| `receipts --action validate --plan <json> --receipts <json>` | Validate a captured receipt manifest read-only. |
| `receipts --action status --plan <json> --receipts <json>` | Classify every proof item from captured receipts. |
| `receipts --action explain --item <proof-item-or-obligation-id> --plan <json> --receipts <json>` | Explain one proof item from the typed captured-receipt status. |
| `receipts --action reconcile --plan <json> --receipts <json>` | Deterministically summarize all captured-receipt statuses and outstanding work. |

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

`receipts --action explain` selects exactly one proof item or obligation; an
ambiguous selector is rejected. `reconcile` preserves plan order and reports
provider availability and outstanding work as a non-gating projection.

Claim boundary: these commands plan, validate, explain, and reconcile explicit
evidence structure. They do not execute providers, mutate source, open gates,
or release anything.
Receipt commands dispatch provider-native payload validators when the selected
feature is available. They do not execute proof commands, mutate policy,
reconcile plans through a semantic gate, open gates, or release anything.
