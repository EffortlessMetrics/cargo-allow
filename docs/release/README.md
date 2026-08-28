# Release Operations

Future cargo-allow releases publish from GitHub Actions when a version tag is
pushed. The selected publication path uses a crates.io API token exposed only
as `CARGO_REGISTRY_TOKEN`; Trusted Publishing is optional post-release
hardening, not a release prerequisite.

This document is the operator source of truth for automated release
prerequisites. `0.1.11` is the latest published cargo-allow release. The
workspace `0.2.0` version and its [release record](0.2.0.md) are an unpublished
candidate until both release rails below are separately authorized and proven.
The [0.1.11 qualification snapshot](0.1.11-readiness.md) remains historical
evidence for the published patch release.

## 0.2.0 release rails

The namespace publication and the cargo-allow tag release are distinct,
irreversible operations. A document, issue, pull request, rehearsal, or merged
authorization-file template does not authorize either operation.

Run them in this order:

1. **Rehearse without publishing.** A normal `workflow_dispatch` of
   [`.github/workflows/release.yml`](../../.github/workflows/release.yml) and
   package-only topology publisher runs do not read the registry token or upload
   crates. They prove the selected source candidate and workflow path, not
   registry publication.
2. **Freeze one exact namespace candidate.** Create
   `release/authorize-v0.2.0.json` only after explicit publication
   authorization. Its commit, parent tree, lockfile digest, topology digest,
   and selected rows must match the authorized candidate exactly.
3. **Publish the new namespaces.** The authorization-file push triggers
   [`.github/workflows/release-authorized.yml`](../../.github/workflows/release-authorized.yml),
   which publishes exactly twelve `0.1.0` namespace rows: four shared, five
   cargo-intent, and three cargo-proof packages. This workflow stops after its
   registry receipt; it does not push a tag or dispatch the cargo-allow release.
4. **Reconcile publication truth.** Verify exact registry checksums, update the
   V2 topology publication state and release/support docs through a separate
   reviewed change, and preserve the namespace receipt.
5. **Authorize the cargo-allow release separately.** Only an explicitly
   authorized annotated `v0.2.0` tag may trigger
   [`.github/workflows/release.yml`](../../.github/workflows/release.yml). Its
   topology-selected candidate has thirteen rows: ten cargo-allow packages plus
   `effortless-repo-protocol`, `effortless-repo-snapshot`, and
   `effortless-repo-edit`. Before pushing the tag, the operator must preserve a
   workflow-owned receipt proving those three selected shared rows are
   `AlreadyPublishedExact` with checksum equality. The tag workflow now derives
   and enforces this
   shared-first read-only registry preflight before any cargo-allow upload. Its
   commit/tree/topology-bound receipt is attached to the run; a missing row,
   version mismatch, checksum mismatch, malformed response, registry error, or
   incomplete result fails closed. Once it is satisfied, the expected missing
   uploads are the ten cargo-allow rows.
   `effortless-rust-source-index` belongs only to the namespace rail;
   cargo-intent and cargo-proof packages are not part of the cargo-allow
   candidate.

When a release may be incomplete, unsafe, or unsupported after publication,
follow the [release incident and recovery runbook](incident-recovery.md). It
preserves the original identity and evidence and separates containment from a
new verified recovery version.

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
`cargo-allow.source-candidate-smoke-receipt.v1`) remains available as a
standalone characterization harness:

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
It does **not** deny the source tree during path install. The exact-candidate
journey below is the release-claim path for #3357.

The exact-candidate install journey binds the isolated thirteen-crate package
receipt to this first-hour journey and refuses a workspace-path fallback:

```bash
bash scripts/exact-candidate-install-journey.sh
```

It emits `target/exact-candidate-install-journey/exact-candidate-install-journey.receipt.json`
(`cargo-allow.exact-candidate-install-journey.v1`). The receipt carries the
SHA-256 digests of the package-set receipt, journey receipt, and canonical
candidate fixture; it requires source-checkout denial, sibling-mismatch
negatives, a finding/rollback path, and cleanup of the temporary consumer and
journey artifacts. It proves a pre-publication exact-candidate journey only;
it does not publish, tag, or install from a registry.

The exact installed upgrade/rollback slice extends that proof across the
published `0.1.11` binary and the candidate binary:

```bash
bash scripts/exact-upgrade-rollback-journey.sh
```

It installs `0.1.11` into a separate root, invokes both binaries by absolute
path, restores a captured repository preimage, and reruns the old binary. The
receipt proves binary identity, candidate package-set binding, and preservation
of an unrelated file. It is deliberately read-only with respect to the
fixture's policy state; migration-write compatibility remains a separate claim.

Schema/example characterization remains
`cargo test -p cargo-allow --test source_candidate_smoke --locked`.

The topology-derived cargo-allow candidate currently contains thirteen rows:
the ten cargo-allow packages plus `effortless-repo-protocol`,
`effortless-repo-snapshot`, and `effortless-repo-edit`. Exact isolation
(#2277 / #2372 / #2378 / #2380 / #2408) is proven by:

```bash
bash scripts/exact-candidate-package-set.sh
```

The four shared `0.1.0` packages have a separate non-publishing rehearsal:

```bash
python scripts/release-topology-publisher.py \
  --mode shared \
  --package-only \
  --receipt target/cargo-allow/shared-package-candidate.receipt.json
```

This writes `SharedPackageCandidateV1`
(`cargo-allow.shared-package-candidate.v1`) with exact commit, tree,
`Cargo.lock`, topology, package-byte digests, and the four selected rows. It
does not query crates.io or upload a crate; publication remains a separate
authorized operation.

That rehearsal packages the four rows selected by the V2 `shared` topology
mode, extracts each `.crate`, and records the exact package-byte digests. The
separate cargo-allow exact-candidate harness uses
[`candidate-crate-set.toml`](../dogfood/fixtures/release/candidate-crate-set.toml),
warms externals via patched `cargo fetch`, assembles a classic Cargo
local-registry (`.crate` + index) for the lockfile graph with candidate crates
injected, installs `cargo-allow` offline with crates-io replaced by that
local-registry while renaming workspace `crates/` away
(`source_checkout_denied` / `CheckoutIsolated`), verifies internal manifests
unpack under the install registry src (not `crates/`), runs omit-crate /
workspace-path / checksum / injected-path / version-conflict / local-registry
omit / candidate-mismatch (`CandidateStale`) / missing-metadata
(`ManifestMalformed`) / source-checkout-denied negatives, and writes
`target/exact-candidate-package-set/exact-candidate-package-set.receipt.json`
(`cargo-allow.exact-candidate-package-set.v1`).

Post-publication registry install remains
[scripts/release-install-smoke.sh](../../scripts/release-install-smoke.sh)
(Stage B). Published `0.1.11` stays on Rust 1.85; the unpublished `0.2.0`
candidate uses Rust 1.95.

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
These scripts prove packaging and clean-install behavior only. On an exact
version tag, the release workflow builds the tagged Linux binary, passes the
tag/commit/tree identity into both receipts, attests the exact archive, verifies
that attestation with `gh attestation verify`, and only then regenerates the
manifest and attaches the archive, sidecars, receipts, and manifest together.
The `ATTESTATION_VERIFIED=true` receipt flag is reserved for that verified
workflow handoff; the verifier does not claim an attestation on its own.
Windows/macOS archives,
universal Linux compatibility, and source-build fallback replacement remain
separate lanes (#2464, #2474, #2476).

This smoke runs in hosted CI on Linux as the `package-smoke` job in
[ci.yml](../../.github/workflows/ci.yml) (on every PR and push to `main`),
producing `package-candidate-smoke-receipt`,
`exact-candidate-package-set-receipt`, and
`exact-candidate-install-journey-receipt` workflow artifacts. Those hosted
receipts are the durable evidence for the #2256 Stage A / #2278 Stage A+ /
#2372 / #3357 candidate claims; Windows and macOS candidate smoke remain a
documented follow-up.

## Linux archive: download and install a tagged release

The tagged release workflow publishes the prebuilt archive only for the
claimed `x86_64-unknown-linux-gnu` target and only after the exact archive,
manifest, clean-install, and attestation gates pass. Use an exact `v<version>`
tag; `latest` is not an evidence-bearing substitute.

From a Linux host with `gh`, `sha256sum`, and `tar` installed:

```bash
VERSION=0.2.0
REPOSITORY=EffortlessMetrics/cargo-allow
ARCHIVE="cargo-allow-v${VERSION}-x86_64-unknown-linux-gnu.tar.gz"
DOWNLOAD_DIR="cargo-allow-v${VERSION}-download"

mkdir -p "${DOWNLOAD_DIR}"
gh release download "v${VERSION}" \
  --repo "${REPOSITORY}" \
  --pattern "${ARCHIVE}*" \
  --dir "${DOWNLOAD_DIR}"
cd "${DOWNLOAD_DIR}"

sha256sum --check "${ARCHIVE}.sha256"
gh attestation verify "${ARCHIVE}" --repo "${REPOSITORY}"
tar -xzf "${ARCHIVE}"
cd "cargo-allow-v${VERSION}-x86_64-unknown-linux-gnu"
./cargo-allow --version
```

The version command must report `cargo-allow <version>` for the downloaded
tag. The executable digest sidecar can be checked after extraction with:

```bash
test "$(sha256sum cargo-allow | cut -d' ' -f1)" = \
  "$(awk '$2 == "cargo-allow" { print $1; exit }' \
    "../${ARCHIVE}.executable.sha256")"
```

Install the verified executable in a user-owned directory, then put that
directory first on `PATH` so an older ambient installation is not selected:

```bash
mkdir -p "$HOME/.local/bin"
install -m 0755 cargo-allow "$HOME/.local/bin/cargo-allow"
export PATH="$HOME/.local/bin:$PATH"
cargo-allow --version
```

For an upgrade, download and verify the new archive in a separate directory
before replacing the existing executable. For rollback, restore the previous
verified executable. To uninstall this archive installation, remove only
`$HOME/.local/bin/cargo-allow`; package-manager or `cargo install` copies are
separate installations and must be removed through their own paths.

This is a Linux archive installation path, not a universal Linux, Windows, or
macOS support claim. The crates.io `cargo install cargo-allow --locked` path
remains the source-build fallback.

## Prerequisites

Complete these checks before the `0.2.0` tag-triggered cargo-allow release:

| Prerequisite | Verification |
| --- | --- |
| GitHub Actions crates.io token | Repository secret `CARGO_REGISTRY_TOKEN` is available to the guarded release workflow; its value is never printed or retained |
| Namespace rail reconciled | The twelve-row namespace workflow receipt, exact crates.io checksums, and V2 topology publication state agree before tag authorization |
| Workflow dry-run green on `main` | **Actions → Release → Run workflow**; preflight passes and publish runs `cargo publish --dry-run` only ([Manual dry-run](#manual-dry-run)) |
| Release prep merged | Version bump, `docs/release/X.Y.Z.md`, and `docs/release/github/vX.Y.Z.md` on `main` before tagging |
| No-new guard green on release head | `cargo-allow check --mode no-new` receipt on the commit to tag |

Do not create `release/authorize-v0.2.0.json` or push a version tag without
separate explicit authorization for that irreversible operation. Before tag
authorization, require the reconciled namespace receipt and a recent green
non-publishing `workflow_dispatch` rehearsal.

## Canonical Path

1. Complete and reconcile the namespace rail described above, then merge
   cargo-allow release-prep PRs to `main` (version bump, release record, install pins,
   GitHub release notes draft under `docs/release/github/vX.Y.Z.md`, and promote
   [`docs/dogfood/fixtures/getting-started/published-command-registry.toml`](../dogfood/fixtures/getting-started/published-command-registry.toml)
   so the Published first-run command contract matches the crates.io binary).
2. Push an annotated tag matching the workspace version:

   ```bash
   git tag -a v0.2.0 -m "cargo-allow 0.2.0"
   git push origin v0.2.0
   ```

3. The [Release workflow](../../.github/workflows/release.yml) runs:
   - **preflight** — `fmt`, `clippy`, `cargo test --workspace`, the
     topology-derived candidate preflight, and the default no-new guard.
   - **publish** — runs [release version preflight](../../scripts/release-version-preflight.sh)
     (tag/workspace alignment, internal dependency versions, CHANGELOG section,
     and release-record files; release-record checks skip on workflow_dispatch),
     then publishes the topology-derived cargo-allow rows to crates.io in dependency order
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

The release publisher derives the exact rows and dependency order from
`policy/product-package-topology-v2.toml`. The expected cargo-allow candidate
contains the ten cargo-allow-family packages plus the selected shared packages;
the count is a receipt result, not a maintained list. Each row carries its own
package version, crate digest, and registry checksum.

Recovery is bound to the original publication evidence. A guarded
`workflow_dispatch` recovery must provide the exact tag commit/tree, a bounded
authorization reference, and the numeric run id that produced the incomplete
`release-publish-receipt` artifact. The workflow downloads that artifact with
`actions: read`; the publisher accepts only an incomplete incident receipt for
the same candidate and skips only rows already recorded as exact. It never
rebuilds recovery from current `main`. Rehearsal runs do not read the registry
token and do not invoke the upload path.

Each crate is dry-run verified immediately before upload. The workflow waits for
crates.io index visibility of the **exact published version** (up to 30 attempts,
10 seconds apart) before publishing dependents. Visibility checks use
[`scripts/verify-crate-registry-version.sh`](../../scripts/verify-crate-registry-version.sh)
rather than a crate-name-only `cargo search` grep.

### Verifying publish order locally

Before tagging, confirm the topology-derived candidate and dry-run publisher:

```bash
python3 scripts/release-topology-publisher.py \
  --mode cargo-allow \
  --receipt target/cargo-allow/topology-publish.receipt.json
```

The [release workflow](../../.github/workflows/release.yml) consumes the same
topology-derived rows. A failure mid-publish leaves earlier crates on crates.io; use
[Recovery and yank](#recovery-and-yank) before retrying.

## crates.io API token publication

The guarded GitHub Actions workflows authenticate to crates.io with the
repository secret `CARGO_REGISTRY_TOKEN`, exposed only as the
`CARGO_REGISTRY_TOKEN` environment variable. The token value is never printed,
hashed, encoded, placed in a step output, or retained in a receipt. Receipts
record only `auth_source = "crates_io_api_token"` and the workflow identity.

Trusted Publishing/OIDC is optional post-release hardening and is not a
precondition for the 0.2.0 publication path.

Do not commit API tokens to the repository.

## Manual Dry-Run

Use workflow_dispatch to validate release automation without uploading. This is
a rehearsal of the current cargo-allow candidate, not registry evidence or
authorization.

1. Open **Actions → Release → Run workflow** on `main`.
2. Leave the branch selector on `main` and start the run.
3. Confirm the **preflight** job passes (`fmt`, `clippy`, `test`, `package`,
   no-new guard).
4. Confirm the **publish** job records `auth: crates_io_api_token` in
   `release-publish.receipt.json` while the logs show token lookup was skipped.
5. Confirm the publish job logs topology-derived candidate validation and
   performs no upload during the rehearsal.
6. Confirm no real `cargo publish` upload occurs.
7. Download the `release-publish-receipt` artifact and record the workflow run
   id in the release record or plan closeout.

Tag pushes always perform real publishes once preflight succeeds. A normal
`workflow_dispatch` run never uploads to crates.io. The guarded
`publish_recovery` dispatch is a separate exception that requires the exact
tagged version, commit, tree, and authorization inputs; do not use it as a
general manual publish route. The recovery path remains unavailable for real
incidents until the remaining #2509 visibility, checksum-plan, missing-crate,
and typed-receipt controls are complete. See the [incident and recovery
runbook](incident-recovery.md).

## Manual Publish Fallback

Manual fallback does not bypass either release authorization. For `0.2.0`, use
it only after the twelve-row namespace rail has separate explicit authorization
and a reconciled receipt, and after the cargo-allow rail receives separate
explicit authorization for the exact candidate. When automation cannot run
after those gates are satisfied, follow the per-release record (for example
[0.1.9.md](0.1.9.md) for the historical patch-release shape):

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo package --workspace --locked
```

Then publish only the rows authorized for that rail in dependency order:

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

## Exact-candidate qualification authority

The cargo-allow release-candidate qualification authority is the exact
candidate chain (#2924 candidate artifact, #2925 isolated local-registry
install, #2926 qualification journey; schema
`cargo-allow.exact-candidate.v2`). The ambient workspace
`cargo package --workspace` smoke (Stage A of package-smoke) is a
crate-byte producer feeding that chain, not a qualification authority: a
Stage-A pass alone does not qualify the release candidate.
