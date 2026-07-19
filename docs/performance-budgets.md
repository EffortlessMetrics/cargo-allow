# Performance Budgets

cargo-allow's value depends on the edit → check → fix loop being fast enough
that it does not materially slow ordinary repository work. These budgets make
"fast enough" a measured, tracked quantity — not a subjective claim.

## Measurement methodology

`scripts/perf-budget-smoke.sh` measures wall-clock elapsed time for the
critical operator-loop commands against the cargo-allow repository itself
(~1000 tracked files, ~320 policy entries). It runs against a debug build
locally and a release build in CI.

The receipt (`target/perf-budget/perf-budget.receipt.txt`) records each
command's elapsed milliseconds, the host, and a timestamp so measurements
can be tracked over time.

## Initial baseline (2026-07-18, Windows debug build)

These are the first measured numbers — the starting point, not the target.
CI (Linux, release build) will be significantly faster.

| Command | Debug (Windows) | Notes |
| --- | ---: | --- |
| `audit` (full scan) | ~22,600 ms | Full tree-sitter parse + classify + evaluate |
| `check --mode no-new` | ~17,500 ms | Same scan, no-new gate |
| `why` (single-file fast path) | ~240 ms | One-file scan (#2425) |
| `diff --base HEAD~1` | TBD | Requires two revisions |
| `audit` (warm repeat) | TBD | After filesystem cache warm |

## Budget targets (0.2.0)

To be set after CI measurements on Linux with release builds. The conceptual
gate from the design docs:

> A narrow source edit must not trigger an expensive full-product ceremony.

Initial targets (subject to revision after CI baseline):

| Budget | Target | Rationale |
| --- | ---: | --- |
| `why` on one finding | < 500 ms | The fast path already meets this |
| `check --mode no-new` (this repo) | < 5,000 ms | CI gate must be fast |
| One-file incremental | < 2,000 ms | Edit → re-check loop |

## What drives the cost

The `audit`/`check` cost is dominated by:
1. Full `git ls-files` inventory walk
2. Tree-sitter parse of every `.rs` file
3. File classification of every non-Rust file
4. Findings × entries matching (O(n×m))

The `why` fast path (#2425) eliminates items 1-3 for one-file questions.
Future optimization targets: bucket policy entries by kind/family/path before
matching, compile globs once, and add bounded parallelism for independent file
parses.
