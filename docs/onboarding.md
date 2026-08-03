# Onboarding

cargo-allow has a small default path and opt-in governance profiles.

Default cargo-allow is the source-exception ledger: it scans repository files,
checks syntax-visible findings against `policy/allow.toml`, and emits reports,
receipts, diffs, explanations, and worklists.

Opt-in profiles use the same source-tree/no-execution model for other governed
repo structures. The first profile is `spec-system`, which validates the static
source-of-truth graph for proposals, specs, ADRs, plans, requirements, support
tiers, policy ledgers, proof-command fields, release records, and closeouts.

## Choose Your Path

| I want to... | Start with | Then read |
| --- | --- | --- |
| get one bounded adoption recommendation (source candidate) | `cargo run -p cargo-allow -- adopt` | [Getting started](getting-started.md#2a-get-one-current-adoption-recommendation-source-candidate) |
| audit source exceptions | `cargo-allow doctor` then `cargo-allow audit` | [Getting started](getting-started.md) |
| manage one exception (Published `0.1.11`) | `cargo-allow list` / `cargo-allow explain` / `cargo-allow why` | [Manage an exception](how-to/manage-an-exception.md), [Explain why a finding](how-to/explain-why-a-finding.md) |
| adopt no-new governance | `cargo-allow check --mode no-new` | [Adopt no-new-debt](how-to/adopt-no-new-debt.md) |
| try spec-system | `cargo-allow init --profile spec-system --dry-run` | [Adopt the spec-system profile](how-to/adopt-spec-system-profile.md) |
| add CI | `cargo-allow check --mode no-new` | [Run in CI](how-to/run-in-ci.md) |
| move another repo | start with one repository and upload artifacts | [Adopt cargo-allow across repos](how-to/adopt-cargo-allow-across-repos.md) |
| file adoption friction | use the adoption-friction issue template | [Cross-repo feedback loop](how-to/adopt-cargo-allow-across-repos.md#7-file-cargo-allow-issues-for-friction) and [issue template](../.github/ISSUE_TEMPLATE/cargo-allow-adoption-friction.yml) |

## Audit Source Exceptions

On the current source-candidate channel, get one read-only recommendation
before choosing a bootstrap path:

```bash
cargo run -p cargo-allow -- adopt
```

```bash
cargo-allow doctor
cargo-allow audit
cargo-allow check --mode no-new
```

Success looks like:

- `doctor` reports setup state clearly.
- `audit` emits source-tree inventory.
- `check --mode no-new` reports no new unreceipted findings or gives a clear
  repair queue.

Inspect:

- `target/cargo-allow/check.md`
- `target/cargo-allow/check.receipt.json`

Do not claim this proves the project is safe, buildable, type-checked, or free
of exceptions outside the scanned source-tree/source-syntax surface.

## Adopt No-New Governance

For an existing repository, create a reviewed baseline and then prevent new
unreceipted findings:

```bash
cargo-allow propose --write policy/allow.toml
cargo-allow check --mode no-new
```

Success looks like:

- retained findings have policy receipts with owner, reason, classification,
  lifecycle, selector, and evidence.
- new source-tree findings fail until they are fixed or deliberately receipted.

## Receipt One New Finding

Once `check --mode no-new` is failing on a finding you have decided to accept,
the supported route is plan-then-apply. Source candidate only (current
`main`); these plan flags are not in the Published `0.1.11` surface.

```bash
cargo run -p cargo-allow -- why \
  --kind panic --path src/lib.rs --line 42 \
  --plan target/cargo-allow/add-plan.json

cargo run -p cargo-allow -- add \
  --from-plan target/cargo-allow/add-plan.json \
  --update \
  --owner core \
  --reason "<why this exception is acceptable>" \
  --evidence doc:docs/design.md

cargo run -p cargo-allow -- why --kind panic --path src/lib.rs --line 42
cargo run -p cargo-allow -- check --mode no-new
```

`why --plan` is read-only. `add --from-plan --update` re-verifies the plan
against the live tree before one atomic write, and refuses if anything moved.
The third command is a targeted recheck of that one finding; the fourth is the
repository proof. A passing targeted recheck does not mean the repository
check passes.

Next:

- [Manage an exception](how-to/manage-an-exception.md)
- [Adopt no-new-debt](how-to/adopt-no-new-debt.md)
- [Source exception ledger](source-exception-ledger.md)

## Try Spec-System

Preview before writing files:

```bash
cargo-allow init --profile spec-system --dry-run
```

Bootstrap and inspect the profile:

```bash
cargo-allow init --profile spec-system
cargo-allow doctor --profile spec-system
cargo-allow check --profile spec-system --mode audit
cargo-allow worklist --profile spec-system --format json
```

Success looks like:

- `doctor --profile spec-system` reports readiness or a concrete setup gap.
- `check --profile spec-system` emits graph posture.
- `worklist --profile spec-system --format json` is empty or contains bounded
  repair items.

The generated current-v2 profile has no goals root and does not generate an
active goal. Live work belongs to the controlling GitHub issue and PR-local
implementation slice; archived legacy goals remain historical evidence only.
Repositories that explicitly need legacy compatibility may select the v1
profile, but a legacy goal cannot authorize current work or promote claims.

Do not claim spec-system executes proof commands or proves semantic
correctness. It validates source-tree graph structure only.

## Add CI

Start with the default source-exception gate:

```bash
cargo-allow check \
  --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

Optionally upload spec-system artifacts:

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

Upload `target/cargo-allow/` on success and failure. JSON artifacts give agents
bounded repair work; Markdown reports give reviewers a human summary.

Next:

- [Run in CI](how-to/run-in-ci.md)
- [Run the spec-system profile in CI](how-to/run-spec-system-in-ci.md)

## Move Another Repo

Move one repository at a time:

```bash
cargo-allow doctor
cargo-allow audit --format json --output target/cargo-allow/audit.json
cargo-allow check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md
cargo-allow init --profile spec-system --dry-run
```

Start advisory or shadow for opt-in profiles. Fix objective structural issues
first, then promote only after burn-in.

Next: [Adopt cargo-allow across repos](how-to/adopt-cargo-allow-across-repos.md).

## File Adoption Friction

Use the
[adoption-friction issue template](../.github/ISSUE_TEMPLATE/cargo-allow-adoption-friction.yml)
when another repository exposes a cargo-allow portability or onboarding gap.

File a cargo-allow issue when adoption exposes:

- confusing init layout.
- profile config that is not portable.
- false-positive graph findings.
- missing artifact kinds or edge types.
- unclear worklist messages.
- doctor readiness confusion.
- schema or artifact mismatches.
- CI integration friction.
- documentation gaps.

Attach focused snippets from `target/cargo-allow/`, `policy/spec-system.toml`,
and `policy/doc-artifacts.toml`. Keep the claim boundary explicit: this is about
static source-tree validation, not proof execution.
