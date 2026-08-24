## Summary

<!-- Briefly describe what changed and why. -->

## Scope and claim boundary

<!--
Make the lane independently understandable from this PR. Separate what the
change establishes from adjacent work and from external/irreversible actions.
Do not imply authorization merely because implementation is complete.
-->

- This PR owns:
- Explicit non-goals / deferred owner:
- Irreversible or external actions performed: `none` unless separately authorized and named here
- Claim boundary — what this PR proves, and what it does not prove:

## Controlling authority and reviewer focus

<!--
Give reviewers the smallest durable map for the current PR. Use `not applicable`
when the change is deliberately mechanical or docs-only; do not invent a spec.
-->

- Controlling issue/spec/requirement:
- Parent/controller and predecessor receipt, when applicable:
- Intended base:
- Exact head intended for review (fill after the final author push):
- Changed seams and semantic owners:
- Highest-risk invariants or failure modes:
- Required negative, adversarial, replay, or platform proof:
- Schema, docs, package, support, release, or migration impact:

## Source-exception ledger impact

<!--
Describe any cargo-allow posture changes. If this PR touches unsafe syntax,
panic-family calls, indexing/slicing, lint suppressions, non-Rust tracked files,
generated-code policy, or policy/allow.toml, include the relevant
cargo-allow diff/audit output or explain why it does not apply.
-->

- [ ] No source-exception posture change
- [ ] Adds or changes source findings
- [ ] Removes or narrows source findings
- [ ] Changes policy entries or evidence

## Validation

<!--
List commands actually run locally or in CI and record pass/fail/not-run rather
than leaving a checked box to imply evidence. Retain receipt/run identities for
load-bearing release or platform proof.
-->

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cargo run -p cargo-allow -- diff --base origin/main --format markdown`
- [ ] `cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md`

Commands/results/receipt or hosted-run identities:

## Review readiness

<!--
The author supplies review inputs; the reviewer/operator determines the verdict.
Follow `.agents/skills/review-current-head/SKILL.md` for substantive review and
merge-readiness verification.
-->

- [ ] The PR body describes the current head, not an earlier candidate.
- [ ] The changed-file set and relevant callers/consumers are ready to inspect.
- [ ] Existing review threads were checked before adding new feedback.
- [ ] Repairs or material base changes after review will receive a fresh affected review.
- [ ] Required checks and retained receipts will be verified against the final base/head pair.

Final reviewed base and merge-base:

Final reviewed head:

Final review source and independence posture:

Required-check and unresolved-thread disposition:

## Review notes

<!-- Call out reviewer focus areas, risks, follow-ups, or migration notes. -->
