<p align="center">
  <img src="https://raw.githubusercontent.com/EffortlessMetrics/cargo-allow/main/docs/assets/cargo-allow-checkmark.svg" alt="cargo-allow green checkmark logo" width="96" height="96">
</p>

# cargo-allow

Source-tree exception ledger and policy scanner for Rust repositories.

## What this crate owns

This package builds the `cargo-allow` command-line interface. It wires together
source-tree inventory, Rust and non-Rust scanners, policy loading, matching,
diff posture, reports, receipts, evidence diagnostics, and worklist output.

## Who should use it

Most users should install and run this binary rather than depending on the
workspace libraries directly:

```bash
cargo install cargo-allow --locked
```

Use `cargo-allow` to audit existing exception debt, block new unreceipted
exceptions in CI, explain retained exceptions, review PR posture, and generate
bounded worklists for humans or agents.

## Claim boundary

`cargo-allow` scans repository files without executing project code. It does
not require Cargo metadata, compilation, rustc, Clippy, build scripts, proc
macro expansion, type analysis, MIR, or proof-tool execution.

## Stability

The CLI, policy schema, and artifact contracts are still hardening during the
0.x series. See the repository README and `docs/claim-boundaries.md` for the
current product documentation.
