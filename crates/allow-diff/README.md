# allow-diff

Part of `cargo-allow`, a direct source-tree exception ledger for Rust
repositories.

## What this crate owns

`allow-diff` provides PR posture and policy-diff helpers. It compares source
findings and policy entries across revisions so cargo-allow can report whether
a change added, removed, broadened, weakened, or improved exception posture.

It supports the `cargo-allow diff --base ...` workflow.

## Who should use it

Most users should use the `cargo-allow` binary. Use this crate directly only if
you are integrating cargo-allow posture evaluation into another review or CI
tool.

## Claim boundary

This crate does not decide source safety, compile code, execute repository
code, run Cargo metadata, or invoke proof tools. It reports posture changes
against the policy ledger and source-syntax inventory.

## Revision path platform boundary

Revision-scoped file reads use exact Git tree lookup with
`--literal-pathspecs`, then read the returned blob by OID:

- On Unix, Git tree path identities are byte-exact.
- On Windows, caller paths must be UTF-8 representable and repository-relative.
  Ordinary relative host separators (`\`) are mapped to Git `/` only after the
  original path components are inspected.
- Drive-prefixed, UNC/device, and rooted host paths are rejected rather than
  normalized into Git identity.
- Valid Git-only names that the Windows worktree or index cannot materialize
  remain explicitly unsupported on that host (`tree_path_unsupported_on_platform`).

## Stability

This crate is versioned with the cargo-allow workspace. Public APIs may evolve
while the 0.x series hardens policy-weakening and improvement detection.

## Links

- Binary crate: `cargo-allow`
- Product docs: repository README
- Claim boundaries: `docs/claim-boundaries.md`
