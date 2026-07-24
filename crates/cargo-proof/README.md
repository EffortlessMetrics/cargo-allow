# cargo-proof

Exact-snapshot evidence orchestration shell (#2589-B).

Most users should use `cargo proof` through the cargo subcommand alias; this crate is the cargo-proof product shell.

## Claim boundary

Product identity, config entrypoint, renderer framework, process exit mapping, and proof-engine plan/dry-run wiring only. Process execution and provider adapters land in follow-on packets.

## PR1 (#2589-B)

Thin binary with `--help` / `--version`, product identity, human/JSON renderer framework, process exit mapping, and `plan` / `dry-run` commands wired to proof-engine surfaces.
