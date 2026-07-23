# cargo-intent

Durable authored intent and obligation compiler (#2599).

Most users should use `cargo intent` through the cargo subcommand alias; this crate is the cargo-intent product shell.

## Claim boundary

Product identity, config entrypoint, renderer framework, and exit mapping only. Spec-system evaluation and proof execution remain in intent-engine and cargo-proof during extraction.

## PR1 (#2599-A)

Thin binary with `--help` / `--version`, product identity, config entrypoint, human/JSON renderer framework, and process exit mapping.

## PR2 (#2599-B)

First vertical: `cargo intent change status --staged --phase precommit` — staged snapshot read, phase obligation compile plan, intent-protocol transport, and process exit mapping. Authoritative precommit findings remain in cargo-allow until #2601.
