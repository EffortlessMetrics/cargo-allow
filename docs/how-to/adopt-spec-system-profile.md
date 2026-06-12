# Adopt The Spec-System Profile

Use the spec-system profile when a repository is ready to govern its
source-of-truth graph in addition to its default source-exception ledger.

Default cargo-allow behavior remains the source exception ledger. The
spec-system profile is one opt-in governance profile: it checks source-tree
relationships between proposals, specs, ADRs, implementation plans, active
goals, support tiers, policy ledgers, proof commands, release records, and
closeouts.

## First Hour

Preview the files cargo-allow would create:

```bash
cargo-allow init --profile spec-system --dry-run
```

Bootstrap the profile when the dry run matches the repo layout:

```bash
cargo-allow init --profile spec-system
```

Check setup readiness:

```bash
cargo-allow doctor --profile spec-system
```

Run the graph check in audit posture:

```bash
cargo-allow check \
  --profile spec-system \
  --mode audit \
  --format markdown \
  --output target/cargo-allow/spec-system.md
```

Generate repair work for humans or agents:

```bash
cargo-allow worklist \
  --profile spec-system \
  --format json \
  --output target/cargo-allow/spec-system-worklist.json
```

Inspect one registered artifact and its graph links:

```bash
cargo-allow explain CARGO-ALLOW-SPEC-0001 \
  --profile spec-system \
  --format json \
  --output target/cargo-allow/spec-system-explain.json
```

## What To Fix First

Start with objective structural repairs:

- duplicate artifact IDs.
- missing registered artifact files.
- invalid artifact kinds or statuses.
- unknown linked artifact IDs.
- registered artifact files that do not contain their declared ID.
- profile config or doc-artifact ledger parse failures.

Leave lifecycle and judgment-heavy checks advisory until the repo has local
burn-in:

- stale active goals.
- missing closeouts.
- support-tier proof completeness.
- README or release claim coverage.

## Adoption Posture

Start with `mode = "advisory"` or `mode = "shadow"` in
`policy/spec-system.toml`. Advisory mode reports findings without failure
posture. Shadow mode reports failure posture in the profile artifact without
making the profile part of default cargo-allow behavior.

Promote only proven, low-noise structural checks later. The profile should not
block on closeout freshness, support-tier completeness, or README claim coverage
until those checks have enough local evidence to stay useful.

## Claim Boundary

The spec-system profile validates source-tree relationships. It may parse TOML
and Markdown, verify IDs, paths, statuses, links, support-tier proof fields,
active-goal references, and closeout links.

It does not execute proof commands, run tests, call GitHub APIs, run Cargo,
rustc, Clippy, build scripts, proc macros, ripr, unsafe-review, coverage, or
network checks. It does not prove semantic correctness, release readiness,
unsafe soundness, test adequacy, or coverage.

Reference: [source-of-truth stack](../source-of-truth/README.md).
