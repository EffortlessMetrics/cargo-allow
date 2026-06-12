# Run The Spec-System Profile In CI

Use CI to publish opt-in spec-system artifacts for reviewers and agents. Keep
default cargo-allow checks focused on the source-exception ledger.

## Advisory Artifact

Start by uploading artifacts without blocking on the profile:

```bash
mkdir -p target/cargo-allow

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

cargo-allow worklist \
  --profile spec-system \
  --format json \
  --output target/cargo-allow/spec-system-worklist.json
```

Upload `target/cargo-allow/` on success and failure. The JSON artifact is useful
for agents. The Markdown artifact is useful for reviewers.

## Shadow Burn-In

When the repo has clean advisory artifacts, set `mode = "shadow"` in
`policy/spec-system.toml`.

Shadow mode reports failure posture in the spec-system artifact when findings
exist, but the profile is still an opt-in governance profile. It does not change
default `cargo-allow check` behavior and does not execute proof commands.

Track a few clean mainline runs before promoting anything. Record the run IDs or
artifact summaries in the relevant implementation plan or closeout.

## Blocking Promotion

Promote only objective structural checks after burn-in:

- malformed explicit profile config.
- missing or invalid doc-artifact ledger.
- duplicate artifact IDs.
- invalid artifact kind or status.
- missing registered artifact files.
- registered artifact files missing their declared IDs.
- unknown linked artifact IDs.

Keep these advisory longer:

- stale active goals.
- missing closeouts.
- support-tier proof completeness.
- README or release claim coverage.

Those checks can be valuable, but they involve more repository judgment.

## GitHub Actions Shape

Run the default source-exception check separately from this opt-in profile:

```yaml
- name: cargo-allow source exception check
  run: |
    cargo-allow check \
      --mode no-new \
      --format markdown \
      --receipt target/cargo-allow/check.receipt.json \
      --output target/cargo-allow/check.md

- name: cargo-allow spec-system profile
  run: |
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
    cargo-allow worklist \
      --profile spec-system \
      --format json \
      --output target/cargo-allow/spec-system-worklist.json
```

This shape keeps the default product path small while making the profile
artifact available to reviewers and agents.

## Claim Boundary

The spec-system cargo-allow scan is structural source-tree graph validation. It
does not execute proof commands, call GitHub APIs, inspect remote PR state, run
Cargo, rustc, Clippy, build scripts, proc macros, ripr, unsafe-review, coverage,
or network checks.

Reference: [CI](../ci.md).
