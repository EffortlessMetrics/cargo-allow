# Diataxis Documentation Map

cargo-allow documentation follows the Diataxis model: tutorials teach, how-to
guides solve task-oriented problems, reference pages describe interfaces, and
explanations clarify concepts and boundaries.

## Tutorials

Start here when you are new to cargo-allow and want a guided path to a working
result.

- [Getting Started Tutorial](tutorials/getting-started.md): create a policy,
  run the first audit, gate future changes, and produce review artifacts.

## How-to guides

Use these when you already know the goal and need the shortest reliable path.

- [Adopt No-New in CI](how-to/adopt-no-new.md): add a no-new gate, upload
  artifacts, and state the supported CI claim accurately.
- [Triage Worklist Items](how-to/triage-worklist.md): route baseline debt,
  stale entries, broken evidence links, and broad-scope cleanup.
- [Migration From xtask](migration-from-xtask.md): replace bespoke allowlist
  tasks with the canonical ledger.
- [CI](ci.md): GitHub Actions examples for PR posture diffs and mainline checks.
- [Agent Worklist Prompt](agents/cargo-allow-worklist.md): bounded prompt for
  agents consuming `cargo-allow worklist`.

## Reference

Use reference pages when you need supported commands, fields, schemas, or
compatibility contracts.

- [CLI Reference](reference/cli.md): command purposes, shared options, and
  common invocations.
- [Source Exception Ledger](source-exception-ledger.md): canonical policy entry
  contract, selector precision, lifecycle, baseline debt, and command behavior.
- [JSON Schemas](schemas/README.md): artifact schemas for machine consumers.
- [Crate Namespace Policy](crate-namespace.md): rules for adding public crates.
- [0.1.0 Release Runbook](release/0.1.0.md): publish order and rollback limits.

## Explanation

Read these when you need to understand why cargo-allow behaves a certain way or
what its reports may safely claim.

- [Design](design.md): product lane, matching direction, evidence model, reports,
  and non-goals.
- [Claim Boundaries](claim-boundaries.md): valid claims, invalid claims, and
  adjacent tool boundaries.
- [Structural Identity V1](identity.md): why selectors are structural and line
  numbers are hints.
- [Roadmap](roadmap.md): staged product direction and milestone claims.
- [Shiplog File-Policy Dogfood](dogfood/shiplog-non-rust.md): what current
  dogfooding proves and does not prove.
