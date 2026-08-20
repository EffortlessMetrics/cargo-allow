# cargo-proof core schema and artifact catalog

The product's current artifact contracts are experimental:

| Artifact | Producer |
| --- | --- |
| `proof.plan.v1` | `plan --obligation-plan` (input to `dry-run`) |
| `intent.obligation-plan.v1` | consumed input produced by the intent side |
| `cargo-proof.identity.v1` | `identity --format json` |
| `cargo-proof.provider-registry.v1` | `providers --format json` |

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
