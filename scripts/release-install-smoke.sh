#!/usr/bin/env bash
# Post-publish install smoke for cargo-allow from crates.io.
#
# Usage:
#   scripts/release-install-smoke.sh VERSION
#
# Optional:
#   INSTALL_ROOT=<path>  install with cargo install --root instead of default
set -euo pipefail

version="${1:?version required}"

log() {
  printf 'release-install-smoke: %s\n' "$*"
}

install_args=(cargo-allow --version "${version}" --locked)
if [[ -n "${INSTALL_ROOT:-}" ]]; then
  install_args+=(--root "${INSTALL_ROOT}")
fi

log "installing cargo-allow ${version} from crates.io"
cargo install "${install_args[@]}"

if [[ -n "${INSTALL_ROOT:-}" ]]; then
  export PATH="${INSTALL_ROOT}/bin:${PATH}"
fi

log "cargo-allow --version"
cargo-allow --version

log "cargo-allow doctor"
cargo-allow doctor

log "cargo-allow check --help"
cargo-allow check --help >/dev/null

log "cargo-allow doctor --profile spec-system --help"
cargo-allow doctor --profile spec-system --help >/dev/null

log "install smoke passed for cargo-allow ${version}"
