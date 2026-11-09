#!/usr/bin/env bash
# Post-publish install smoke for cargo-allow from crates.io.
#
# Usage:
#   scripts/release-install-smoke.sh VERSION
#
# Optional:
#   INSTALL_ROOT=<path>  install with cargo install --root instead of default
#
# The version arrives as an exact value from the caller (the release
# workflow passes the typed release-identity projection output); this
# script adds no release grammar. The installed binary is resolved to one
# absolute path and invoked through it — no PATH substitution and no
# source-checkout fallback — and must self-report exactly the installed
# identity (#3787).
set -euo pipefail

version="${1:?version required}"

log() {
  printf 'release-install-smoke: %s\n' "$*"
}

fail() {
  printf 'release-install-smoke: error: %s\n' "$*" >&2
  exit 1
}

install_args=(cargo-allow --version "${version}" --locked)
if [[ -n "${INSTALL_ROOT:-}" ]]; then
  install_args+=(--root "${INSTALL_ROOT}")
fi

log "installing cargo-allow ${version} from crates.io"
cargo install "${install_args[@]}"

# Resolve the installed binary to one absolute path. With INSTALL_ROOT the
# location is exact; without it cargo installs into its default bin
# directory, which must already be on PATH.
if [[ -n "${INSTALL_ROOT:-}" ]]; then
  bin="${INSTALL_ROOT}/bin/cargo-allow"
else
  bin="$(command -v cargo-allow)" || fail "cargo-allow not found after install"
fi
[[ "${bin}" == /* ]] || fail "installed binary path is not absolute: ${bin}"
[[ -x "${bin}" ]] || fail "installed binary is not executable: ${bin}"

log "cargo-allow --version (${bin})"
reported_version="$("${bin}" --version)"
[[ "${reported_version}" == "cargo-allow ${version}" ]] \
  || fail "installed binary reported ${reported_version}, expected cargo-allow ${version}"

log "cargo-allow doctor"
"${bin}" doctor

log "cargo-allow check --help"
"${bin}" check --help >/dev/null

log "cargo-allow doctor --profile spec-system --help"
"${bin}" doctor --profile spec-system --help >/dev/null

log "install smoke passed for cargo-allow ${version} at ${bin}"
