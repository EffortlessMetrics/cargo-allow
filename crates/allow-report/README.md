# allow-report

Part of `cargo-allow`, a direct source-tree exception ledger for Rust
repositories.

## What this crate owns

`allow-report` renders human and machine artifacts for cargo-allow: reports,
receipts, explain output, worklists, doctor diagnostics, Markdown summaries,
HTML, SARIF, and JSON contract surfaces.

It keeps claim boundaries visible in artifacts: source-tree inventory,
source-syntax scanning, no repository code execution, and scanner limitations.

## Who should use it

Most users should use the `cargo-allow` binary. Use this crate directly only if
you are integrating cargo-allow outputs into CI, dashboards, review tooling, or
agent workflows.

## Claim boundary

This crate renders data produced by the scanner, policy, matcher, and diff
layers. It does not scan files, compile code, execute evidence providers, call
network services, or strengthen the underlying product claim.

## Stability

This crate is versioned with the cargo-allow workspace. Public APIs and artifact
contracts may evolve during the 0.x series, with schema versions documenting
machine-facing changes.

## Links

- Binary crate: `cargo-allow`
- Product docs: repository README
- Claim boundaries: `docs/claim-boundaries.md`
