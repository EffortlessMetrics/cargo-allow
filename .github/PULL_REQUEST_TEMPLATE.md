## Summary

<!-- Briefly describe what changed and why. -->

## Controlling authority and reviewer focus

<!--
Give reviewers the smallest durable map for the current PR. Use `not applicable`
when the change is deliberately mechanical or docs-only; do not invent a spec.
-->

- #3768 campaign rail / controlling child:
- Execution role: author | reviewer | observer | reconciliation
- Predecessor evidence consumed:
- Current base / head / merge-base:
- Exact head intended for review (fill after the final author push):
- Scope and non-goals:
- Changed seams and semantic owners:
- Highest-risk false-green route:
- Highest-risk invariants or failure modes:
- Negative controls:
- External state observed:
- Incident / recovery lineage:
- Irreversible actions performed: none | exact authorized reference
- Claim boundary:
- Post-merge child / controller handoff:


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

<!-- List commands run locally or in CI, and note any intentionally skipped checks. -->

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cargo run -p cargo-allow -- diff --base origin/main --format markdown --require-change-note`

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
