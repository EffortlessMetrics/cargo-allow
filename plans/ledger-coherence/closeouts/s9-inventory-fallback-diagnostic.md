---
id: CARGO-ALLOW-CLOSEOUT-0032
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2140
merged_commit: 2b70c2cb
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: Include-Untracked Fallback Diagnostic

## Landed

- When Git inventory fails for `include_untracked`, filesystem fallback now
  preserves the original `git ls-files` error in `Inventory.git_error`.
- The requested inventory source remains explicitly
  `filesystem_include_untracked`; the error metadata discloses why Git could
  not provide the requested inventory.
- A regression test covers fallback files, source classification, and the
  non-empty preserved diagnostic.

## Acceptance proof

- `cargo test -p allow-inventory --locked`: 28 passed.
- Workspace Clippy with `-D warnings`, workspace check, current no-new guard,
  and `git diff --check` passed.
- PR #2141 merged as `2b70c2cb`; its required CI test passed. UB Review
  stopped at the known missing `MINIMAX_API_KEY` preflight and emitted no code
  finding.

## Validation boundary and remaining work

This closes only fallback error preservation. It does not yet guarantee that
all commands render the diagnostic uniformly, provide a single completeness
enum, enforce resource limits, or prove cross-platform inventory parity. Those
remain under #1783 and the completion roadmap.
