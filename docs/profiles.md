# Opt-In Governance Profiles

cargo-allow has a small default product path and optional source-tree
governance profiles around it.

The default path is the source-exception ledger:

```text
repository files
-> syntax-visible source findings
-> policy/allow.toml receipts
-> reports, receipts, diffs, explanations, and worklists
```

Profiles use the same source-tree and no-execution model for other governed
repo structures:

```text
profile config
-> source-tree graph or ledger
-> structural validation
-> JSON or Markdown artifact
-> repair worklist
-> advisory, shadow, or blocking rollout
```

Profiles are explicit. A repository opts in by choosing `--profile <name>` on
commands that support profiles and by adding the matching profile config.
Default `cargo-allow check`, `audit`, `diff`, `explain`, and `worklist`
behavior remains the source-exception ledger unless a command explicitly
selects a profile.

## Current Profile

`spec-system` is the first opt-in governance profile. It validates the
source-of-truth graph for proposals, specs, ADRs, implementation plans, active
goals, support tiers, policy ledgers, proof-command fields, release records,
and closeouts.

Supported preview command shape:

```bash
cargo-allow init --profile spec-system
cargo-allow doctor --profile spec-system
cargo-allow check --profile spec-system
cargo-allow audit --profile spec-system
cargo-allow worklist --profile spec-system --format json
cargo-allow explain <artifact-id> --profile spec-system
```

The profile validates relationships, not prose style. It checks IDs, paths,
statuses, required fields, links, support-tier proof mappings, active-goal
references, and closeout links. Formatting requirements exist only where they
make IDs, tables, front matter, and TOML parseable.

## Profile Anatomy

An opt-in profile should have:

| Surface | Role |
| --- | --- |
| Profile config | Declares roots, requirements, and advisory/shadow/blocking posture. |
| Source-tree graph or ledger | Records governed nodes, edges, owners, paths, and statuses. |
| Structural validators | Check parseability, identity, required fields, and links. |
| JSON artifact | Gives agents and CI stable machine-readable posture. |
| Human report | Gives reviewers a short posture summary and claim boundary. |
| Worklist items | Route bounded repair work with suggested actions and proof commands. |
| `doctor` support | Explains setup readiness and missing profile inputs. |
| `init` support | Bootstraps a portable starter layout without changing defaults. |

Profiles can be dogfooded in one repository and then adopted elsewhere without
copying bespoke repository-specific scripts.

## Rollout Modes

Profiles should use the same adoption ladder:

```text
advisory:
  report findings and work items without failure posture

shadow:
  report what would fail, but do not make the profile a hard merge gate

blocking:
  fail only the checks that have enough local burn-in to stay low-noise
```

For `spec-system`, safe structural checks include duplicate IDs, missing
registered files, invalid kinds or statuses, unknown linked IDs, missing
declared IDs, and config or ledger parse failures.

Judgment-heavy checks should stay advisory longer: stale active goals, missing
closeouts, support-tier completeness, and README or release claim coverage.

## Claim Boundary

Profiles must stay inside cargo-allow's source-tree scan boundary. A profile
may parse files, TOML, Markdown, tables, IDs, paths, and links. It may emit
reports, receipts, and worklists.

A profile must not execute proof commands, call GitHub APIs, inspect remote
state, run Cargo, rustc, Clippy, build scripts, proc macros, ripr,
unsafe-review, coverage, network checks, or repository code as part of the
cargo-allow scan.

Future profiles such as `release-system`, `ci-system`, or `evidence-system`
should reuse this architecture only after their source-tree graph and claim
boundary are clear. They are not implemented by the current release.
