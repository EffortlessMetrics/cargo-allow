# cargo-proof getting started

`cargo-proof` is the opt-in experimental exact-snapshot evidence
orchestrator. It plans and dry-runs proof execution from authored
intent obligations. It is a separate product: installing or running
`cargo-allow` or `cargo-intent` does not install, enable, or imply
`cargo-proof`.

## First hour

This guide describes the current source tree. Exact public `0.1.0`
registry observations and checksums are retained in
[#3707](https://github.com/EffortlessMetrics/cargo-allow/issues/3707#issuecomment-5390990745).
Registry visibility does not establish installed-product qualification,
publication provenance, or stable/direct support; those remain incomplete.
These source commands do not qualify the immutable installed `0.1.0` binary.

Run from the repository root. Replace the angle-bracket placeholders with
actual paths: an existing `intent.obligation-plan.v1` JSON envelope, a captured
receipt inventory, and a **new** output path in an existing directory. The
planner refuses to overwrite an existing output file.

```bash
cargo run -p cargo-proof -- identity
cargo run -p cargo-proof -- --format json providers
cargo run -p cargo-proof -- plan \
  --obligation-plan <intent.obligation-plan.v1.json> \
  --receipt-inventory <captured-receipts.json> \
  --output <proof.plan.v2.json>
cargo run -p cargo-proof -- dry-run --proof-plan <proof.plan.v2.json>
```

The receipt inventory is a `proof.captured-receipt-store.v1` JSON object.
For a first plan with no captured receipts, its contents can be:

```json
{"schema_id":"proof.captured-receipt-store.v1","sets":[]}
```

Supplying both `--receipt-inventory` and `--output` selects the V2 planner.
It consumes the intent envelope and receipt inventory, uses the compile-time
selected provider registry, and atomically writes `proof.plan.v2` JSON.
`dry-run` reads that **same output artifact**, validates it, and displays only
items selected for execution. It never executes provider processes. An empty
dry-run display is not evidence that obligations are satisfied.

Default features select **zero providers**. Provider-dependent obligations
can therefore remain `ProviderUnavailable`. The item-disposition check returns
non-success for any `RepositoryDecisionRequired` item, or a blocking item outside
`SelectedForExecution`, `SelectedForCapturedIngestion`, `SatisfiedByCurrentReceipt`,
and `NotApplicableWithReason`, even when `plan` has written the artifact.
Advisory non-clean items do not by themselves make this check fail.
Inspect the diagnostics and item dispositions before
using that artifact. Successful plan generation or dry-run validation is not
proof execution, receipt currentness, or a satisfied gate.

To opt into a provider such as `provider-cargo-allow`, add
`--features provider-cargo-allow` before `--` to **every** cargo-proof invocation
above, keeping the same feature set for discovery, planning, and dry-run.
Feature selection exposes that provider's capabilities; it does not guarantee
that every obligation is supported, selected, or satisfied.

Use `--format json providers` to inspect the selected provider capability
projection and explicit feature-disabled posture. The output is identified
as `cargo-proof.provider-registry.v1`; it is read-only and does not execute
or resolve provider processes.

Historical `proof.plan.v1` TOML fixtures remain accepted by the dry-run engine.
They are a compatibility route, not the output of the V2 sequence above. The
legacy `plan --obligation-plan` route without receipt/output inputs deliberately
returns `ProviderUnavailable` for valid input; it is not a first-use success
path.

Claim boundary: cargo-proof orchestrates exact-snapshot evidence. It
does not execute proof commands today, does not scan source trees, and
does not turn a dry-run into a runtime, security, or release claim.
