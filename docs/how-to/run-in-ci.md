# Run in CI

Use cargo-allow in CI to show pull request posture and keep mainline policy
receipts current.

> Maturity: `check` is Stable in published `0.1.11` and Stabilizing on current
> main. See the [command maturity table](../status/SUPPORT_TIERS.md#command-maturity).

Copy the complete GitHub Actions workflows rather than inventing checkout,
install, fetch, or artifact steps:

| Lane | Complete workflow |
| --- | --- |
| Pull-request posture | [`examples/github-actions/cargo-allow-diff.yml`](../../examples/github-actions/cargo-allow-diff.yml) |
| Mainline no-new | [`examples/github-actions/cargo-allow-check.yml`](../../examples/github-actions/cargo-allow-check.yml) |

Canonical reference (formats, flags, claim language): [CI](../ci.md).
Diagnosis: [Troubleshoot cargo-allow](troubleshoot-cargo-allow.md).
Removal: [Rollback cargo-allow adoption](rollback-cargo-allow-adoption.md).

## Choose the product channel

| Channel | Install step |
| --- | --- |
| **Published** `0.1.11` | `cargo install cargo-allow --version 0.1.11 --locked` |
| **Source candidate** (unreleased main) | `cargo install --git https://github.com/EffortlessMetrics/cargo-allow cargo-allow --locked` |

The committed examples pin Published `0.1.11`. Keep first-run commands inside
the offline registry
([`published-command-registry.toml`](../dogfood/fixtures/getting-started/published-command-registry.toml)).
Do not teach candidate-only commands such as `why` as ordinary Published CI
steps.

## Pull-request posture

Requirements:

- Checkout with `fetch-depth: 0` (or an equivalent explicit fetch of the selected
  base). Shallow defaults often leave `origin/<base>` unavailable and make
  `diff --base` fail closed.
- Resolve an exact base ref (for example
  `origin/${GITHUB_BASE_REF:-main}`). Do not silently substitute `HEAD` or an
  empty comparison.
- Run `cargo-allow diff --base <base>` with a retained Markdown (and optional
  JSON) artifact under `target/cargo-allow/`.
- Upload `target/cargo-allow/` with `if: always()` so posture failures still
  leave a review artifact.
- Do **not** set `continue-on-error` on the gate step. Policy/runtime failures
  remain exit `1`; invocation misuse remains exit `2` (see
  [Error codes](../error-codes.md)).
- Use `--require-change-note` only when the repository intentionally adopts that
  gate (operator guide:
  [Manage an exception](manage-an-exception.md)).

Command shape:

```bash
cargo-allow diff \
  --base origin/main \
  --format markdown \
  --output target/cargo-allow/pr-summary.md
```

## Mainline no-new

Requirements:

- Checkout the repository (default depth is fine for `check`; history depth is
  required for `diff`).
- Install the selected cargo-allow identity above.
- Create `target/cargo-allow/` before writing outputs.
- Run `cargo-allow check --mode no-new` with Markdown and receipt outputs
  (examples also emit advisory `audit` and optional SARIF).
- Upload `target/cargo-allow/` under `if: always()`.
- Keep the gate blocking: no `continue-on-error` on the check step.

Command shape:

```bash
mkdir -p target/cargo-allow
cargo-allow check \
  --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

## Output formats

| Format | Purpose |
| --- | --- |
| `human` | Terminal / step log reading |
| `markdown` | PR review artifact and `$GITHUB_STEP_SUMMARY` |
| `json` | Automation and agents |
| `html` | Browsable local/CI report |
| `sarif` | Code-scanning integration where the host supports it |

Aliases exist only when the selected command/`--format` help for that release
lists them. Prefer the names above on the Published `0.1.11` path.

## Artifact and exit contract

- Upload artifacts on success **and** failure (`if: always()`).
- Exit `1` means policy/runtime/config failure — keep the job red.
- Exit `2` means usage/invocation error — fix flags or workflow wiring; do not
  broaden policy.
- Never hide a required gate with `continue-on-error`.

## Claim boundary

The CI scan should not require the checked-out repository to build. It does not
invoke Cargo metadata, rustc, Clippy, build scripts, proc macros, or external
evidence tools for its own scan. The install step fetches the `cargo-allow`
tool; the policy scan remains source-tree only.

This how-to proves the committed GitHub Actions examples and ownership
instructions are coherent and checked offline. Hosted shallow-checkout
characterization for `diff --base` lives in
`scripts/shallow-diff-base-smoke.sh` (CI job `shallow-diff-smoke`, #2366).
It does not prove every third-party CI platform or installed package isolation
(#2278).
