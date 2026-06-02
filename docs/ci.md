# CI

cargo-allow has two different CI jobs:

- PR CI should run `cargo-allow diff --base <base>` so reviewers can see how a
  pull request changes source-tree exception posture.
- Mainline CI should run `cargo-allow check --mode no-new` so the committed
  policy remains a passing source-tree ledger.

The example workflows are intentionally small and copyable:

- [cargo-allow-diff.yml](../examples/github-actions/cargo-allow-diff.yml)
- [cargo-allow-check.yml](../examples/github-actions/cargo-allow-check.yml)

The examples install and run the standalone `cargo-allow` binary before
scanning. They pin the published crates.io release by default:

```bash
cargo install cargo-allow --version 0.1.4 --locked
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

## Pull Requests

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
synthetic code-scanning results.

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
