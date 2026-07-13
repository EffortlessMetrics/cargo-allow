---
id: CARGO-ALLOW-CLOSEOUT-0024
kind: closeout
status: done
owner: repo-infra
created: 2026-07-13
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/2046
merged_commit: c7123a1fdd25ad391a6aed1eeddf1a4f210eea82
support_tier_impact: advisory
---

# Closeout: S1 Concurrent Mutation Transaction

## Landed

- `init`, `add`, `propose --write`, `refresh --write`, `prune --write`, and
  `migrate` acquire a cross-process advisory lock before loading policy and
  hold it through validation and installation.
- The lock coordination file lives in the operating-system temp directory and
  is released by file-handle close, so it cannot become a source-tree finding
  or strand a future writer after process termination.
- Atomic writes use unique sibling temporary files, propagate write/flush/sync
  failures, preserve existing permissions, sync Unix parent directories, and
  restore forced replacements when installation fails.
- `concurrent_mutation.rs` proves two simultaneous `add --glob` commands both
  survive in the final policy.

## Validation

- GitHub PR #2124 workspace CI passed: format, workspace Clippy, workspace and
  doc tests, documentation, audit, no-new, spec-system checks, and artifact
  upload.
- Current-main no-new guard passed at the merged commit.
- Local cargo-allow binary suite: 502 passed; focused IO, lock, and subprocess
  mutation tests passed.

## Claim boundary and remaining work

This closes the lost-update and local durable-write implementation slice. It
does not claim exhaustive fault injection for every filesystem failure or
complete cross-platform durability proof. Those remain follow-up work under
the broader safety-complete roadmap.
