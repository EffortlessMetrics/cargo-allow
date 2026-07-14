---
id: CARGO-ALLOW-CLOSEOUT-0037
kind: closeout
status: done
owner: repo-infra
created: 2026-07-14
linked_issue: https://github.com/EffortlessMetrics/cargo-allow/issues/1802
merged_commit: ca7beef6
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
---

# Closeout: Unique Strongest Match Resolution

## Landed

- `allow-match` now selects a unique strongest candidate when multiple policy
  entries match a finding.
- Genuine equal-strength ties remain `Ambiguous` and expose deterministic
  candidate IDs; tied candidates are retained as used rather than demoted to
  stale.
- The implementation uses the explicit `MatchStrength` tiers and no longer
  treats every multi-candidate result as equally ambiguous.

## Acceptance proof

- The focused evaluation fixture proves a unique top scorer is matched rather
  than reported as `Ambiguous`.
- The tie fixture proves equal-strength candidates remain `Ambiguous`, reports
  both candidate IDs, and does not create stale outcomes.
- PR #2123 merged the implementation as `ca7beef6`.
- Current-main `cargo test -p allow-match --locked`: passed.
- Current-main `cargo clippy -p allow-match --all-targets --locked -- -D warnings`:
  passed.
- The current-main no-new guard passed with no new findings.

## Validation boundary and remaining work

This closes the unique-best ambiguity regression tracked by #1802. It does not
close the broader #1777 matching umbrella, add generated property tests,
candidate mismatch reasons, or change occurrence accounting and shared read
surface work.
