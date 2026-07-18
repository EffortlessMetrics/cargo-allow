# Adopt cargo-allow Across Repos

Use this playbook when moving another repository onto cargo-allow's default
source-exception ledger and the opt-in `spec-system` preview profile.

Start one repository at a time. The goal is to produce useful source-tree
artifacts and file cargo-allow issues for adoption friction, not to make every
target repository perfect in the first PR.

## 1. Pick The Version

Use the latest published cargo-allow release for normal adoption. The
`spec-system` preview is available in the published `0.1.7` release and later.
The first-hour bootstrap cleanup is available in `0.1.8` and later.

```bash
cargo install cargo-allow --locked
```

For a pinned published release with the `spec-system` preview:

```bash
cargo install cargo-allow --version 0.1.11 --locked
```

Do not pin a release-candidate version in another repository before it is
published. Copy a pinned install command from this repository only after that
version is visible on crates.io.

## 2. Inventory The Default Ledger

From the target repository root:

```bash
cargo-allow doctor
cargo-allow audit --format json --output target/cargo-allow/audit.json
cargo-allow check \
  --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

This establishes the default source-exception posture. A passing no-new check
means no new unreceipted findings were found in scanned source-tree inventory.
It does not prove the project is safe, buildable, type-checked, or free of all
possible exceptions.

## 3. Preview The Spec-System Bootstrap

Run a dry run before writing profile files:

```bash
cargo-allow init --profile spec-system --dry-run
```

Review the proposed layout against the repository's existing docs, plans,
goals, and support-tier surfaces. If the generated layout is confusing or too
cargo-allow-specific, file an adoption-friction issue instead of working around
it silently in the target repo.

The generated `policy/spec-system.toml` starts with
`active_goal_required = false`. That is intentional for first-hour adoption:
the bootstrap active goal is a placeholder until the target repo registers real
proposal, spec, plan, support-tier, and closeout artifacts. Flip the setting to
`true` after those links exist and the repo wants active-goal validation.

## 4. Bootstrap Advisory Profile State

When the dry run is acceptable:

```bash
cargo-allow init --profile spec-system
cargo-allow doctor --profile spec-system
cargo-allow check \
  --profile spec-system \
  --mode audit \
  --format json \
  --output target/cargo-allow/spec-system.json
cargo-allow worklist \
  --profile spec-system \
  --format json \
  --output target/cargo-allow/spec-system-worklist.json
```

Start in advisory or shadow posture. Do not add a hard CI gate in the first
adoption PR.

## 5. Fix Objective Structure First

Close one low-judgment repair class before touching lifecycle policy:

- duplicate artifact IDs.
- missing registered files.
- invalid artifact kinds or statuses.
- unknown linked artifact IDs.
- files that do not contain their declared IDs.
- profile config or doc-artifact ledger parse failures.

Keep these advisory during early adoption:

- stale active goals.
- missing closeouts.
- support-tier proof completeness.
- README or release claim coverage.

## 6. Upload CI Artifacts

Add a non-blocking artifact job before any blocking gate:

```bash
cargo-allow check \
  --profile spec-system \
  --mode audit \
  --format json \
  --output target/cargo-allow/spec-system.json

cargo-allow worklist \
  --profile spec-system \
  --format json \
  --output target/cargo-allow/spec-system-worklist.json
```

Upload `target/cargo-allow/` on success and failure. The JSON files give agents
bounded repair work; Markdown reports give reviewers a human summary.

## 7. File cargo-allow Issues For Friction

File issues in cargo-allow when adoption exposes:

- confusing init layout.
- profile config that is not portable.
- false-positive graph findings.
- missing artifact kinds or edge types.
- unclear worklist messages.
- doctor readiness confusion.
- schema or artifact mismatches.
- CI integration friction.
- documentation gaps.

Use the
[cargo-allow-adoption-friction issue template](../../.github/ISSUE_TEMPLATE/cargo-allow-adoption-friction.yml)
and attach the relevant snippets from
`target/cargo-allow/spec-system.json`,
`target/cargo-allow/spec-system-worklist.json`,
`policy/spec-system.toml`, and `policy/doc-artifacts.toml`.

## Done For One Repo

A target repository has a useful first adoption when:

- the default cargo-allow check runs or reports a clear setup gap.
- `policy/spec-system.toml` and `policy/doc-artifacts.toml` exist or their
  absence is intentionally deferred.
- `doctor --profile spec-system` reports clear readiness.
- `check --profile spec-system` emits a JSON artifact.
- `worklist --profile spec-system` emits bounded repair work or an empty queue.
- CI uploads `target/cargo-allow/` artifacts.
- blocking is off or limited to proven structural checks.
- any adoption friction has been filed back on cargo-allow.

## Claim Boundary

The spec-system profile is structural source-tree graph validation. It may
parse TOML and Markdown and inspect repository files. It must not execute proof
commands, call GitHub APIs, inspect remote PR state, run Cargo, rustc, Clippy,
build scripts, proc macros, ripr, unsafe-review, coverage, or network checks as
part of the cargo-allow scan.
