# Release on Tag

Future cargo-allow releases publish from GitHub Actions when a version tag is
pushed. Manual `cargo publish` remains a documented fallback during Trusted
Publishing setup or when automation is blocked.

## Canonical Path

1. Merge release-prep PRs to `main` (version bump, release record, install pins,
   GitHub release notes draft under `docs/release/github/vX.Y.Z.md`).
2. Push an annotated tag matching the workspace version:

   ```bash
   git tag -a v0.1.10 -m "cargo-allow 0.1.10"
   git push origin v0.1.10
   ```

3. The [Release workflow](../../.github/workflows/release.yml) runs:
   - **preflight** — `fmt`, `clippy`, `cargo test --workspace`,
     `cargo package --workspace --locked`, and the default no-new guard.
   - **publish** — publishes the ten workspace crates to crates.io in dependency
     order (dry-run before each upload).
   - **github-release** — creates a GitHub Release from
     `docs/release/github/vX.Y.Z.md` when that file exists.

4. After the workflow succeeds, finish the release record in
   `docs/release/X.Y.Z.md` with workflow run id, registry visibility checks, and
   installed-binary smoke evidence.

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
10. cargo-allow
```

Each crate is dry-run verified immediately before upload. The workflow waits for
crates.io index visibility before publishing dependents.

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

## Token Fallback (Migration Only)

If Trusted Publishing is not yet configured for every crate, add a repository
secret named `CARGO_REGISTRY_TOKEN`. The publish job uses it when OIDC exchange
is unavailable. Remove the secret after Trusted Publishing is verified for all
ten crates.

Do not commit API tokens to the repository.

## Manual Dry-Run

Use workflow dispatch to validate release automation without uploading:

1. Open **Actions → Release → Run workflow** on `main`.
2. Confirm preflight passes and publish steps run `cargo publish --dry-run` only.

Tag pushes always perform real publishes once preflight succeeds.

## Manual Publish Fallback

When automation cannot run, follow the per-release record (for example
[0.1.9.md](0.1.9.md)):

```bash
rtk cargo fmt --all --check
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo test --workspace
rtk cargo package --workspace --locked
```

Then publish each crate in order:

```bash
rtk cargo publish --dry-run -p <crate> --locked
rtk cargo publish -p <crate> --locked
```

Create the GitHub Release from `docs/release/github/vX.Y.Z.md` after publication.

## Claim Boundary

The release workflow proves formatting, lint, tests, packaging, no-new policy
posture, and successful crates.io uploads for the tagged commit. It does not
execute install-smoke checks against the published binary; record those in the
release closeout after the workflow completes.
