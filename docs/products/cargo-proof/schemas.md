# cargo-proof core schema and artifact catalog

The product's current artifact contracts are experimental:

| Artifact | Producer |
| --- | --- |
| `proof.plan.v1` | `plan --obligation-plan` (input to `dry-run`) |
| `intent.obligation-plan.v1` | consumed input produced by the intent side |
| `cargo-proof.identity.v1` | `identity --format json` |
| `cargo-proof.provider-registry.v1` | `providers --format json` |
| `proof.receipt-manifest.v1` | Captured receipt manifest input. |
| `proof.receipt-status-report.v1` | `receipts --action status` output. |
| `proof.receipt-validation.v1` | `receipts --action validate` output. |
| `proof.receipt-explain.v1` | `receipts --action explain` output for one selected item. |
| `proof.receipt-reconcile.v1` | `receipts --action reconcile` output for the complete plan. |

`proof.plan.v1` is structured-argv only: it names the provider request
and its arguments, and `dry-run` validates shape without executing.
Consumers should validate the schema ID and version and treat any
execution-shaped extension as unsupported until a live-execution command
actually exists.

Claim boundary: a valid plan proves structure and declared identity
only. It does not prove the planned command was run, succeeded, or
produced evidence.

`cargo-proof.provider-registry.v1` is a read-only projection. Its
`providers` entries contain selected provider capability catalogs, while its
`availability` entries retain every known provider ID and report
`selected` or `provider_unavailable` according to compile-time feature
selection. `provider_unsupported` is reserved for a future registry version;
the projection never claims provider execution.

Receipt status is derived from one typed report for both human and JSON
output. Findings, partial, unsupported, not-proven, instrument-failure,
conflict, stale, and missing states are not successful plan satisfaction. A
structurally valid receipt can therefore remain historical evidence without
satisfying the current plan. Unknown provider rows, duplicate rows, malformed
payloads, and identity mismatches fail closed. Provider-native receipt fields
remain in the namespaced payload and are validated by the selected provider
module; the manifest's embedded snapshot root, provider payload schema, and
receipt generation are checked against each plan item's expected contract; no
provider process is started.

Explain and reconcile are projections over the same `ProofPlanV2` and typed
receipt-status report. Explain adds plan-item context and a bounded next
action; reconcile adds deterministic status counts, provider availability, and
outstanding work. Neither projection opens a phase gate or treats receipt
presence as execution proof.
