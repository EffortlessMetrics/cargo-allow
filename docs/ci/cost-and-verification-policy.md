# Cost and verification policy

The CI objective is proof per Linux-equivalent minute, not fewer checks.
Default PR lanes should stay fast enough for high-throughput review while deep
validation remains available for the changes that need it.

## Default posture

- Run cheap, deterministic, high-signal checks on ordinary PRs.
- Route expensive checks by risk pack, label, mainline, nightly, or release
  event.
- Keep optional lane outcomes explicit: `passed`, `failed`,
  `skipped-by-policy`, or `advisory-failed`.
- Prefer one aggregate branch-protection signal such as `PR Gate Success` over
  requiring every leaf workflow directly.

## What belongs in the default PR gate

Default PR validation should prioritize checks with a strong proof-to-cost
ratio:

- formatting and compilation-oriented checks for changed Rust surfaces;
- focused unit and integration tests;
- `cargo-allow diff` or `cargo-allow check --mode no-new` for source-exception
  posture;
- advisory static source signals such as `ripr` when the repository has adopted
  them and their runtime cost is low.

## What belongs in routed lanes

Routed lanes preserve deeper proof without making every small PR pay for it:

- full coverage collection;
- runtime mutation testing;
- Miri or sanitizer execution;
- large platform matrices;
- release readiness and semver checks;
- expensive dependency, workflow, or security sweeps.

A routed lane should state why it ran, what artifact it produced, and what it
does not prove.

## Claim boundary

CI should not imply more than it measured. A passing source-exception ledger is
not a type-aware proof. Coverage is execution-surface telemetry, not proof of
correctness. Runtime mutation is a backstop for test adequacy, not a replacement
for review. Miri is a concrete UB execution witness, not a universal memory
safety proof.
