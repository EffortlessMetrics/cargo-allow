---
id: CARGO-ALLOW-CLOSEOUT-0052
kind: closeout
status: done
owner: core/policy
created: 2026-07-15
linked_plan: CARGO-ALLOW-PLAN-0009
linked_proposal: CARGO-ALLOW-PROP-0008
linked_spec: CARGO-ALLOW-SPEC-0008
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0004
support_tier_impact: advisory
policy_impact: []
---

# Closeout: GOAL-0004 PR 8 — Change-control dogfood

## Summary

The fixture-only change-control dogfood landed through PR #2309, merge commit
`cdf4428a4e92ae4ba04e86e24a4de02da76078d4`, with implementation commit
`0e68ea8ea14acaaecc7346163a0af235acde9395`.

It exercises one exact policy weakening through the complete operator loop:

```text
missing note
→ bounded generated template
→ exact authored note
→ passing current proof
→ stale-note rejection
```

The generated template is a starter only. It does not approve a policy change
or author rationale. The live repository policy is unchanged; the lifecycle
test owns the policy fixture and its temporary receipt.

## Evidence

- Hosted full CI for PR #2309 passed, including workspace tests, docs, audit,
  no-new, and spec-system checks.
- Focused lifecycle dogfood, template parser, Clippy, and no-new proof passed
  on the PR head.
- The fixture receipt binds the allow ID `allow-transition`, the
  `occurrence_limit_loosened` change, before/after fingerprints, missing-note,
  exact-note, and stale-note outcomes.

## Claim boundary

This proves one fixture-only exact weakening-note-repair journey. It does not
approve future policy changes, automatically author rationale, or prove every
policy change kind.

## Remaining work

GOAL-0004 PR 9 is now eligible and has landed as #2310. Its final campaign
closeout is tracked separately by #2298. Repository-global active-goal
retirement remains owned by #2259 and must preserve this historical identity.
