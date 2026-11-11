# CI

cargo-allow has two different CI jobs:

- PR CI should run `cargo-allow diff --base <base>` so reviewers can see how a
  pull request changes source-tree exception posture.
- Mainline CI should run `cargo-allow check --mode no-new` so the committed
  policy remains a passing source-tree ledger.

The example workflows are intentionally small and copyable:

- [cargo-allow-diff.yml](../examples/github-actions/cargo-allow-diff.yml)
- [cargo-allow-check.yml](../examples/github-actions/cargo-allow-check.yml)

Primary copy-paste how-to: [Run in CI](how-to/run-in-ci.md).
Troubleshooting: [Troubleshoot cargo-allow](how-to/troubleshoot-cargo-allow.md).
Rollback: [Rollback cargo-allow adoption](how-to/rollback-cargo-allow-adoption.md).

The examples install and run the standalone `cargo-allow` binary before
scanning. They pin the published crates.io release by default:

```bash
cargo install cargo-allow --version 0.1.11 --locked
```

If you are testing an unreleased branch, replace only the install step with a
Git source such as:

```bash
cargo install --git https://github.com/EffortlessMetrics/cargo-allow cargo-allow --locked
```

`cargo allow ...` remains optional Cargo external subcommand compatibility.

The scan itself is source-tree only. It does not invoke Cargo metadata, Cargo
commands, rustc, Clippy, build scripts, proc macros, external evidence tools,
or repository code. The install step fetches the `cargo-allow` tool; the policy
scan should remain usable even when the checked-out repository does not build.

## Repository CI

cargo-allow's own GitHub Actions workflow is split into product-scoped lanes
(#3358) so one product's experimental failure cannot block an unrelated
cargo-allow patch or release lane. The checked authority for the topology and
gating posture is [ci-lanes.toml](ci-lanes.toml), validated against the
workflow by `ci_lane_topology_tests.rs` (including the seeded-failure proof:
no required lane depends on, shares a job with, or references an experimental
product except through a declared cross-product reference).

Lane applicability on a pull request is `scripts/ci-proof-route.py`
(#2569/#4332), not a workflow `paths:` filter and not a second registry
beside feature-configuration row selection or release-impact. A successor
diff that is only `.allow/review-dispositions/` records narrows the heavy
lanes that are not ruleset contexts only when the effective diff is
source-bearing or shared impact and the predecessor's proof is exact,
terminal, and admissible for the same source paths and base. That
successor route is the one that reuses predecessor proof. An effective
diff that is entirely `.allow/review-dispositions/` records, with no
other paths, narrows those lanes on its own and does not claim that
missing source proof is reusable. A source-bearing successor does not
use that route. A successor that reverts a source change remains
source-bearing, so the plan stays wide even when the effective diff
contains only those ledger records. Missing, stale,
wrong-source, failed, pending, cancelled, skipped, ambiguous, or
base-moved proof stays wide and is not reused. An effective diff that is
provably docs-only also records those lanes NotApplicable. Paths under
`docs/schemas/` and any `*.schema.json` are source, so an
operator-latency schema change keeps that lane. Markdown is the only
prose under `docs/` that narrows. The feature-configuration matrix,
`docs/ci-lanes.toml`, `docs/support-matrix.toml`, receipt JSON, and any
other non-markdown path under `docs/` stay source, so those changes
keep `feature-configuration-proof` and the other heavy lanes. An
unrecognized docs payload is source, not prose. GitHub skips a heavy lane only
when the plan succeeds with `narrow=true`. A `test_only` plan does not
set that flag: it records `product-candidates-interop`,
`operator-latency`, `integrated-dogfood`, and `compat-delegation`
NotApplicable, and it still runs `msrv`, `test-core-platforms`, and
`coverage`. `package-smoke` is NotApplicable only for `docs_only` and
`test_only`. A skip is not a green
execution. Unknown, malformed, and shared ledger impact stay wide. A
test path mixed with any other kind stays wide. Ordinary crate tests under
`crates/<crate>/tests/` are test-only.
`crates/cargo-allow/tests/intent_staged_precommit_delegate.rs` stays source,
because only `compat-delegation` runs that test with a candidate.
A `fixtures` path segment outside `docs/` is test-only except
`tests/compat/fixtures/`, which stays source for the same lane, and except
fixture paths that a test_only-skipped lane reads. `package-smoke` and the
four test_only product lanes contribute those paths from their steps, plus
one callee hop. `docs/dogfood/fixtures/` is already source or docs and is
not part of that set. An unreadable or unparsed job map keeps every fixture
path source.
A `scripts/test-*` file is test-only only when the ci.yml job map shows
that no test_only-skipped lane runs it, or an explicit decision records
that fact. `package-smoke` and the four test_only product lanes contribute
their `scripts/test-*` steps, plus one callee hop, to the source set.
Every `scripts/test-exact-candidate-*` file stays source.
A new `scripts/test-*` file with neither a job-map hit nor an explicit
decision stays source, and `scripts/test-ci-proof-route.py` fails until
that decision is recorded. There is no bare `scripts/test-*` prefix. A
docs-only last commit on a source-bearing pull request stays wide.
`push`, `workflow_dispatch`, `merge_group`, and release events stay wide.
The ruleset contexts are `pregate`, `test`, `review-readiness`, and
`frozen-subject-lock`. Proof routing never skips `pregate` or `test`.
`pregate` runs its current assertions, including the no-new guard, on
every workflow run. The change-note guard (`--require-change-note`)
belongs to `test` and runs only when `test` executes. `test` also runs
no-new when it executes. `test` needs `pregate`, `package-smoke`, and
`proof-route`. A failed `pregate` or a failed `package-smoke` skips
`test`, and the change-note guard with it.
That skip is a dependency failure, not a green execution and not a
narrow plan. A skipped `package-smoke` does not skip `test`. A successful `docs_only`
plan skips the heavy steps inside `test`. The inventory guard, fmt,
audit, no-new, and the change-note guard still run. `docs_only`
and `test_only` leave the cutover receipt restore and status unrun,
because those receipts are produced only by `package-smoke`. The
change-note guard stays in `test` and runs whenever `test` runs. A
pregate failure remains a failure. `cargo-deny` and the release-impact
graph stay.
`review-readiness` and `frozen-subject-lock` are separate workflows and
are not path-filtered. `cargo-deny`, `proof-route`, and
the release graph stay on a narrow plan and are not ruleset contexts.
`package-smoke` stays for a ledger narrow plan and for every wide plan.
It does not run for `docs_only` or `test_only`.

Measurement receipts in `docs/ci/receipts/` are admitted one file at a time.
Each has an exact-path `non_rust_file` allow in `policy/allow.toml`.
`allow-11245` names
`docs/ci/receipts/direct-floor-proof-cargo-allow-v1.json`, and the selector
glob is that same path. There is no directory glob, so a new receipt needs
its own allow. That addition is an `added_allow` change of severity Review,
so the change-note guard also requires a revision note. Receipt JSON stays
source, so a receipt pull request stays wide. Issue #4332 accepts a receipt
or a run record for the docs-only measurement. A docs-only pull request's
own narrow run is admissible evidence for that record.

### Measured lane reduction

One docs-only pull request was measured before `#4771` landed.
[PR #4770](https://github.com/EffortlessMetrics/cargo-allow/pull/4770)
head `6b33aa12b72922e782164be863b4499e68783f01`,
[ci.yml run 42980600013](https://github.com/EffortlessMetrics/cargo-allow/actions/runs/42980600013)
(`pull_request`, 2026-11-11T07:35:25Z through 2026-11-11T08:14:38Z).
The hosted plan class was `docs_only` and `narrow` was true.
The effective path was `docs/ci.md`. Seventeen lane rows were
NotApplicable, and GitHub skipped those jobs: `msrv`,
`product-candidates-interop`, `operator-latency`, `integrated-dogfood`,
`test-core-platforms`, `compat-delegation`, `test-intent-experimental`,
`test-proof-experimental`, `test-shared-protocol`, `coverage`,
`workflow-syntax`, `workflow-security`, `workflow-construction`,
`dependency-graph-delta`, `collect-performance-fixtures`,
`shallow-diff-smoke`, and `feature-configuration-proof`.
`package-smoke` was still `execute` on that plan, with reason
"test restores this lane's cutover receipts", because the `#4771`
skip had not landed.

| Job | Started (UTC) | Finished (UTC) | Duration |
| --- | --- | --- | --- |
| `package-smoke` | 2026-11-11T07:37:51Z | 2026-11-11T07:55:13Z | 17m22s |
| `test` | 2026-11-11T07:55:15Z | 2026-11-11T08:14:38Z | 19m23s |

`#4771` landed as `b815604f806b39fba39fbf1215cfffd93e274aa9`.
`package-smoke` is NotApplicable for `docs_only` and `test_only`.
`test` runs when `package-smoke` succeeds or is skipped. The cutover
receipt restore and status steps run only when `package-smoke`
succeeds. `msrv`, the four `test-core-platforms` matrix rows, and
`coverage` still run for `test_only`, because that class does not set
`narrow=true`. Unknown, malformed, and non-`pull_request` plans stay
wide.

After `#4771`, a successful `docs_only` plan (`narrow=true`) runs
`pregate`, `test`, `proof-route`, `cargo-deny`, and the release graph
(`release-impact`, `release-rehearsal`, `release-rehearsal-required`).
`review-readiness` and `frozen-subject-lock` stay in their own
workflows. Heavy lanes skip only when `proof-route` succeeds with
`narrow=true`. `package-smoke` is NotApplicable when that plan's class
is `docs_only` or `test_only`. A failed or missing plan stays wide.

This is one observed run, not a distribution. The live docs-only
demonstration is this pull request's own run record. The orchestrator
records that record on issue #4332.

Required lanes (gate cargo-allow patch and release on wide pull-request and
full-boundary runs; `test` and `cargo-deny` also stay on a narrow plan, and
`package-smoke` stays unless the class is `docs_only` or `test_only`):

- **`msrv`**: cargo-allow release-set `cargo check --all-targets` on the
  declared `rust-version` (`1.95`). cargo-allow-scoped by policy — cargo-intent
  and cargo-proof claim no toolchain yet.
- **`package-smoke`**: candidate packaging, release-binary contract, exact
  candidate package set, cutover build receipts, install journey.
- **`test`**: rustfmt (workspace-wide, repo hygiene), release-set Clippy,
  tests, doc tests, docs, audit, no-new check, cutover status chain.
- **`feature-configuration-proof`**: routed feature-configuration matrix
  proof (#3905). Derives the affected configuration rows from the changed
  files — wide by default — via `scripts/feature-config-routing.py` reading
  the checked-in projection
  `docs/assets/feature-configuration-matrix-v1.json`, then proves exactly
  those rows through `allow-report`'s `#[ignore]`d recursive cargo proof
  suite; docs-only changes select no rows. The weekly scheduled
  **`feature-configuration-qualification`** workflow is the full selected
  union backstop.
- **`test-core-platforms`**: complete release-set suite on Linux, Windows,
  Apple Silicon macOS, and Intel macOS, including allow-rust's persistent
  scan-cache tests (#3915 PR C); a contract test drift-guards the rows
  against reintroducing platform-wide cache exclusions.
- **`compat-delegation`**: the staged precommit delegation e2e
  (#2601-B) — Linux against the exact installed cargo-intent
  candidate, Windows against a fresh build. Core lanes carry no
  cargo-intent edge; this lane owns the compatibility contract
  (#3369).
- **`coverage`**: release-set Tarpaulin coverage.
- **`shallow-diff-smoke`**, **`operator-latency`**, **`cargo-deny`**:
  cargo-allow-specific lanes and the workspace supply-chain audit.

Experimental and integrated lanes (never gate cargo-allow):

- **`test-intent-experimental`** / **`test-proof-experimental`**: each
  product's Clippy, tests, and doc tests on stable.
- **`test-shared-protocol`**: the namespace-rail shared crate
  (`effortless-rust-source-index`) outside the cargo-allow release set.
- **`product-candidates-interop`**: intent and proof candidate smokes plus
  the three-product interop matrix.
- **`integrated-dogfood`**: governance validation, three-product dogfood,
  and the simplification audit — where cross-product contracts change.

## Pull Requests

The PR example checks out with `fetch-depth: 0` so `origin/<base>` is available.
Shallow checkouts often make `diff --base` fail closed; do not silently
substitute `HEAD` or an empty comparison. See [Run in CI](how-to/run-in-ci.md).

Use the diff workflow for pull requests:

```bash
cargo-allow diff \
  --base origin/main \
  --format markdown \
  --output target/cargo-allow/pr-summary.md
```

For focused review lanes, add `--kind <kind>`. That narrows source finding
changes and allow-entry policy changes to the selected governed kind while
still preserving ledger-level policy contract signals.

If CI passes an explicit `--head <rev>`, cargo-allow reads policy and source
posture from the compared git revisions rather than from the working tree.
Default policy paths are discovered from the head revision first, then from
the base revision, so current PR posture stays tied to the head snapshot if the
policy file moved. A relative `--config` is treated as a source-tree path in
the compared revisions and fails closed if neither side contains it.

The Markdown output starts with a PR Summary section. That section reports:

- net posture: `unchanged`, `improved`, `review-required`, or `worse`;
- current check failures, including no-new and broken local evidence signals;
- new and removed source findings;
- policy failures, policy review items, and policy improvements;
- the reviewer action implied by those signals.

This is reviewer guidance for source-syntax and policy-ledger posture. It does
not claim macro expansion, type information, build awareness, proof adequacy, or
coverage.

## Mainline

Use the check workflow on `main`:

```bash
cargo-allow audit \
  --format json \
  --output target/cargo-allow/audit.json

cargo-allow audit \
  --format html \
  --output target/cargo-allow/audit.html

cargo-allow check \
  --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md

cargo-allow check \
  --mode no-new \
  --format sarif \
  --output target/cargo-allow/check.sarif
```

The JSON audit is useful for machines and future trend reporting. The HTML
audit is a static human-readable artifact for maintainers and auditors. The
receipt is the durable CI claim for the current source exception ledger. SARIF
output contains non-matched source-tree outcomes for code-scanning surfaces; it
does not include proof-tool results or build-derived findings. SARIF run
properties may include advisory policy/evidence-health counts such as
`policy_missing_evidence`, `broken_evidence_links`, and
`weak_evidence_references`, but those counts are run context rather than
synthetic code-scanning results. When repair work exists, SARIF run properties
may also include an `evidence_repair_queues` array with the matching
`cargo-allow worklist ... --format json` commands.
Finding-backed SARIF results also include `partialFingerprints` derived from
the normalized snippet, normalized path, source line, and structural finding
identity so code-scanning consumers can deduplicate stable findings while still
detecting movement. Each run includes a stable `automationDetails.id` for the
command and an `invocations` record with execution status and UTC start/end
timestamps. These fingerprints identify scanner observations; they do not
prove that an unsafe construct or policy exception is correct.

For cargo-allow's own repository, CI also emits opt-in spec-system profile
artifacts:

```bash
cargo test --doc --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

These docs gates are part of cargo-allow's self-hosting proof surface. They
verify crate documentation and rustdoc warnings, but they do not broaden the
source-tree scan or make cargo-allow execute proof providers.

```bash
cargo-allow check \
  --profile spec-system \
  --mode audit \
  --format json \
  --output target/cargo-allow/spec-system.json

cargo-allow check \
  --profile spec-system \
  --mode audit \
  --format markdown \
  --output target/cargo-allow/spec-system.md
```

This is dogfood for the repo's source-of-truth graph. The profile currently
runs in blocking posture for selected structural findings in this repo. That
does not make spec-system validation part of default cargo-allow behavior, and
it does not execute proof commands.

For adoption details, see
[Run the spec-system profile in CI](how-to/run-spec-system-in-ci.md).

## Release

Tag pushes matching `v*` trigger the [Release
workflow](../.github/workflows/release.yml). The workflow runs preflight checks,
processes and verifies the topology-derived thirteen-row cargo-allow candidate,
and creates a GitHub Release from `docs/release/github/vX.Y.Z.md` when that file
exists. The release identity — including the GitHub prerelease posture — comes
from the typed `cargo-allow release-identity` projection validated in the
preflight job, so an `-rc.N` tag publishes as a prerelease and a stable tag
publishes as a full release; no workflow step infers the channel itself.
Before any cargo-allow upload, the workflow derives the
three selected shared rows from the V2 topology and enforces a shared-first,
commit/tree/topology-bound read-only registry preflight proving
`AlreadyPublishedExact` and checksum equality. A missing or malformed result
fails closed. With that precondition
satisfied, the expected missing uploads are the ten cargo-allow-family rows. The separate
[authorized namespace workflow](../.github/workflows/release-authorized.yml)
publishes twelve `0.1.0` namespace rows and deliberately does not trigger the
cargo-allow tag rail.

See [Release Operations](release/README.md) for Trusted Publishing setup, manual
dry-run via workflow dispatch, and the manual publish fallback.

Copy-paste shape for an optional profile artifact:

```yaml
- name: cargo-allow spec-system artifact
  run: |
    cargo-allow check \
      --profile spec-system \
      --mode audit \
      --format json \
      --output target/cargo-allow/spec-system.json
    cargo-allow worklist \
      --profile spec-system \
      --format json \
      --output target/cargo-allow/spec-system-worklist.json

- uses: actions/upload-artifact@v7.0.1
  if: always()
  with:
    name: cargo-allow
    path: target/cargo-allow/
```

## Artifacts

Upload `target/cargo-allow/` even on failure. The report and receipt explain
which exception changed, whether the change was unmatched or stale, and the
claim boundary for the command.

Broken local evidence links should be treated as failing gate signals, not as
missing artifacts. `cargo-allow check` fails closed when retained policy points
to missing, symlinked, directory, or out-of-tree local evidence. Reporting
commands such as `audit`, `diff`, `list`, `explain`, `worklist`, `propose`, and
`prune --stale` dry-run can still emit artifacts that identify the broken links
so maintainers can repair them in the next PR.
