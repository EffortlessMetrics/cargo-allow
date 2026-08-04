# Adopting cargo-allow in a new repository

> Maturity: `init`, `audit`, and `doctor` are Stable in published `0.1.11` and
> Stabilizing on current main. See the [command maturity table](../status/SUPPORT_TIERS.md#command-maturity).

This guide is the **repeatable rollout recipe** for adopting cargo-allow from
an empty or existing repository. It records the exact bootstrap, CI, and
no-new-debt enforcement path — the same path cargo-allow dogfoods on itself.

## Prerequisites

- Rust toolchain (MSRV 1.95+)
- C toolchain (GCC/Clang on Unix-like systems or MSVC on Windows) to compile
  cargo-allow's tree-sitter parser dependency
- `cargo-allow` installed: `cargo install --git https://github.com/EffortlessMetrics/cargo-allow`
- A git repository (cargo-allow scans git-tracked files)

## Optional pre-commit integration

cargo-allow also ships a pre-commit framework hook for a local, blocking
no-new check. The hook deliberately uses `language: system`: install the
`cargo-allow` binary in the environment that runs pre-commit, then pin the
repository revision in the consumer's `.pre-commit-config.yaml`.

The hook's source subject is the current tracked **worktree**, not the exact
Git index candidate. It can inspect unstaged bytes, so treat it as fast local
feedback rather than proof of the bytes a commit will contain. CI remains the
authoritative merge backstop. The shipped hook remains worktree-scoped; do not
add `--staged` to this entry. For an exact source-exception candidate check,
use the explicit staged command below.
The published template is registered for both the `pre-commit` and `pre-push`
stages. In either stage it remains a worktree advisory check, not a claim about
the exact staged index or pushed commit/tree bytes. Use the pre-push stage for a
last local reminder before sending commits, while keeping CI as the enforcing
merge backstop.

For the current unreleased candidate, use the source revision temporarily:

```yaml
repos:
  - repo: https://github.com/EffortlessMetrics/cargo-allow
    rev: main
    hooks:
      - id: cargo-allow
```

Replace `main` with the first release tag that contains this hook before
adopting it as a stable consumer contract. The hook ignores filenames passed
by pre-commit and scans the repository's tracked worktree, so it has the same
source-tree scope as `cargo-allow check --mode no-new` in CI while retaining a
different source subject. Run it manually with `pre-commit run cargo-allow
--all-files`.

If pre-commit is not part of the repository workflow, the equivalent local
command is:

```bash
# Subject: current tracked worktree, not the exact staged index.
cargo-allow check --mode no-new
```

For an exact source-exception check of the Git index candidate, use the
pre-commit phase explicitly. It reads staged bytes (including partial files),
never falls back to dirty worktree bytes, and records the staged source
identity in JSON reports and receipts:

```bash
cargo-allow check --staged --phase precommit --mode no-new --format json \
  --receipt target/cargo-allow/staged-receipt.json
```

This exact source-exception path currently fails closed when the policy uses
worktree-derived companion families such as workflow extraction, executable
bits, or `.gitattributes` generated-file metadata. It also fails closed when
federated `.allow` inputs are configured, or when `--mode no-new`/`strict`
would require product-move ledger enforcement that is not yet staged-aware.
Use the tracked-worktree check for those repositories until the corresponding
staged adapters exist. It does not invoke the separate `spec-system`/cargo-intent
compatibility profile or self-hosted tool-selection flags.

Before adopting either checked stage, preview the machine-readable hook
contract from the installed binary:

```bash
cargo-allow hooks plan --stage pre-commit
cargo-allow hooks plan --stage pre-push --format json
```

The default plan is read-only and reports the exact `cargo-allow check
--mode no-new` argv, the `tracked_worktree` subject, and the current
ambient-PATH binary resolution. It is a worktree advisory: it does not prove
exact staged-index or pushed-tree bytes, install or overwrite a hook, write a
receipt, contact the network, or mutate policy. CI remains the enforcing merge
backstop. Safe installation, rollback, and exact-index enforcement are separate
capabilities and are not implied by this preview.

When evaluating a direct hook's binary choice, verify an explicitly selected
executable rather than relying on ambient PATH. The verifier invokes only the
selected binary's offline `tool identity` command, checks the expected digest,
and checks the current capability generations. Published installs use the
default `installed-pinned` mode; source-built candidates must be explicitly
marked as preview evidence:

```bash
cargo-allow tool identity --format json
cargo-allow hooks verify \
  --binary /absolute/path/to/cargo-allow \
  --digest sha256:v1:<digest-from-tool-identity> \
  --mode installed-pinned \
  --format human
```

`hooks verify` is an offline preflight report. It does not install a binary or
rewrite an existing plan. The closed runtime seam can be exercised explicitly:

```bash
cargo-allow hooks run \
  --binary /absolute/path/to/cargo-allow \
  --digest sha256:v1:<digest-from-tool-identity> \
  --mode installed-pinned \
  -- check --mode no-new
```

`hooks run` rejects any command other than the exact offline check, requires an
absolute executable path, verifies the selected tool before launch and after
exit, and inherits the check's output and failure posture. It does not invoke a
shell or mutate the policy ledger.

To install a generated hook that uses this verified runtime, pass the same
explicit binary and digest to `hooks plan`. Planning verifies the selected
binary's identity and capability generations, then records the exact runtime
argv in the plan identity:

```bash
cargo-allow hooks plan --stage pre-commit --format json \
  --binary /absolute/path/to/cargo-allow \
  --digest sha256:v1:<digest-from-tool-identity> \
  --mode installed-pinned \
  --output target/cargo-allow/pre-commit-verified-hook-plan.json
cargo-allow hooks apply \
  --plan target/cargo-allow/pre-commit-verified-hook-plan.json \
  --accept
cargo-allow hooks status --stage pre-commit \
  --plan target/cargo-allow/pre-commit-verified-hook-plan.json
```

The resulting managed block invokes `hooks run` with the selected absolute
binary, digest, and closed `check --mode no-new` command. The wrapper verifies
the binary before launch and after exit. This still observes tracked worktree
bytes rather than the exact staged index; CI remains the authoritative merge
backstop.

For a repository that deliberately uses a direct Git hook, the checked JSON
plan can be inspected and applied with an explicit acceptance step:

```bash
cargo-allow hooks plan --stage pre-commit --format json \
  --output target/cargo-allow/pre-commit-hook-plan.json
cargo-allow hooks status --stage pre-commit
cargo-allow hooks apply --plan target/cargo-allow/pre-commit-hook-plan.json --accept
```

`hooks status` is read-only. Without `--plan`, it reports the default ambient
plan; pass the exact JSON plan when inspecting a verified hook. `hooks apply`
resolves the repository's Git hooks directory, validates the plan schema and
identity, and atomically creates the selected hook only when it is absent. It
never overwrites an existing hook:
unmanaged or mismatched managed content is reported as `ManualMerge` or
`Conflict`; a single exact cargo-allow block inside otherwise unrelated hook
content is reported as `Composed` and is left unchanged. These dispositions
are recorded in the apply receipt. The generated wrapper invokes either the
ambient exact offline `cargo-allow check --mode no-new` command or, for a
verified plan, the selected `hooks run` runtime over the tracked worktree; it
does not claim exact staged-index evidence or install a binary.
For a verified hook, retain the exact JSON plan alongside its apply receipt.
Ambient hooks preserve the receipt-only removal route; verified hooks pass the
matching plan explicitly. To remove a recognized managed hook or block, run:

```bash
cargo-allow hooks remove \
  --receipt target/cargo-allow/hooks/pre-commit.apply.receipt.json \
  --plan target/cargo-allow/pre-commit-verified-hook-plan.json \
  --accept
```

Removal is fail-closed: it recomputes the current stage, plan identity, and Git
common hook path from the receipt. An exact standalone hook is removed as a
file; a `Composed` hook removes only its exact managed block and preserves
unrelated bytes. Changed, malformed, unmanaged, or symbolic-link content is
refused. It writes a separate
`cargo-allow.local-hook-remove-receipt.v1`; the receipt records the exact
recreate route through `hooks plan` and `hooks apply`. Existing hook
composition and exact staged-index enforcement remain separate follow-up
capabilities.

## Optional reusable GitHub Action

For hosted Linux CI, cargo-allow also provides a read-only composite Action.
Pin an exact published version; moving channels such as `latest` are rejected.
The current source-install route installs that exact version with Cargo and
verifies the installed binary before running one closed capability (`check`,
`diff`, `audit`, or `doctor`). It does not mutate the ledger, push GitHub
state, or accept arbitrary shell commands.

```yaml
jobs:
  cargo-allow:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - uses: EffortlessMetrics/cargo-allow@<immutable-action-commit>
        with:
          version: '0.1.11'
          command: check
          mode: no-new
          upload-artifacts: 'true'
```

Use an immutable Action commit or a reviewed release reference in a consumer
workflow. The Action uploads its bounded JSON report and, for `check`/`diff`,
receipt under `target/cargo-allow-action`; an analysis failure remains a failed
step even when diagnostics are uploaded. The source-install Action currently
supports Linux runners only. Prebuilt installation and a moving supported
Action tag require separate release/provenance evidence. For `diff`, use a
full-history checkout (`fetch-depth: 0`) and pass an exact `base` revision; the
Action does not fetch or infer a revision on the consumer's behalf.

## Step 1: Bootstrap a policy (first hour)

From your repository root:

```bash
# See what cargo-allow finds (advisory, no policy yet):
cargo-allow audit --kind panic

# Generate a baseline policy from current findings:
cargo-allow propose --kind panic --write policy/allow.toml

# Verify the generated policy passes its own check:
cargo-allow check --mode no-new --config policy/allow.toml --kind panic
```

If `check` passes, commit `policy/allow.toml`. The baseline is now a ratchet
floor — adding a new `.unwrap()` inside the baselined scope will fail the gate.

## Step 2: Broad-scope baseline (optional)

For a broad scope (e.g. all panics in `src/`), use `add --glob` to pin the
current occurrence count:

```bash
cargo-allow add --kind panic --family unwrap --callee unwrap \
  --glob "src/**/*.rs" --owner core --reason "baseline" \
  --classification reviewed_exception --review-after 2027-01-01 \
  --config policy/allow.toml --write policy/allow.toml --force
```

The N+1th in-scope occurrence will fail `check --mode no-new` with
"occurrence_limit exceeded".

## Step 3: CI integration

Add this to `.github/workflows/cargo-allow.yml`:

```yaml
name: cargo-allow
on: [pull_request]
jobs:
  no-new-debt:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # required for diff
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install --git https://github.com/EffortlessMetrics/cargo-allow
      - run: cargo-allow check --mode no-new --config policy/allow.toml --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: cargo-allow-reports
          path: target/cargo-allow/
```

This fails the PR if new exception debt is added beyond the baseline.

## Step 4: Diff posture (optional, for policy-review enforcement)

To require a revision note for weakening policy edits:

```bash
cargo-allow diff --base origin/main --require-change-note
```

This fails if a policy change (e.g. scope broadening) lacks a matching
revision note in `.allow/revisions/`.

## Step 5: Doctor (health check)

```bash
cargo-allow doctor
```

Reports: config validity, inventory source, deleted-tracked files, git
inventory errors, skipped paths, submodule detection, broken evidence links.

## Self-adoption evidence

cargo-allow dogfoods itself on `main` — the repo's own CI runs:

```bash
cargo-allow check --mode no-new --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

This guard passes with 0 new findings on every merge to `main`. The
`policy/allow.toml` ledger receipts every scanned file.

## Known limitations

- Submodule contents are not scanned; run cargo-allow inside each submodule.
- Non-UTF-8 filenames are not yet handled (planned).
- The spec-system governance profile is advisory until portability is proven.
- `--recurse-submodules` for inventory is not supported.
