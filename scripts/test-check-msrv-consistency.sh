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
# The toolchain tag is applied via `rustup default`, which rust-toolchain.toml
# outranks. RUSTUP_TOOLCHAIN is what actually pins the lane, so moving it off
# the MSRV must fail even though the tag and cache key above still match.
drift_case "rustup toolchain override drift" ".github/workflows/ci.yml" \
  "RUSTUP_TOOLCHAIN: \"${msrv}.0\"" "RUSTUP_TOOLCHAIN: \"1.100.0\""

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

accepts_case "reformatted RUSTUP_TOOLCHAIN is still matched" ".github/workflows/ci.yml" \
  "RUSTUP_TOOLCHAIN: \"${msrv}.0\"" "RUSTUP_TOOLCHAIN:   ${msrv}.0"

# Deleting the pin outright, not just moving it, must also fail: the tag and
# cache key stay correct, so nothing else in the guard would notice.
drift_case "rustup toolchain override removed" ".github/workflows/ci.yml" \
  "RUSTUP_TOOLCHAIN: \"${msrv}.0\"" "UNRELATED_ENV: \"${msrv}.0\""

# Relocating the pin to a different job must fail. A whole-file search would
# accept this: the correct string is still present, just on a lane that is not
# the MSRV proof, leaving msrv resolving whatever the runner's channel is.
relocated_dir="${work}/relocated-pin"
mkdir -p "${relocated_dir}/.github/workflows"
cp Cargo.toml "${relocated_dir}/Cargo.toml"
cp .github/workflows/release.yml "${relocated_dir}/.github/workflows/release.yml"
python3 - "${relocated_dir}/.github/workflows/ci.yml" \
  ".github/workflows/ci.yml" "${msrv}" <<'PY'
import re
import sys

out_path, src_path, msrv = sys.argv[1], sys.argv[2], sys.argv[3]
with open(src_path, encoding="utf-8") as handle:
    text = handle.read()

pin = f'RUSTUP_TOOLCHAIN: "{msrv}.0"'
if pin not in text:
    sys.exit(f"fixture setup failed: {pin!r} not present in {src_path}")

# Strip the msrv job's env block, then graft the same pin onto another job so
# the string still exists in the file but not where it counts.
text = text.replace(f"    env:\n      {pin}\n", "", 1)
if pin in text:
    sys.exit("fixture setup failed: msrv pin was not removed")

target = re.search(r"^  cargo-deny:[ \t]*$", text, re.MULTILINE)
if target is None:
    sys.exit("fixture setup failed: no cargo-deny job to relocate the pin onto")
insert_at = target.end() + 1
text = text[:insert_at] + f"    env:\n      {pin}\n" + text[insert_at:]

with open(out_path, "w", encoding="utf-8") as handle:
    handle.write(text)
PY
run_expect_failure "rustup toolchain override relocated to another job" \
  env CARGO_TOML="${relocated_dir}/Cargo.toml" \
  CI_WORKFLOW="${relocated_dir}/.github/workflows/ci.yml" \
  RELEASE_WORKFLOW="${relocated_dir}/.github/workflows/release.yml" \
  bash scripts/check-msrv-consistency.sh

# The override requirement exists only because a toolchain file outranks the
# workflow tag. A repository without one must not be asked for it.
toolchain_relaxation_dir="${work}/no-toolchain-file"
mkdir -p "${toolchain_relaxation_dir}/.github/workflows"
cp Cargo.toml "${toolchain_relaxation_dir}/Cargo.toml"
cp .github/workflows/release.yml "${toolchain_relaxation_dir}/.github/workflows/release.yml"
python3 - "${toolchain_relaxation_dir}/.github/workflows/ci.yml" \
  ".github/workflows/ci.yml" "RUSTUP_TOOLCHAIN: \"${msrv}.0\"" <<'PY'
import sys
out_path, src_path, needle = sys.argv[1], sys.argv[2], sys.argv[3]
with open(src_path, encoding="utf-8") as handle:
    text = handle.read()
if needle not in text:
    sys.exit(f"fixture setup failed: {needle!r} not present in {src_path}")
with open(out_path, "w", encoding="utf-8") as handle:
    handle.write(text.replace(needle, 'UNRELATED_ENV: "unset"', 1))
PY
run_expect_success "no toolchain file relaxes the override requirement" \
  env CARGO_TOML="${toolchain_relaxation_dir}/Cargo.toml" \
  CI_WORKFLOW="${toolchain_relaxation_dir}/.github/workflows/ci.yml" \
  RELEASE_WORKFLOW="${toolchain_relaxation_dir}/.github/workflows/release.yml" \
  TOOLCHAIN_FILE="${toolchain_relaxation_dir}/rust-toolchain.toml" \
  bash scripts/check-msrv-consistency.sh

printf 'all check-msrv-consistency characterization checks passed\n'
