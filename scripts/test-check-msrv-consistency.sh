#!/usr/bin/env bash
# Characterization checks for scripts/check-msrv-consistency.sh.
#
# Proves the guard passes on the live repository and fails closed on each
# drift site independently.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

run_expect_success() {
  local label="$1"
  shift
  if "$@" >/dev/null 2>&1; then
    printf 'ok %s\n' "${label}"
  else
    printf 'fail %s\n' "${label}" >&2
    exit 1
  fi
}

run_expect_failure() {
  local label="$1"
  shift
  if "$@" >/dev/null 2>&1; then
    printf 'fail %s (expected non-zero exit)\n' "${label}" >&2
    exit 1
  else
    printf 'ok %s\n' "${label}"
  fi
}

run_expect_success "live repository is consistent" \
  bash scripts/check-msrv-consistency.sh

# Each drift case copies the real files, corrupts exactly one, and expects a
# non-zero exit. Corrupting the workflows (rather than Cargo.toml) mirrors the
# real failure mode: an automated action bump moving off the claimed MSRV.
drift_case() {
  local label="$1" file="$2" from="$3" to="$4"
  local dir="${work}/${label// /-}"
  mkdir -p "${dir}/.github/workflows"
  cp Cargo.toml "${dir}/Cargo.toml"
  cp .github/workflows/ci.yml "${dir}/.github/workflows/ci.yml"
  cp .github/workflows/release.yml "${dir}/.github/workflows/release.yml"
  # `from` and `to` are literal fixed strings, not patterns.
  python3 - "${dir}/${file}" "${from}" "${to}" <<'PY'
import sys
path, old, new = sys.argv[1], sys.argv[2], sys.argv[3]
with open(path, encoding="utf-8") as handle:
    text = handle.read()
if old not in text:
    sys.exit(f"fixture setup failed: {old!r} not present in {path}")
with open(path, "w", encoding="utf-8") as handle:
    handle.write(text.replace(old, new, 1))
PY
  run_expect_failure "${label}" \
    env CARGO_TOML="${dir}/Cargo.toml" \
    CI_WORKFLOW="${dir}/.github/workflows/ci.yml" \
    RELEASE_WORKFLOW="${dir}/.github/workflows/release.yml" \
    bash scripts/check-msrv-consistency.sh
}

msrv="$(awk '
  /^\[workspace\.package\]/ { in_section = 1; next }
  /^\[/ { if (in_section) exit }
  in_section && /^[[:space:]]*rust-version[[:space:]]*=[[:space:]]*"/ {
    gsub(/^[[:space:]]*rust-version[[:space:]]*=[[:space:]]*"/, "")
    gsub(/".*/, "")
    print
    exit
  }
' Cargo.toml)"

drift_case "toolchain pin drift" ".github/workflows/ci.yml" \
  "dtolnay/rust-toolchain@${msrv}.0" "dtolnay/rust-toolchain@1.100.0"
drift_case "cache key drift" ".github/workflows/ci.yml" \
  "key: msrv-${msrv}" "key: msrv-1.100"
drift_case "attested release manifest drift" ".github/workflows/release.yml" \
  "MSRV: \"${msrv}\"" "MSRV: \"1.100\""

# Reformatting a matching site must not be reported as drift. Without these,
# the tolerant patterns in the guard would be untested and could silently
# regress to exact-spacing matching.
accepts_case() {
  local label="$1" file="$2" from="$3" to="$4"
  local dir="${work}/accept-${label// /-}"
  mkdir -p "${dir}/.github/workflows"
  cp Cargo.toml "${dir}/Cargo.toml"
  cp .github/workflows/ci.yml "${dir}/.github/workflows/ci.yml"
  cp .github/workflows/release.yml "${dir}/.github/workflows/release.yml"
  python3 - "${dir}/${file}" "${from}" "${to}" <<'PY'
import sys
path, old, new = sys.argv[1], sys.argv[2], sys.argv[3]
with open(path, encoding="utf-8") as handle:
    text = handle.read()
if old not in text:
    sys.exit(f"fixture setup failed: {old!r} not present in {path}")
with open(path, "w", encoding="utf-8") as handle:
    handle.write(text.replace(old, new, 1))
PY
  run_expect_success "${label}" \
    env CARGO_TOML="${dir}/Cargo.toml" \
    CI_WORKFLOW="${dir}/.github/workflows/ci.yml" \
    RELEASE_WORKFLOW="${dir}/.github/workflows/release.yml" \
    bash scripts/check-msrv-consistency.sh
}

accepts_case "reformatted rust-version is still parsed" "Cargo.toml" \
  "rust-version = \"${msrv}\"" "rust-version   =    \"${msrv}\""
accepts_case "reformatted release MSRV is still matched" ".github/workflows/release.yml" \
  "MSRV: \"${msrv}\"" "MSRV:   \"${msrv}\""

# The MSRV is interpolated into an ERE, where an unescaped `.` matches any
# character. A same-shape value must not be accepted as a match.
dotted="${msrv/./x}"
if [[ "${dotted}" != "${msrv}" ]]; then
  drift_case "regex dot is literal, not a wildcard" ".github/workflows/release.yml" \
    "MSRV: \"${msrv}\"" "MSRV: \"${dotted}\""
fi

printf 'all check-msrv-consistency characterization checks passed\n'
