# Agent Cadence Prompt

Use this pattern when asking an agent to work the policy lifecycle queue. The
agent should treat `cadence` as a scheduling surface for owner work, not
permission to suppress findings, and never as a candidate gate: it shares no
code path with `check`/`diff` verdicts, mutates nothing, and never extends an
expiry.

## Prompt

```text
Run:

cargo-allow cadence --format json --output target/cargo-allow/cadence.json

Add `--as-of <YYYY-MM-DD>` to classify at one explicit date; omitting it uses
the ambient UTC day, which is documented as approximate and is not
deterministic across days. Every allow entry lands in exactly one class:
`current`, `review_due_soon`, `review_overdue`, `expiring`, or `expired`.
A sixth class, `invalid`, is a loader-level posture, not a row a cadence
report can render: malformed lifecycle dates fail closed at load with
`error[E0003_INVALID_POLICY]` and never reach classification. The
classification reuses the match-engine lifecycle law exactly:
`expires` flips to `expired` only strictly after the expires day (the expires
day itself is `expiring` with zero days remaining), while `review_after` is
overdue on the deadline day itself (the pre-existing #2008 `<` versus `<=`
asymmetry, preserved so cadence never disagrees with `list`, `worklist`, or
`check`).
Work the rows by `required_disposition`:
- `review_due_soon` (within 14 days): schedule the review.
- `review_overdue`: review now or narrow the entry.
- `expiring` (within 14 days): renew, narrow, or plan removal.
- `expired`: renew via a reviewed commit or remove the entry.
- `invalid`: never rendered; the loader fails closed on a malformed
  lifecycle date with `error[E0003_INVALID_POLICY]`, so fix the flagged
  date in policy and rerun.
- `current`: nothing required.
Each row retains the durable identity an owner needs: `allow_id`, owner,
policy classification (including `baseline_debt`), source path or glob,
selector summary, evidence references, the raw lifecycle dates, the signed
`days_remaining` (negative means overdue), which lifecycle date drove the
class, and the required disposition. Rows sort by class urgency, then allow
id, so the same policy and as-of date render identically every run.
Use `--format markdown` when the consumer wants the same classification as a
reviewable table; human, JSON, and markdown all derive from the one typed
result.
Cite `allow_id` when handing work back, verify details with
`cargo-allow explain <allow_id>` before changing policy, and renew lifecycle
dates only through a reviewed policy commit — cadence output is scheduling
guidance, not approval and not a license to rewrite history or policy bytes.

Do not add suppressions just to silence cargo-allow.
Do not broaden selectors, globs, occurrence limits, or expiry dates.
Do not convert baseline_debt into approval without owner, reason,
classification, lifecycle, selector, and evidence.
Do not execute external proof tools unless this task explicitly authorizes
that tool.

After the lifecycle work lands, prove it the normal way:

cargo-allow check --mode no-new

Report what changed, what proof passed, what remains uncertain, and the
source-tree claim boundary.
```

## Review Rules

Before accepting an agent change driven by cadence, check that it did one of
these:

- renewed or reviewed an entry through a reviewed policy commit.
- narrowed a broad entry instead of extending its expiry.
- removed an entry whose exception is genuinely obsolete.
- fixed a malformed lifecycle date the loader flagged with E0003.

And that it did none of these:

- treated a cadence class or disposition as a check/diff verdict or a
  candidate gate.
- extended an expiry or auto-renewed a lifecycle date without review.
- mutated policy, receipts, or candidate evaluation state as a side effect of
  running `cadence` itself (the command is read-only).
