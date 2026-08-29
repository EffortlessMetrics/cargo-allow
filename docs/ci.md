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

Required lanes (gate cargo-allow patch and release):

- **`msrv`**: cargo-allow release-set `cargo check --all-targets` on the
  declared `rust-version` (`1.95`). cargo-allow-scoped by policy — cargo-intent
  and cargo-proof claim no toolchain yet.
- **`package-smoke`**: candidate packaging, release-binary contract, exact
  candidate package set, cutover build receipts, install journey.
- **`test`**: rustfmt (workspace-wide, repo hygiene), release-set Clippy,
  tests, doc tests, docs, audit, no-new check, cutover status chain.
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
exists. Before any cargo-allow upload, the workflow derives the
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
