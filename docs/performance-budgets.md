# Performance Budgets

cargo-allow's value depends on the edit → check → fix loop being fast enough
that it does not materially slow ordinary repository work. These budgets make
"fast enough" a measured, tracked quantity — not a subjective claim.

## Measurement methodology

`scripts/perf-budget-smoke.sh` measures end-to-end wall-clock elapsed time for
the critical operator-loop commands against the cargo-allow repository itself.
It runs against a debug build locally and a release build in the hosted Linux
`operator-latency` CI job.

The receipt (`target/perf-budget/operator-latency.receipt.json`) records the
tested binary digest and profile, host/toolchain, repository fixture counts,
ordered argv, per-sample elapsed milliseconds, artifact digests, and semantic
result checks. The receipt follows
[`cargo-allow.operator-latency.v1`](schemas/operator-latency.schema.json), a
supporting harness contract rather than a governed cargo-allow command
artifact. The generated command artifacts are uploaded with the receipt in
CI.

Run the local smoke with the default debug profile, or select release to match
the hosted profile:

```bash
bash scripts/perf-budget-smoke.sh
PROFILE=release bash scripts/perf-budget-smoke.sh
```

Each measured command must remain at or below the conservative 60,000 ms
catastrophic-regression ceiling. Advisory product targets below are tracked
separately and are not asserted by this harness.

## Initial baseline (2026-07-18, Windows debug build)

These are the first measured numbers — the starting point, not the target.
The hosted receipt is the comparable Linux release observation; it is not a
universal hardware baseline.

| Command | Debug (Windows) | Notes |
| --- | ---: | --- |
| `audit` (full scan) | ~22,600 ms | Full tree-sitter parse + classify + evaluate |
| `check --mode no-new` | ~17,500 ms | Same scan, no-new gate |
| `why` (single-file fast path) | ~240 ms | One-file scan (#2425) |
| `diff --base HEAD~1` | receipt | Requires two revisions; see hosted artifact |
| `audit` (warm repeat) | receipt | After process/filesystem cache warm |

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
