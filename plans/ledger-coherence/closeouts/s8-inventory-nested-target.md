---
id: CARGO-ALLOW-CLOSEOUT-0031
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/1847
merged_commit: 7f0a534e
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: Root-Scoped Inventory Target Pruning

## Landed

- Filesystem inventory now prunes a directory named `target` only when it is
  the repository-root build directory.
- Nested source directories such as `src/target/` are traversed and retained.
- Existing policy glob filtering continues to exclude the root `target/**`
  tree in the final inventory.

## Acceptance proof

- Regression coverage verifies nested `src/target/mod.rs` discovery through
  Git-tracked, Git include-untracked, and filesystem-fallback inventory paths.
- Root `target/debug.txt` remains excluded in all three paths.
- `cargo test -p allow-inventory --locked`: 27 passed.
- Workspace Clippy with `-D warnings`, workspace check, current no-new guard,
  and `git diff --check` passed.
- PR #2138 merged as `7f0a534e`; its required CI test passed. UB Review
  stopped at the known missing `MINIMAX_API_KEY` preflight and emitted no code
  finding.

## Validation boundary and remaining work

This closes only the hard-coded nested-`target` prune gap from #1847. It does
not establish complete inventory semantics, resource limits, symlink or
submodule policy, fallback diagnostics, permission-boundary coverage, or
cross-platform path determinism. Those remain under inventory umbrella #1783
and the completion roadmap.
