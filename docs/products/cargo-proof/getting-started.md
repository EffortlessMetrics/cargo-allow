# cargo-proof getting started

`cargo-proof` is the opt-in experimental exact-snapshot evidence
orchestrator. It plans and dry-runs proof execution from authored
intent obligations. It is a separate product: installing or running
`cargo-allow` or `cargo-intent` does not install, enable, or imply
`cargo-proof`.

## First hour

`cargo-proof` is an experimental `0.1.0` workspace product; it is not
on the published install channel yet. Evaluate it from the source tree:

```bash
cargo run -p cargo-proof -- identity
cargo run -p cargo-proof -- --format json providers
cargo run -p cargo-proof -- plan --obligation-plan <intent.obligation-plan.v1.json>
cargo run -p cargo-proof -- dry-run --proof-plan <proof.plan.v1.toml>
```

`plan` consumes an `intent.obligation-plan.v1` JSON file produced by the
intent side and projects a structured proof plan. `dry-run` validates a
`proof.plan.v1` TOML file with structured argv only — it never executes
the planned commands. Live provider execution is not part of the current
surface.

Use `providers --format json` to inspect the selected provider capability
projection and explicit feature-disabled posture. The output is identified
as `cargo-proof.provider-registry.v1`; it is read-only and does not execute
or resolve provider processes.

Claim boundary: cargo-proof orchestrates exact-snapshot evidence. It
does not execute proof commands today, does not scan source trees, and
does not turn a dry-run into a runtime, security, or release claim.
