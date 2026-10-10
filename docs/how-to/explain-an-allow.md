# Explain an Allow Entry

Use `explain` when a maintainer or reviewer needs to know why a retained
exception exists.

> Maturity: `explain` and its companion `list` command are Stable in published
> `0.1.11` and Stabilizing on current main. See the [command maturity table](../status/SUPPORT_TIERS.md#command-maturity).

## Human Output

```bash
cargo-allow explain allow-0042
```

The human view shows the allow entry, current match status, owner, reason,
classification, lifecycle, selector details, evidence diagnostics, suggested
actions, proof commands, and claim boundary.

Current match state comes from evaluating the complete effective ledger and
then selecting the outcomes for the requested entry. If two entries tie for a
finding, explaining either entry retains the `ambiguous` finding outcome and
identifies the competing IDs in the attention and next-action sections. Narrow
the competing selectors and rerun `check --mode no-new` to verify the result.

The entry's `current_status` follows the shared lifecycle precedence, so an
expired or review-due entry keeps that status even when one of its findings is
ambiguous. The common summary uses the same entry status and also states the
finding that still needs attention. Its posture considers every projected
outcome: an advisory lifecycle annotation does not remove an ambiguity decision
or a blocking new finding.

`current_matches` counts the finding rows shown for this entry, including tied
findings. It is not a count of authorized exceptions: check each row's status.
A weaker entry that loses to a unique stronger match retains its own evaluated
state, such as `stale`, instead of reporting the winner's finding as its match.

## JSON Output

```bash
cargo-allow explain allow-0042 \
  --format json \
  --output target/cargo-allow/explain-allow-0042.json
```

Use JSON when handing work to an agent or saving audit evidence.

## What to Check

- Is the entry still matched?
- Is the selector narrow enough?
- Is the owner still correct?
- Is local evidence present?
- Is review due or expiry approaching?

## Claim Boundary

`explain` reports source-tree/source-syntax state. It does not prove that the
exception is safe or that tests are adequate.

Reference: [Source exception ledger](../source-exception-ledger.md).
