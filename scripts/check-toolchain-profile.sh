#!/usr/bin/env bash
# Verify the repository's installation profile and explicitly required tools.
# This is configuration evidence; it does not claim an MSRV or a channel pin.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
toolchain_file="${TOOLCHAIN_FILE:-${ROOT}/rust-toolchain.toml}"

[[ -f "${toolchain_file}" ]] || {
  printf 'toolchain profile check: missing %s\n' "${toolchain_file}" >&2
  exit 1
}

grep -qE '^[[:space:]]*profile[[:space:]]*=[[:space:]]*"minimal"[[:space:]]*$' "${toolchain_file}" || {
  printf 'toolchain profile check: profile must be "minimal" in %s\n' "${toolchain_file}" >&2
  exit 1
}

for component in rustfmt clippy; do
  grep -qE "^[[:space:]]*components[[:space:]]*=.*\\\"${component}\\\"" "${toolchain_file}" || {
    printf 'toolchain profile check: required component %s is missing in %s\n' "${component}" "${toolchain_file}" >&2
    exit 1
  }
done

printf 'ok toolchain profile=minimal components=rustfmt,clippy\n'
