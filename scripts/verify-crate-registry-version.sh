#!/usr/bin/env bash
# Verify an exact crate version is visible on crates.io.
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

search_line="$(cargo search "${crate}" --limit 1 2>/dev/null | head -n 1 || true)"
[[ -n "${search_line}" ]] || fail "no search results for ${crate}"

expected_prefix="${crate} = \"${version}\""
if [[ "${search_line}" == "${expected_prefix}"* ]]; then
  log "${crate} ${version} is visible in the crates.io index"
  exit 0
fi

fail "${crate} ${version} not visible in crates.io index (latest line: ${search_line})"
