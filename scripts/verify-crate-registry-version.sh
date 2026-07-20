#!/usr/bin/env bash
# Verify an exact crate version is visible on crates.io.
#
# Uses the crates.io REST API for per-version visibility, not `cargo search`
# (which returns only the latest/featured version and fails for non-latest
# checks — #2510).
#
# Usage:
#   scripts/verify-crate-registry-version.sh CRATE VERSION
set -euo pipefail

crate="${1:?crate name required}"
version="${2:?version required}"

log() {
  printf 'verify-crate-registry-version: %s\n' "$*"
}

fail() {
  printf 'verify-crate-registry-version: error: %s\n' "$*" >&2
  exit 1
}

# Query the crates.io REST API for the exact version. Returns HTTP 200 if
# the version exists and is not yanked, 404 otherwise.
status=$(curl --silent --output /dev/null --write-out '%{http_code}' \
  --connect-timeout 10 --max-time 30 \
  "https://crates.io/api/v1/crates/${crate}/${version}" 2>/dev/null || echo "000")

if [ "${status}" = "200" ]; then
  log "${crate} ${version} is visible in the crates.io index"
  exit 0
fi

fail "${crate} ${version} not visible in crates.io index (HTTP ${status})"
