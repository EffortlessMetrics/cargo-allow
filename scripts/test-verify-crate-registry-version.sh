#!/usr/bin/env bash
# Characterization checks for scripts/verify-crate-registry-version.sh.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

run_expect_success() {
  local label="$1"
  shift
  if "$@"; then
    printf 'ok %s\n' "${label}"
  else
    printf 'fail %s\n' "${label}" >&2
    exit 1
  fi
}

run_expect_failure() {
  local label="$1"
  shift
  if "$@"; then
    printf 'fail %s (expected non-zero exit)\n' "${label}" >&2
    exit 1
  else
    printf 'ok %s\n' "${label}"
  fi
}

workspace_version="$(awk '/^\[workspace\.package\]/{f=1;next} /^\[/{if(f) exit} f && /^version = /{gsub(/version = "/,""); gsub(/".*/,""); print; exit}' Cargo.toml)"

run_expect_success "published allow-core@${workspace_version}" \
  bash scripts/verify-crate-registry-version.sh allow-core "${workspace_version}"

run_expect_failure "missing version" \
  bash scripts/verify-crate-registry-version.sh allow-core "0.0.0-nonexistent-verify"
