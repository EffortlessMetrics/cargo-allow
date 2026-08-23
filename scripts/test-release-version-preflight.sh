#!/usr/bin/env bash
# Characterization checks for scripts/release-version-preflight.sh.
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

run_expect_success "current workspace dry-run" \
  env DRY_RUN=true bash scripts/release-version-preflight.sh "${workspace_version}"

run_expect_success "current workspace full artifacts" \
  bash scripts/release-version-preflight.sh "${workspace_version}"

run_expect_success "matching observed tag uses typed identity" \
  env DRY_RUN=true GITHUB_EVENT_NAME=push GITHUB_REF_NAME="v${workspace_version}" \
  bash scripts/release-version-preflight.sh "${workspace_version}"

run_expect_failure "observed tag mismatch fails closed" \
  env DRY_RUN=true GITHUB_EVENT_NAME=push GITHUB_REF_NAME="v9.9.9" \
  bash scripts/release-version-preflight.sh "${workspace_version}"

run_expect_failure "release version mismatch" \
  bash scripts/release-version-preflight.sh "0.0.0-missing"

grep -q 'release-identity --version' scripts/release-version-preflight.sh || {
  printf 'fail typed release identity invocation missing\n' >&2
  exit 1
}
printf 'ok typed release identity invocation retained\n'
