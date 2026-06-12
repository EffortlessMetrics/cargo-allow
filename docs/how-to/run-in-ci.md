# Run in CI

Use cargo-allow in CI to show pull request posture and keep mainline policy
receipts current.

## Pull Requests

Run a diff posture check:

```bash
cargo-allow diff \
  --base origin/main \
  --format markdown \
  --output target/cargo-allow/pr-summary.md
```

Upload `target/cargo-allow/pr-summary.md` as a PR artifact or post it as a
review summary.

## Mainline

Run the full no-new check and save a receipt:

```bash
cargo-allow check \
  --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

Upload `target/cargo-allow/` on success and failure. The receipt is the durable
source-tree claim for the run.

Minimal GitHub Actions shape:

```yaml
- name: cargo-allow source exception check
  run: |
    mkdir -p target/cargo-allow
    cargo-allow check \
      --mode no-new \
      --format markdown \
      --receipt target/cargo-allow/check.receipt.json \
      --output target/cargo-allow/check.md

- uses: actions/upload-artifact@v7.0.1
  if: always()
  with:
    name: cargo-allow
    path: target/cargo-allow/
```

## Claim Boundary

The CI scan should not require the checked-out repository to build. It does not
invoke Cargo metadata, rustc, Clippy, build scripts, proc macros, or external
evidence tools.

Reference: [CI](../ci.md).
