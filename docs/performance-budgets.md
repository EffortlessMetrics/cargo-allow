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
ordered argv, per-sample elapsed milliseconds and payload byte sizes, artifact
digests, and semantic result checks. The receipt follows
[`cargo-allow.operator-latency.v3`](schemas/operator-latency.v3.schema.json), a
supporting harness contract rather than a governed cargo-allow command
artifact. The generated command artifacts are uploaded with the receipt in
CI. Per-sample payload byte sizes and the #4366 agentic-surface samples
(`agent_loop` phase: the four-command repair composite plus paired `hooks
run` overhead) are recorded under the same contract.

Run the local smoke with the default debug profile, or select release to match
the hosted profile:

```bash
bash scripts/perf-budget-smoke.sh
PROFILE=release bash scripts/perf-budget-smoke.sh
```

For an already-built executable, `CARGO_ALLOW_BIN` skips the build. Relative
overrides resolve against the repository root before the harness enters its
fixture clone; absolute paths are also accepted.

```bash
PROFILE=release CARGO_ALLOW_BIN=target/release/cargo-allow bash scripts/perf-budget-smoke.sh
```

Each measured command must remain at or below the conservative 60,000 ms
catastrophic-regression ceiling. Advisory product targets below are tracked
separately and are not asserted by this harness. Per-sample payload byte
sizes are recorded in the receipt, and catastrophic payload ceilings for the
emitted machine artifacts are asserted by the harness; the advisory
agent-read target (≤ 65,536 B per machine read, ~16k tokens) is tracked
separately and is not asserted.

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

## Agentic-surface budgets (#4366)

The agentic loop — route a work item, diagnose it, receipt it, re-run the
gate — is the loop agents run dozens of times per session. These budgets
guard its end-to-end cost at the self-hosted ledger's realistic denominator
(~2,575 tracked files, ~1,049 policy entries, ~16,170 findings, ~158
worklist items). The harness measures every surface below in its `agent_loop`
phase against a disposable full-scale clone with a fresh unreceipted probe
finding, and records `payload_bytes` for every sample.

### Baseline caveat

Measured baselines below are from #4366's design session (2026-10-04,
Windows debug host, loaded machine, pair `f4321d19`; per-surface min-of-6
across two passes, composite min-of-3). They are **indicative debug-host
observations, not universal hardware claims**. The hosted Linux release
receipt is the comparable observation and re-baselines these numbers per the
revision clause above; until then the catastrophic ceilings (60,000 ms
latency; payload ceilings in the table below) are the only harness-enforced
gates, and every advisory target here is non-blocking.

### Per-surface latency (advisory targets; catastrophic ceiling stays 60,000 ms)

| Surface | Advisory target | Measured min (2026-10-04, Windows debug) | Basis |
| --- | ---: | ---: | --- |
| `worklist --format json` (self-hosted scale) | < 8,000 ms | 3,026 ms | release CI well under |
| `why --plan` on one new finding | < 2,000 ms | 1,007 ms | plan re-scans the tree |
| `add --from-plan --update` | < 12,000 ms | 9,146 ms | full re-scan + full-result validation + atomic replace |
| `check --mode no-new` | < 5,000 ms (existing target) | 8,826 ms | loaded-debug overshoot; re-baseline per #4339 |
| `audit --format json` | < 8,000 ms | 2,891 ms | full scan + report |
| `hooks run` wrapper overhead | ≤ 2,000 ms over a paired bare `check` | within host noise | paired-sample budget keeps it bounded |

### Agent-loop composite (advisory target)

`worklist` → `why --plan` → `add --from-plan --update` → `check --mode
no-new`: **≤ 30,000 ms end-to-end** for one receipted repair at self-hosted
scale (measured min 28,627 ms on the loaded debug host; per-step rows plus a
derived composite total are recorded in the receipt so a regression in any
step is attributable).

### Payload budgets

An agent pays token cost on every machine read; the advisory agent-read
target is ≤ 65,536 B (64 KiB ≈ ~16k tokens) per machine read. Catastrophic
payload ceilings are harness-asserted; advisory targets are not.

| Machine artifact | Catastrophic ceiling (asserted) | Advisory agent-read target | Measured (2026-10-04, Windows debug) |
| --- | ---: | ---: | ---: |
| `worklist.json` (~159 items) | 524,288 B | 65,536 B (exceeded ~4.4×; needs paging/`--limit` or summary-first guidance) | 290,023 B |
| `audit.json` (~16,170 findings) | 8,388,608 B | not agent-readable today; read `--command-summary-output` instead | 6,919,136 B |
| `check` receipt | 16,384 B | 65,536 B | 9,520 B |
| `why.json` (fast path, matched) | 8,192 B | 65,536 B | 3,165 B |
| `why --plan` plan artifact | 12,288 B | 65,536 B | 7,328 B |
| `add` summary JSON | 4,096 B | 65,536 B | 2,448 B |
| `--command-summary-output` (any supported command) | 4,096 B | ≤ 4,096 B (already compliant) | 2,099–2,901 B |

The bounded `--command-summary-output` projection (2.1–2.9 KB) is the cheap
agent read that already exists; agents should prefer it over full
`worklist.json`/`audit.json` reads. Worklist paging/`--limit` is a separate
UX slice and is intentionally not part of #4366.

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

## Persistent scan facts (#2571)

`check` persists parsed Rust scan facts under
`target/cargo-allow/cache/`, keyed by the SHA-256 of the exact text the
scanner evaluated. A warm invocation skips the tree-sitter parse for
unchanged files; invalidation is content-exact, so preserved mtimes cannot
mask changed content and mtime churn alone never changes results. Entries
are bound to a scanner generation (crate version plus scanner semantic epoch);
an accidentally corrupted or truncated store fails open to an ordinary cold
scan, and skipped files are never persisted. The checksum detects corruption
and truncation; it is not an authenticity or anti-tamper mechanism.

Claim boundary: the cache is trusted-local *performance state*, not authority.
Every durable entry is re-validated against the current file digest before
use; failed validation or decode falls back to ordinary scanning. The
checksum detects accidental corruption and truncation but is not an
authenticity mechanism. It lives under `target/`, is never part of the
source-tree inventory, and every failure path degrades to ordinary scanning.
