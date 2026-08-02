# Release on Tag

Future cargo-allow releases publish from GitHub Actions when a version tag is
pushed. Manual `cargo publish` remains a documented fallback during Trusted
Publishing setup or when automation is blocked.

This document is the operator source of truth for automated release
prerequisites. Sequencing for the `0.1.10` adoption-trust cut lives in
[plans/release/0.1.10-implementation-plan.md](../../plans/release/0.1.10-implementation-plan.md)
(PR E1 documents prerequisites here; PR E2 records workflow_dispatch dry-run
evidence).

The current qualification snapshot is
[0.1.11-readiness.md](0.1.11-readiness.md). It is a go/no-go input, not a tag
or publication authorization.

## Pre-publication package candidate smoke

Before freezing a version/changelog candidate, prove the exact source tree
packages cleanly and the installed binary still answers the first-hour surface
(#2256 Stage A):

```bash
bash scripts/package-candidate-smoke.sh
```

That script:

1. runs `cargo package --workspace --locked`
2. asserts every packaged crate `Cargo.toml` has no `path =` dependencies
3. installs `cargo-allow` into an isolated root after packaging succeeded
4. runs `--version`, `doctor`, and `check`/`list`/`why` `--help` checks
5. writes `target/package-candidate-smoke/package-candidate-smoke.receipt.txt`

Installed first-hour journey (temp consumer repo + JSON receipt
`cargo-allow.source-candidate-smoke-receipt.v1`) is separately proven by:

```bash
bash scripts/source-candidate-smoke.sh
```

That harness path-installs into a temp root (or reuses `CARGO_ALLOW_BIN`),
runs the brownfield first-hour journey plus refresh (location_drift),
`diff --base`, prune preview→write, and git policy rollback after prune in an
isolated git consumer, records omitted-step / preview-apply / malformed-receipt
/ post-install source-hidden ordinary-scan / package-rebuild omit
(`MissingAsset`) / wrong-version / ordinary-scan offline / unexpected-network /
failed-policy-rollback / optional-profile-without-assets (`NotProven`)
negatives, and writes
`target/source-candidate-smoke/source-candidate-smoke.receipt.json`.
It does **not** deny the source tree during path install. Offline
schema/example characterization remains
`cargo test -p cargo-allow --test source_candidate_smoke --locked`.

Exact ten-crate isolation (#2277 / #2372 / #2378 / #2380 / #2408) is
proven by:

```bash
bash scripts/exact-candidate-package-set.sh
```

That harness packages the shared
[`candidate-crate-set.toml`](../dogfood/fixtures/release/candidate-crate-set.toml),
extracts each `.crate`, warms externals via patched `cargo fetch`, assembles a
classic Cargo local-registry (`.crate` + index) for the lockfile graph with
candidate crates injected, installs `cargo-allow` offline with crates-io
replaced by that local-registry while renaming workspace `crates/` away
(`source_checkout_denied` / `CheckoutIsolated`), verifies internal manifests
unpack under the install registry src (not `crates/`), runs omit-crate /
workspace-path / checksum / injected-path / version-conflict / local-registry
omit / candidate-mismatch (`CandidateStale`) / missing-metadata
(`ManifestMalformed`) / source-checkout-denied negatives, and writes
`target/exact-candidate-package-set/exact-candidate-package-set.receipt.json`
(`cargo-allow.exact-candidate-package-set.v1`).

Post-publication registry install remains
[scripts/release-install-smoke.sh](../../scripts/release-install-smoke.sh)
(Stage B). The next 0.1.x cut stays on Rust 1.85; do not raise MSRV here.

## Linux binary archive contract

The first prebuilt distribution lane is Linux-only and target-specific. Build
the exact archive envelope with:

```bash
bash scripts/package-release-binary.sh --version 0.2.0
bash scripts/verify-release-binary.sh \
  target/cargo-allow/release-assets/cargo-allow-v0.2.0-x86_64-unknown-linux-gnu.tar.gz
```

The packager emits the executable and archive checksum sidecars plus bounded
package/install receipts. The verifier extracts outside the source checkout,
rejects unexpected or unsafe archive entries, checks the executable version,
and runs the first-hour command surface from a clean temporary repository.
These scripts prove packaging and clean-install behavior only. They do not
claim GitHub attestation or release publication; the tag-triggered workflow
must perform those checks before attachment. Windows/macOS archives,
universal Linux compatibility, and source-build fallback replacement remain
separate lanes (#2464, #2474, #2476).

This smoke runs in hosted CI on Linux as the `package-smoke` job in
[ci.yml](../../.github/workflows/ci.yml) (on every PR and push to `main`),
producing `package-candidate-smoke-receipt`,
`exact-candidate-package-set-receipt`, and
`source-candidate-smoke-receipt` workflow artifacts. Those hosted receipts are
the durable evidence for the #2256 Stage A / #2278 Stage A+ / #2372 Stage A
candidate claims; Windows and macOS candidate smoke remain a documented
follow-up.
## Prerequisites

Complete these checks before the first tag-triggered automated release:

| Prerequisite | Verification |
| --- | --- |
| Trusted Publishing on all twelve crates | Each crate in [Publish order](#publish-order) has crates.io **Settings → Trusted Publishing** with owner `EffortlessMetrics`, repo `cargo-allow`, workflow `release.yml` |
| Prior manual publish per crate | `0.1.0`–`0.1.9` manual publishes satisfy crates.io's first-publish requirement |
| Workflow dry-run green on `main` | **Actions → Release → Run workflow**; preflight passes and publish runs `cargo publish --dry-run` only ([Manual dry-run](#manual-dry-run)) |
| Token fallback documented | `CARGO_REGISTRY_TOKEN` secret exists only when OIDC is not yet configured for every crate ([Token fallback](#token-fallback-migration-only)) |
| Release prep merged | Version bump, `docs/release/X.Y.Z.md`, and `docs/release/github/vX.Y.Z.md` on `main` before tagging |
| No-new guard green on release head | `cargo-allow check --mode no-new` receipt on the commit to tag |

Do not push a version tag until Trusted Publishing or an approved token fallback
is verified and a recent workflow_dispatch dry-run is green.

## Canonical Path

1. Merge release-prep PRs to `main` (version bump, release record, install pins,
   GitHub release notes draft under `docs/release/github/vX.Y.Z.md`, and promote
   [`docs/dogfood/fixtures/getting-started/published-command-registry.toml`](../dogfood/fixtures/getting-started/published-command-registry.toml)
   so the Published first-run command contract matches the crates.io binary).
2. Push an annotated tag matching the workspace version:

   ```bash
   git tag -a v0.1.10 -m "cargo-allow 0.1.10"
   git push origin v0.1.10
   ```

3. The [Release workflow](../../.github/workflows/release.yml) runs:
   - **preflight** — `fmt`, `clippy`, `cargo test --workspace`,
     `cargo package --workspace --locked`, and the default no-new guard.
   - **publish** — runs [release version preflight](../../scripts/release-version-preflight.sh)
     (tag/workspace alignment, internal dependency versions, CHANGELOG section,
     and release-record files; release-record checks skip on workflow_dispatch),
     then publishes the twelve workspace crates to crates.io in dependency order
     (dry-run before each upload).
   - **install-smoke** *(tag pushes only)* — after `cargo-allow` is published,
     runs [release install smoke](../../scripts/release-install-smoke.sh):
     `cargo install cargo-allow --version "$VERSION" --locked`, then
     `cargo-allow --version`, `doctor`, `check --help`, and
     `doctor --profile spec-system --help`. Skipped on workflow_dispatch dry-run.
   - **github-release** — creates a GitHub Release from
     `docs/release/github/vX.Y.Z.md` when that file exists, after install-smoke
     passes.

4. After the workflow succeeds, finish the release record in
   `docs/release/X.Y.Z.md` with workflow run id, registry visibility checks, and
   the install-smoke receipt artifact (`release-install-smoke-receipt`).

## Publish Order

Internal crates must publish in dependency order:

```text
1. allow-core
2. allow-policy
3. allow-inventory
4. allow-files
5. allow-rust
6. allow-match
7. allow-report
8. allow-policy-legacy
9. allow-diff
10. repo-protocol
11. repo-edit
12. cargo-allow
```

Each crate is dry-run verified immediately before upload. The workflow waits for
crates.io index visibility of the **exact published version** (up to 30 attempts,
10 seconds apart) before publishing dependents. Visibility checks use
[`scripts/verify-crate-registry-version.sh`](../../scripts/verify-crate-registry-version.sh)
rather than a crate-name-only `cargo search` grep.

### Verifying publish order locally

Before tagging, confirm packaging and dry-run publish for the full workspace:

```bash
cargo package --workspace --locked
for crate in allow-core allow-policy allow-inventory allow-files allow-rust \
  allow-match allow-report allow-policy-legacy allow-diff repo-protocol repo-edit \
  cargo-allow; do
  cargo publish --dry-run -p "${crate}" --locked
done
```

The [release workflow](../../.github/workflows/release.yml) uses the same crate
list and order. A failure mid-publish leaves earlier crates on crates.io; use
[Recovery and yank](#recovery-and-yank) before retrying.

## crates.io Trusted Publishing (Preferred)

The release workflow uses [crates.io Trusted
Publishing](https://crates.io/docs/trusted-publishing) via
`rust-lang/crates-io-auth-action@v1` and `permissions.id-token: write`.

Configure once per published crate on crates.io (**Settings → Trusted
Publishing**):

| Field | Value |
| --- | --- |
| Repository owner | `EffortlessMetrics` |
| Repository name | `cargo-allow` |
| Workflow filename | `release.yml` |
| Environment | *(optional)* leave blank unless you add a GitHub `release` environment |

Trusted Publishing requires at least one prior manual publish for each crate.
The `0.1.0`–`0.1.9` releases were published manually and satisfy that
prerequisite.

Configure Trusted Publishing on **each** published crate:

| # | Crate | crates.io settings |
| --- | --- | --- |
| 1 | `allow-core` | Settings → Trusted Publishing |
| 2 | `allow-policy` | Settings → Trusted Publishing |
| 3 | `allow-inventory` | Settings → Trusted Publishing |
| 4 | `allow-files` | Settings → Trusted Publishing |
| 5 | `allow-rust` | Settings → Trusted Publishing |
| 6 | `allow-match` | Settings → Trusted Publishing |
| 7 | `allow-report` | Settings → Trusted Publishing |
| 8 | `allow-policy-legacy` | Settings → Trusted Publishing |
| 9 | `allow-diff` | Settings → Trusted Publishing |
| 10 | `repo-protocol` | Settings → Trusted Publishing |
| 11 | `repo-edit` | Settings → Trusted Publishing |
| 12 | `cargo-allow` | Settings → Trusted Publishing |

All twelve rows use the same GitHub binding: owner `EffortlessMetrics`, repository
`cargo-allow`, workflow filename `release.yml`. Leave **Environment** blank
unless the workflow is later scoped to a GitHub `release` environment.

## Token Fallback (Migration Only)

If Trusted Publishing is not yet configured for every crate, add a repository
secret named `CARGO_REGISTRY_TOKEN`. The publish job uses it when OIDC exchange
is unavailable. Remove the secret after Trusted Publishing is verified for all
twelve crates.

Do not commit API tokens to the repository.

## Manual Dry-Run

Use workflow_dispatch to validate release automation without uploading. This is
the proof step for [PR E2](../../plans/release/0.1.10-implementation-plan.md#pr-e2-dry-run-release-workflow-on-main)
in the `0.1.10` plan.

1. Open **Actions → Release → Run workflow** on `main`.
2. Leave the branch selector on `main` and start the run.
3. Confirm the **preflight** job passes (`fmt`, `clippy`, `test`, `package`,
   no-new guard).
4. Confirm the **publish** job authenticates (`auth: oidc` in
   `release-publish.receipt.json` when Trusted Publishing is configured, or
   `auth: secret` when using `CARGO_REGISTRY_TOKEN`).
5. Confirm the publish job logs workspace packaging validation from preflight and
   a single `allow-core` `cargo publish --dry-run` (dependent crate dry-runs
   require index visibility and are skipped on workflow_dispatch).
6. Confirm no real `cargo publish` upload occurs.
7. Download the `release-publish-receipt` artifact and record the workflow run
   id in the release record or plan closeout.

Tag pushes always perform real publishes once preflight succeeds. A
workflow_dispatch run never uploads to crates.io.

## Manual Publish Fallback

When automation cannot run, follow the per-release record (for example
[0.1.9.md](0.1.9.md)):

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo package --workspace --locked
```

Then publish each crate in order:

```bash
cargo publish --dry-run -p <crate> --locked
cargo publish -p <crate> --locked
```

Create the GitHub Release from `docs/release/github/vX.Y.Z.md` after publication.

## Recovery and yank

Published crate versions are irreversible. If a published package is wrong:

1. **Stop** — do not push another tag for the same version string.
2. **Assess scope** — identify which crates from [Publish order](#publish-order)
   reached crates.io before the failure.
3. **Yank** affected versions (newest dependent first when multiple crates
   published):

   ```bash
   cargo yank <crate> --vers X.Y.Z
   ```

4. **Fix on `main`** — merge corrective changes under a new patch version; never
   reuse the yanked version number.
5. **Republish** — either re-run the release workflow on a new tag or follow
   [Manual publish fallback](#manual-publish-fallback) in dependency order.
6. **Record** — update `docs/release/X.Y.Z.md` with yank actions, workflow run
   ids, and the replacement version.

Yanking removes the version from default dependency resolution but does not
delete download history. Prefer yank plus a new patch over force-publishing the
same version.

If a workflow fails mid-publish, inspect the publish job log for the last
successful crate, yank any incorrect uploads, fix `main`, and dry-run again
before tagging.

## Install smoke (tag pushes)

Tag-triggered releases run install smoke automatically after publish completes.
The job installs the exact published version from crates.io and exercises core
CLI surfaces:

```bash
cargo install cargo-allow --version "$VERSION" --locked
cargo-allow --version
cargo-allow doctor
cargo-allow check --help
cargo-allow doctor --profile spec-system --help
```

Local characterization against the latest published release:

```bash
bash scripts/test-release-install-smoke.sh
```

workflow_dispatch dry-runs skip install-smoke because no new version is uploaded.

## Claim Boundary

The release workflow proves formatting, lint, tests, packaging, no-new policy
posture, successful crates.io uploads, and post-publish install smoke for the
tagged commit. Install smoke verifies the published binary installs and exposes
expected CLI help surfaces; it does not run repository checks, proof commands,
or spec-system graph validation against a consumer repo.
