# proof-adapter-command

Reviewed proof command registry and adapter contracts for three-product extraction (#2603-B).

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) or downstream cargo-proof products; `proof-adapter-command` defines reviewed command registry entries, structured program/argv invocation specs, dry-run projections, and receipt interpretation. It does not spawn processes or execute shell derived from issue/spec prose.

## Claim boundary

Packet 2603-B lands reviewed command registry, structured invocation specs, cwd/env/io declarations, dry-run projection, and receipt interpretation. Process execution remains proof-engine owned.

`proof-adapter-command` does not scan source files, does not invoke Cargo, compile code, or depend on intent crates.

## Packet 2603-B

- `proof-adapter-command::command_registry` — reviewed command registry transport
- `proof-adapter-command::command_spec` — structured program/argv invocation specs
- `proof-adapter-command::dry_run` — dry-run projections without prose-to-shell
- `proof-adapter-command::receipt_interpretation` — command receipt interpretation
