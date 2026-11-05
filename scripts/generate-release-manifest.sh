#!/usr/bin/env bash
# Thin local convenience wrapper for the typed release-manifest command
# (#2497). The semantic authority is `cargo-allow release-manifest`: this
# script only translates the documented environment contract into typed
# arguments and invokes the Rust command. It performs no version, channel,
# equality, or result decisions of its own.
#
# Inputs (environment):
#   VERSION        workspace version (e.g. 0.2.0)            [required]
#   REPOSITORY     GitHub repository full name                [required]
#   TAG            git tag ref (e.g. v0.2.0)                  [required]
#   COMMIT         commit SHA                                 [required]
#   TREE           tree SHA                                   [required]
#   AUTH_SOURCE    authentication class (crates_io_api_token) [required]
#   AUTHORIZATION_DIGEST  compiled one-use authorization digest
#   WORKFLOW_RUN_ID  GitHub Actions run id
#   PLATFORMS      space-separated proven platforms (default: linux)
#   RUST_TOOLCHAIN  toolchain used for the binary (default: stable)
#   RUNNER          runner used for the binary (default: ubuntu-latest)
#   BINARY_PACKAGE_RECEIPT  verified binary package receipt
#   BINARY_INSTALL_RECEIPT  clean-install receipt
#   TOPOLOGY_RECEIPT  exact topology publisher receipt
#   REQUIRE_PUBLISHED_TOPOLOGY  true for the public release entry
#   EXPECTED_TOPOLOGY_RECEIPT_SHA256  digest bound at download
#   REQUIRED_RESULT  typed result name the command must produce (e.g.
#                    complete); the command fails closed otherwise
#   OUTPUT         output path (default: target/cargo-allow/release-manifest-v2.json)
#   RESULT_OUT     typed result receipt path
#   GITHUB_OUTPUT  GitHub Actions output file; receives result=<name>
#   MSRV           retained as release evidence for
#                  scripts/check-msrv-consistency.sh; the typed manifest
#                  does not consume it
#
# Outputs:
#   <OUTPUT> — the typed ReleaseManifestV2 envelope
#   <OUTPUT>.sha256 — SHA-256 checksum record of the manifest bytes
#   <RESULT_OUT> — cargo-allow.release-manifest-result.v1 typed receipt
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

version="${VERSION:?VERSION is required}"
repository="${REPOSITORY:?REPOSITORY is required}"
tag="${TAG:?TAG is required}"
commit="${COMMIT:?COMMIT is required}"
tree="${TREE:?TREE is required}"
auth_source="${AUTH_SOURCE:?AUTH_SOURCE is required}"
msrv="${MSRV:?MSRV is required}"
command -v cargo >/dev/null 2>&1 || {
  printf 'release-manifest: error: cargo is required to run the typed authority\n' >&2
  exit 1
}

args=(
  release-manifest
  --version "${version}"
  --repository "${repository}"
  --tag "${tag}"
  --commit "${commit}"
  --tree "${tree}"
  --auth-source "${auth_source}"
)
[ -n "${WORKFLOW_RUN_ID:-}" ] && args+=(--workflow-run-id "${WORKFLOW_RUN_ID}")
[ -n "${BINARY_PACKAGE_RECEIPT:-}" ] && args+=(--binary-package-receipt "${BINARY_PACKAGE_RECEIPT}")
[ -n "${BINARY_INSTALL_RECEIPT:-}" ] && args+=(--binary-install-receipt "${BINARY_INSTALL_RECEIPT}")
[ -n "${TOPOLOGY_RECEIPT:-}" ] && args+=(--topology-receipt "${TOPOLOGY_RECEIPT}")
[ "${REQUIRE_PUBLISHED_TOPOLOGY:-false}" = "true" ] && args+=(--require-published-topology)
[ -n "${EXPECTED_TOPOLOGY_RECEIPT_SHA256:-}" ] && args+=(--expected-topology-receipt-sha256 "${EXPECTED_TOPOLOGY_RECEIPT_SHA256}")
[ -n "${PLATFORMS:-}" ] && args+=(--platforms "${PLATFORMS}")
[ -n "${RUST_TOOLCHAIN:-}" ] && args+=(--rust-toolchain "${RUST_TOOLCHAIN}")
[ -n "${RUNNER:-}" ] && args+=(--runner "${RUNNER}")
[ -n "${REQUIRED_RESULT:-}" ] && args+=(--require-result "${REQUIRED_RESULT}")
[ -n "${OUTPUT:-}" ] && args+=(--output "${OUTPUT}")
[ -n "${RESULT_OUT:-}" ] && args+=(--result-out "${RESULT_OUT}")
[ -n "${GITHUB_OUTPUT:-}" ] && args+=(--github-output "${GITHUB_OUTPUT}")

cargo run --quiet -p cargo-allow --locked -- "${args[@]}"
