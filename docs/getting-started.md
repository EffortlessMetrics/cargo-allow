# Getting Started

This tutorial is the executable first-hour journey: choose a product channel,
verify install prerequisites, run `doctor` and `audit`, then take **one**
bootstrap path (`init` **or** `propose`), gate with `check --mode no-new`, and
hand off to list / explain / why / worklist.

`cargo-allow` scans repository files without executing project code. The steps
below do not require Cargo metadata, compilation, rustc, Clippy, build scripts,
proc macro expansion, or proof-tool execution against the **target** repository.

Executable proof for this journey lives in
`crates/cargo-allow/tests/first_hour_adoption.rs` and the checked step inventory
at [`docs/dogfood/fixtures/getting-started/step-inventory.toml`](dogfood/fixtures/getting-started/step-inventory.toml).
Expected-output markers are validated against the just-built binary in that
same test (they are not hand-copied from stale receipts).

The recurring policy and selector terms in this guide are defined in the
[cargo-allow glossary](glossary.md).

## 0. Choose a product channel

| Channel | How you invoke cargo-allow | Commands this guide treats as ordinary |
| --- | --- | --- |
| **Published** `0.1.11` | `cargo install cargo-allow --version 0.1.11 --locked` then `cargo-allow …` | `doctor`, `audit`, `init`, `propose`, `check`, `list`, `explain`, `why`, `worklist`, `diff`, `add`, `refresh`, `prune` |
| **Source candidate** (this checkout / current `main`) | `cargo run -p cargo-allow -- …` | Published set plus any later explicitly labeled unreleased surfaces |

Mechanical published-vs-candidate command registry enforcement uses the offline
snapshot
[`published-command-registry.toml`](dogfood/fixtures/getting-started/published-command-registry.toml)
(`cargo-allow.published-quick-start.v1`). Do not copy a source-candidate-only
command into a Published install path. Installed-binary execution of these
snippets remains [#2278](https://github.com/EffortlessMetrics/cargo-allow/issues/2278).

## 1. Install prerequisites

For the `0.1.x` line:

- **Rust 1.95 or newer** (workspace `rust-version`) to build or install
  cargo-allow itself.
- Installing from crates.io or from source may need a **C toolchain** because
  cargo-allow's own parser dependencies compile native code.
- Scanning a target repository still does **not** build or execute that
  repository. Do not conflate cargo-allow's build prerequisites with its
  no-build scan claim.

Published install:

```bash
cargo install cargo-allow --version 0.1.11 --locked
```

Source-candidate invoke (from this repository):

```bash
cargo run -p cargo-allow -- doctor
```

### 2a. Get one current adoption recommendation (source candidate)

The source-candidate binary can project the bounded adoption plan before the
first policy decision. It is read-only and does not execute the recommended
command:

```bash
cargo run -p cargo-allow -- adopt
cargo run -p cargo-allow -- adopt --format json --output target/cargo-allow/adoption-plan.json
```

Human output leads with `Repository state:`, one `Recommended next step:`, a
copyable `Run:` command, an explicit `Writes:` line, and a rollback statement.
The JSON artifact is `cargo-allow.core-adoption-plan.v1`; it is derived from
the same plan and is portable across checkout paths. Published `0.1.11` does
not expose `adopt` yet, so keep this command on the Source-candidate path.

Do not copy release-candidate crate versions into install commands until they
are published.

## 2. Check setup (`doctor`)

Run from the repository root (or pass `--root`):

```bash
cargo-allow doctor
```

Healthy output with **no policy yet** includes markers such as:

```text
config: not found; run `cargo-allow init --root "…"`
  tip: `cargo-allow audit` works without a policy to surface source findings before bootstrapping
inventory: source_tree/source_syntax
Claim boundary: scanned source-tree/source syntax only
```

JSON identity (stable for scripts and the first-hour test):

```text
"schema_id": "cargo-allow.doctor.v1"
"command": "doctor"
```

`doctor` reports the source-tree root, inventory mode, policy path, scanner
limitations, and local evidence-health diagnostics. It does not build the
project.

## 2b. Inspect scanner capabilities

Before choosing a policy or interpreting a finding, inspect the capability
matrix that the current source-candidate binary carries:

```bash
cargo run -p cargo-allow -- capabilities
cargo run -p cargo-allow -- capabilities --format json
cargo run -p cargo-allow -- capabilities --class not-included
cargo run -p cargo-allow -- capabilities --root . --config policy/allow.toml --format json
```

This is a current source-candidate command; the published `0.1.11` binary
predates this matrix and does not expose it yet.

The JSON form is `cargo-allow.sensor-capabilities.v1` and lists the owning
module, selected input, analysis class, completeness model, limitations,
supported claims, excluded claims, fixtures, and documentation anchors for
each built-in finding family. `not_included` rows are explicit exclusions, not
clean-scan results. This command describes source-tree observations; it does
not add compilation, type, macro-expansion, MIR, runtime, or test-adequacy
analysis.

When a repository defines custom file-family rules in its policy, provide the
source-tree root and policy path to include them in the JSON projection. These
rows appear under `configured_file_families` and identify the rule id, family,
glob, and configured path-presence support. They do not claim file-content
safety or any compilation, type, flow, runtime, or test-adequacy behavior. If
no root or policy is supplied, the command reports the static catalog only;
when a supplied policy is invalid, the command fails instead of presenting a
partial configured catalog.

## 3. Audit current exceptions

```bash
cargo-allow audit
```

`audit` is advisory. It surfaces syntax-visible exception surfaces (unsafe,
panic-family, indexing/slicing, lint suppressions, non-Rust tracked files,
stale/expired policy rows, broad selectors, `baseline_debt`, and evidence-health
issues) without failing the process for unreceipted findings alone.

### Expected audit output

The full report contains additional inventory and claim-boundary fields. These
stable JSON excerpts show the markers to look for when checking the first run:

Clean audit:

```json
{
  "command": "audit",
  "status": "passed",
  "summary": { "findings": 0, "new": 0 }
}
```

Audit with one unreceipted finding:

```json
{
  "command": "audit",
  "status": "passed",
  "summary": { "new": 1 }
}
```

An advisory audit can pass while reporting findings. Choose a bootstrap path
only after reviewing what the audit found; do not treat `status: "passed"` as
approval of an unreceipted exception.

### Choose a report format

`audit`, `check`, and `diff` support these report formats:

| Format | Use it for |
| --- | --- |
| `human` (default) | Readable terminal output. |
| `markdown` or `md` | Reviewer summaries and checked-in or uploaded reports. |
| `json` | Scripts, automation, and versioned receipts. |
| `html` | A static report for maintainers or auditors. |
| `sarif` | Code-scanning integrations; it contains source-tree outcomes, not build or proof-tool results. |

Write a non-terminal report to a file with `--output`:

```bash
cargo-allow audit --format html --output target/cargo-allow/audit.html
cargo-allow check --mode no-new --format sarif --output target/cargo-allow/check.sarif
```

Use the full `markdown` spelling in shared documentation; `md` is a supported
CLI alias for interactive use.

### Branch A — clean / zero findings

When the audit summary shows no findings (JSON markers including
`"command": "audit"`, `"findings": 0`, and `"new": 0`), **do not manufacture baseline debt**.
Wire CI later with `check --mode no-new` only after you deliberately create a
policy (`init` for a strict empty ledger, or keep scanning advisory until you
need a gate).

### Branch B — retained findings

Continue to step 4 and choose **one** bootstrap path. Fixture markers for one
unreceipted finding include `"command": "audit"`, `"new": 1`, and
`"status": "passed"`.

The claim is scoped to scanned source-tree inventory. It is not a proof that no
exception exists outside the syntax-visible surface cargo-allow scanned.

## 4. Choose ONE bootstrap path

```text
Choose ONE bootstrap path:
- init: small/strict repository (starter ledger, no generated debt)
- propose: existing repository with retained debt (preview, then reviewed write)
```

### `init` — small / strict repository

```bash
cargo-allow init --root .
```

Creates `policy/allow.toml` with workspace defaults and no generated debt.
Useful when the tree is clean or you want to receipt findings one-by-one with
`add` later.

Both bootstrap paths seed one entry: the ledger's receipt for itself. The
policy file is a tracked non-Rust file, so once it is committed it appears in
its own inventory; without that receipt the first `check --mode no-new` after
adoption would fail on `policy/allow.toml` rather than on your code. It is
recorded as `classification = "source_exception_policy"` with a `review_after`
date, not as `baseline_debt` — the ledger is a governance record, not retained
debt. It is not generated debt and does not expire.

### `propose` — existing debt (preview, then write)

Preview only (does not write):

```bash
cargo-allow propose
```

Persist a reviewed candidate:

```bash
cargo-allow propose --write policy/allow.toml
```

For automation, write the machine-readable summary separately from the policy
candidate:

```bash
cargo-allow propose \
  --summary-format json \
  --summary-output target/cargo-allow/propose.json
```

Omitting `--write` keeps the policy candidate in preview mode. The summary
output is a report for routing and review; it does not write or approve the
policy.

Generated entries use `classification = "baseline_debt"`. Treat that as a queue
for review, narrowing, evidence, or removal. Do not convert generated debt into
approval just to pass CI.

`check` needs a policy path (default discovery or `--config`). Running
`check --mode no-new` with no policy fails closed and tells you to `init` or
pass `--config`.

When `--config` is omitted, discovery starts at the requested source-tree root
and walks upward. At each directory it checks `Cargo.toml` source text first:
`[package.metadata.cargo-allow] config = "..."` wins over
`[workspace.metadata.cargo-allow] config = "..."` in the same manifest. The
metadata path must be a non-empty relative path without `..`. If no metadata
path is selected, cargo-allow retains its conventional order:
`policy/cargo-allow.toml`, `policy/allow.toml`, `.cargo/allow.toml`, then
`allow.toml`. This reads committed manifest text; it does not invoke Cargo
metadata or infer workspace membership. The complete source-exception,
spec-system profile, and federation precedence contract is in
[Configuration Discovery](source-exception-ledger.md#configuration-discovery).

## 5. Run the no-new check

Prerequisite: `policy/allow.toml` must exist from step 4's `init` or
`propose --write` path, unless you pass an explicit `--config` path.

```bash
cargo-allow check --mode no-new
```

Passing baseline (after `propose --write` or an empty `init` ledger with no new
debt) includes:

```text
Result: passed (enforcing)
```

JSON markers: `"command": "check"`, `"status": "passed"`, and summary `"new": 0`.

Failing after one new in-scope exception includes:

```text
new: unreceipted …
Result: failed
```

JSON marker: `"status": "failed"`.

A passing no-new check means no new unreceipted findings were found in scanned
source-tree inventory. It does not mean the project is safe, buildable,
type-checked, or free of all possible exceptions.

## 6. Understand and repair

When a check failure or audit finding needs a deliberate decision, follow the
issue-first lifecycle in [Manage an exception](how-to/manage-an-exception.md):

| Intent | Command | Preview vs write |
| --- | --- | --- |
| create a strict starter policy | `init` | writes `policy/allow.toml` (use only when chosen in step 4) |
| generate a reviewed baseline candidate | `propose` | omit `--write` to preview; `--write <path>` to persist |
| receipt one deliberate finding | `why --plan` then `add --from-plan --update` | see the plan-then-apply route below; `--write <new-path>` emits a candidate policy file for inspection and is not the live-ledger apply path |
| refresh drift for one selected ID | `refresh --allow-id <id>` | `--dry-run` preview; `--write` apply |
| remove selected stale entries | `prune` | `--dry-run` preview; `--write` apply |
| repair the source instead of policy | edit code, then rerun `check` | no policy mutation |

Published-channel diagnosis:

```bash
cargo-allow list
cargo-allow explain <allow-id>
cargo-allow worklist --format json
```

Published diagnosis (`why` is included in 0.1.11):

```bash
cargo-allow why --kind panic --path src/lib.rs --line 1
```

### Receipt one finding: source-candidate plan-then-apply

Source candidate only (current `main`). The plan flags below are not part of
the Published `0.1.11` surface; do not copy them into a Published install
path.

```bash
cargo run -p cargo-allow -- why \
  --kind panic --path src/lib.rs --line 1 \
  --plan target/cargo-allow/add-plan.json

cargo run -p cargo-allow -- add \
  --from-plan target/cargo-allow/add-plan.json \
  --update \
  --owner core \
  --reason "bounded fixture exception" \
  --evidence doc:docs/design.md \
  --summary-format json \
  --summary-output target/cargo-allow/add-application.json

cargo run -p cargo-allow -- why --kind panic --path src/lib.rs --line 1

cargo run -p cargo-allow -- check --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

The boundary between those four commands is the point:

- `why --plan` is read-only and `New`-only; it writes a plan, not policy.
- `add --from-plan --update` revalidates every plan binding against the live
  tree, then performs one atomic ledger write. If the tree moved since the
  plan was written it refuses and prints the exact regeneration command.
- the third command is a **targeted recheck** proving that one finding now
  reports `status: matched`. It is not a repository proof.
- the final `check --mode no-new` is the CI-grade repository proof.

Full lifecycle detail, including the expert `add --update` shortcut, is in
[Manage an exception](how-to/manage-an-exception.md).

## 7. Reset / uninstall

To undo a cargo-allow adoption, restore or delete `policy/allow.toml` with
Git, revert any adoption-specific CI changes, and remove generated
`target/cargo-allow/` reports if they are no longer needed. Remove optional
`.allow/` profile files only when cargo-allow installed them and nothing
else uses them.

To remove the installed binary from your machine:

`cargo uninstall cargo-allow`

The binary uninstall does not change repository policy or CI files. For the
ownership map and a safe order of operations, see [Rollback cargo-allow
adoption](how-to/rollback-cargo-allow-adoption.md).

## Terminology (first use)

- **`baseline_debt`**: generated adoption classification; a review queue, not
  approval.
- **selector / structural identity**: how a ledger row matches a syntax-visible
  finding (AST kind, path, fingerprints) rather than a fragile line-only guess.
- **`review_after` / `expires`**: live lifecycle thresholds. Once the current
  date reaches `review_after`, the entry is `review_due`; after an `expires`
  date passes, it is `expired`. These statuses surface in `list` / `worklist` /
  check summaries; they do not auto-extend or mutate the policy.
- **stale vs location drift**: unused or unmatched receipts versus matched
  findings whose `last_seen` location moved.
- **evidence reference**: `doc:`, `test:`, `issue:`, … pointers checked for
  presence or reported as traceability without running external tools.
- **occurrence headroom**: how many matching findings a receipt may still cover;
  prefer narrowing or source repair over silently widening.

More detail: [source-exception-ledger.md](source-exception-ledger.md) and the
[cargo-allow glossary](glossary.md).

## Policy entry shape

**Illustrative only** — the `allow-0042` / `crates/parser/src/span.rs` example
below is not a runnable path in this repository. For a live bootstrap that
creates a real allow ID, run the first-hour test fixture
(`crates/cargo-allow/tests/first_hour_adoption.rs`) or follow
[Manage an exception](how-to/manage-an-exception.md) against your own tree.

Always use forward slashes in `path` and `glob` values, even on Windows.

```toml
[[allow]]
id = "allow-0042"
kind = "panic"
family = "indexing_slicing"
path = "crates/parser/src/span.rs"
owner = "parser"
classification = "validated_span_invariant"
reason = "Parser validates TextRange before slicing."
created = "2026-06-01"
review_after = "2026-09-01"
evidence = [
  "doc:docs/safety/parser-spans.md",
  "test:parser_rejects_invalid_text_range",
]

[allow.selector]
ast_kind = "index_expr"
container = "slice_checked_text_range"
```

Local-file evidence such as `doc:`, `spec:`, `adr:`, `ripr:`,
`unsafe-review:`, and `coverage:` can be checked for presence. Traceability
references such as `test:`, `cargo:`, `issue:`, `pr:`, and `legacy-policy:` are
reported without running tools or contacting services.

## Next workflows

- Review a pull request: `cargo-allow diff --base origin/main`
- Generate agent work: `cargo-allow worklist --format json`
- Read claim boundaries: [claim-boundaries.md](claim-boundaries.md)
- Read the ledger model: [source-exception-ledger.md](source-exception-ledger.md)
- Channel synchronization follow-up: keep
  [`published-command-registry.toml`](dogfood/fixtures/getting-started/published-command-registry.toml)
  in sync when promoting the next crates.io release ([#2364](https://github.com/EffortlessMetrics/cargo-allow/issues/2364))
- Installed-package journey: [#2278](https://github.com/EffortlessMetrics/cargo-allow/issues/2278)

## Checked step inventory

Stable step IDs shared with
[`step-inventory.toml`](dogfood/fixtures/getting-started/step-inventory.toml)
and `crates/cargo-allow/tests/first_hour_adoption.rs` (consumable by #2278):

| Step ID | Stage |
| --- | --- |
| `channel_select` | Choose published vs source-candidate channel |
| `install_prereqs` | Rust 1.95+ / C toolchain for installing cargo-allow |
| `doctor_no_policy` | Healthy doctor with no policy yet |
| `adoption_plan` | Source-candidate read-only recommendation before `init` or `propose` |
| `audit_clean` | Clean audit; no manufactured baseline debt |
| `audit_with_finding` | Audit with retained findings → bootstrap choice |
| `bootstrap_init` | Strict `init` path |
| `bootstrap_propose_preview` | `propose` without `--write` |
| `bootstrap_propose_write` | `propose --write` baseline debt |
| `check_no_new_pass` | Passing baseline no-new check |
| `check_no_new_fail` | Failing no-new after new debt |
| `list_explain_worklist` | Published diagnosis commands |
| `why_published` | Published diagnosis with `cargo-allow why` |
| `receipt_plan` | Source-candidate `why --plan` (read-only plan artifact) |
| `receipt_apply` | Source-candidate `add --from-plan --update` (atomic write) |
| `receipt_targeted_recheck` | Source-candidate recheck of the one finding |
| `receipt_full_check` | Source-candidate full `check --mode no-new` proof |

## Per-product surfaces

This guide covers `cargo-allow`, the supported product. The optional
experimental products have their own independent documentation surfaces:

- [cargo-intent](products/cargo-intent/getting-started.md) — opt-in
  experimental intent and obligation compiler; not installed by default.
- [cargo-proof](products/cargo-proof/getting-started.md) — opt-in
  experimental exact-snapshot evidence orchestrator; not installed by
  default.

Their commands and artifacts are separate products with separate claim
boundaries; this guide does not describe them.
