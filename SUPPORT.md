# Support

This page states what the cargo-allow repository currently **proves**, not
what it aspires to. Every claim below is backed by an executed CI job or a
file in this repository, and the machine-readable source is
[`docs/support-matrix.toml`](docs/support-matrix.toml).

`crates/cargo-allow/tests/support_matrix.rs` fails closed if any row here
drifts from the repository it describes.

## Versions

| Channel | Version | Installable | Evidence |
| --- | --- | --- | --- |
| Published | `0.1.11` | yes | `cargo install cargo-allow --version 0.1.11 --locked` |
| Source candidate | `0.2.0` | **no** | workspace version on `main`; deliberately unpublished pending the 0.2.0 blocker set ([#2501](https://github.com/EffortlessMetrics/cargo-allow/issues/2501), [`docs/release/0.2.0.md`](docs/release/0.2.0.md)) |

Installing from `main` produces `cargo-allow 0.2.0`, which is a pre-release
candidate. Do not treat it as a published release.

## Rust version

**MSRV: 1.95**, declared as `rust-version` under `[workspace.package]`.

CI proves it: the `msrv` job pins `dtolnay/rust-toolchain@1.95.0`, sets
`RUSTUP_TOOLCHAIN=1.95.0` so the repository's `rust-toolchain.toml` cannot
override that pin, and runs `cargo check --locked --all-targets` over the
thirteen-package cargo-allow release set, plus
`cargo test -p cargo-allow --bins --locked`. The full suite additionally runs
on stable.

The MSRV check is scoped to the release set, not the whole workspace (#3358):
the cargo-intent and cargo-proof packages claim no toolchain yet and are
proven only on stable. **The MSRV claim therefore covers the cargo-allow
release set, not every package in the repository.**

Two guards keep that honest, because a pin can be stated correctly and still be
overridden at run time. `scripts/check-msrv-consistency.sh` proves the MSRV is
stated identically in the CI pin, the build cache key, the attested release
manifest, and the job's `RUSTUP_TOOLCHAIN`. `scripts/check-msrv-resolved.sh`
then asks the resolved compiler what it is and fails the job unless it reports
the declared MSRV series.

## Platforms

`ci_proven` means the workspace test suite executes on that runner on every
pull request. `ci_proven_bounded` means the release-set suite executes with a
named fixture exclusion. Both are statements of executed evidence, not support
promises — see *Not yet decided* below.

| Target | Tier | Evidence |
| --- | --- | --- |
| `x86_64-unknown-linux-gnu` | ci_proven | `test`, `msrv`, `package-smoke`, `shallow-diff-smoke` jobs on `ubuntu-latest`; release install-smoke on tag push |
| `x86_64-pc-windows-msvc` | ci_proven | `test-core-platforms` runs the full release set; release install-smoke on tag push |
| `aarch64-apple-darwin` | ci_proven_bounded | `test-core-platforms` runs the release set and semantic fixture on `macos-14`; persistent scan-cache fixture is excluded |
| `x86_64-apple-darwin` | ci_proven_bounded | `test-core-platforms` runs the release set and semantic fixture on `macos-15-intel`; persistent scan-cache fixture is excluded |

macOS is listed explicitly rather than omitted. cargo-allow may well build
there; this repository executes nothing that proves it.

## Installation channels

| Channel | Available | Notes |
| --- | --- | --- |
| crates.io | yes | published via Trusted Publishing; install verified by `scripts/release-install-smoke.sh` |
| Prebuilt binary archives | no | tracked by [#2464](https://github.com/EffortlessMetrics/cargo-allow/issues/2464) / [#2474](https://github.com/EffortlessMetrics/cargo-allow/issues/2474) |
| `cargo-binstall` | no | tracked by [#2481](https://github.com/EffortlessMetrics/cargo-allow/issues/2481); depends on the archives |

## Artifact schemas

All emitted artifacts are at **generation 1** (`*.v1`). The complete list is
in the machine-readable matrix and is asserted against
`allow_report::ARTIFACT_CONTRACTS`, so it cannot fall behind the code.

## Security

Vulnerability reporting is documented in [`SECURITY.md`](SECURITY.md).

## Not yet decided

These are support **policy** questions. They are listed as open rather than
given plausible defaults, because publishing a commitment the project has not
agreed to would be worse than publishing none:

- how many published releases receive fixes;
- acknowledgement and patch timeframes for a reported vulnerability;
- whether fixes are backported to the previous minor, or fix-forward only;
- which `ci_proven` platforms are *committed* support versus best-effort;
- whether raising the MSRV requires a minor bump, and with what notice.

Until these are decided, treat the tables above as a description of executed
evidence, not as a guarantee.
