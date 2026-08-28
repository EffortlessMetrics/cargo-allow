#!/usr/bin/env bash
# Characterization checks for scripts/check-toolchain-profile.sh.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

run_expect_success() {
  local label="$1"
  shift
  "$@" >/dev/null || { printf 'fail %s\n' "${label}" >&2; exit 1; }
  printf 'ok %s\n' "${label}"
}

run_expect_failure() {
  local label="$1"
  shift
  if "$@" >/dev/null 2>&1; then
    printf 'fail %s (expected non-zero exit)\n' "${label}" >&2
    exit 1
  fi
  printf 'ok %s\n' "${label}"
}

run_expect_success "live toolchain profile is valid" \
  bash scripts/check-toolchain-profile.sh

for case in missing-profile missing-rustfmt missing-clippy; do
  fixture="${work}/${case}.toml"
  cp rust-toolchain.toml "${fixture}"
  case "${case}" in
    missing-profile) sed -i 's/profile = "minimal"/profile = "default"/' "${fixture}" ;;
    missing-rustfmt) sed -i 's/"rustfmt", //' "${fixture}" ;;
    missing-clippy) sed -i 's/, "clippy"//' "${fixture}" ;;
  esac
  run_expect_failure "${case} fails closed" \
    env TOOLCHAIN_FILE="${fixture}" bash scripts/check-toolchain-profile.sh
done

printf 'all check-toolchain-profile characterization checks passed\n'
