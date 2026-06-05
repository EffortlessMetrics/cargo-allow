# LEM budgeting

Linux-equivalent minutes, or LEM, are the repository's CI fuel gauge:

```text
LEM = wall-clock minutes x runner multiplier
```

The goal is to reduce wasted CI so the repository can afford stronger evidence
where it matters.

## Runner multipliers

Use Linux as the baseline multiplier of `1.0`. More expensive or constrained
runners should carry higher multipliers so a PR plan reflects their real cost.
Typical examples are:

| Runner class | Example multiplier |
| --- | ---: |
| Ubuntu/Linux | 1.0 |
| Windows | 2.0 |
| Docker-heavy lanes | 6.0 |
| GPU lanes | 6.0 |
| macOS | 10.0 |

The exact values are repository policy, but the budget should make expensive
runners visible before they become default PR requirements.

## Budget posture

A mature repository should define:

- a preferred default PR budget;
- a default hard limit;
- an elevated limit for labeled or risk-routed PRs;
- a hard cap that requires explicit maintainer acknowledgement.

The budget is a planning tool, not an excuse to drop proof. If a change needs
expensive validation, route it deliberately and record the reason.

## Receipts

CI planning and actuals should produce machine-readable receipts when the
repository has the orchestration surface to do so. Useful artifacts include:

- selected lanes;
- estimated LEM;
- actual wall time;
- runner class;
- cache behavior;
- skipped optional lanes and skip reasons.
