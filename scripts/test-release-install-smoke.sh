#!/usr/bin/env bash
# Characterization checks for scripts/release-install-smoke.sh.
# Requires network access to crates.io for the pinned published version.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

published_version="0.1.10"
install_root="${ROOT}/target/release-install-smoke-test"

rm -rf "${install_root}"
mkdir -p "${install_root}"

INSTALL_ROOT="${install_root}" bash scripts/release-install-smoke.sh "${published_version}"

printf 'ok release-install-smoke characterization for %s\n' "${published_version}"
