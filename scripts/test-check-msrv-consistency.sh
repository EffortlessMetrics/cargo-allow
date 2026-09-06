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

# Removing every numeric toolchain pin must fail the positive existence
# law, not just the off-MSRV rejection (#3836 review).
all_removed="${work}/all-pins-removed"
mkdir -p "${all_removed}/.github/workflows"
cp Cargo.toml "${all_removed}/Cargo.toml"
cp .github/workflows/ci.yml "${all_removed}/.github/workflows/ci.yml"
cp .github/workflows/release.yml "${all_removed}/.github/workflows/release.yml"
python3 - "${all_removed}/.github/workflows/ci.yml" "${msrv}" <<'PY'
import re
import sys

path, msrv = sys.argv[1], sys.argv[2]
with open(path, encoding="utf-8") as handle:
    text = handle.read()
stripped = re.sub("dtolnay/rust-toolchain@[0-9][^ \\\"\\n]*", "dtolnay/rust-toolchain", text)
with open(path, "w", encoding="utf-8") as handle:
    handle.write(stripped)
PY
run_expect_failure "all toolchain pins removed" \
  env CARGO_TOML="${all_removed}/Cargo.toml" \
  CI_WORKFLOW="${all_removed}/.github/workflows/ci.yml" \
  RELEASE_WORKFLOW="${all_removed}/.github/workflows/release.yml" \
  bash scripts/check-msrv-consistency.sh
drift_case "cache namespace drift" ".github/workflows/ci.yml" \
  "lane: msrv" "lane: msrv-drift"
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

# The terminator must tolerate a trailing comment on a job header. Without
# that, the extracted block runs past the commented header into the next job
# and accepts its pin — silently undoing the scoping above.
#
# The pin has to move to the job *immediately* after `msrv:` for this to
# discriminate. Parked further away, an over-running block would still exit at
# some later uncommented header before reaching it, and the case would pass
# with or without the fix.
commented_dir="${work}/relocated-pin-commented-header"
mkdir -p "${commented_dir}/.github/workflows"
cp Cargo.toml "${commented_dir}/Cargo.toml"
cp .github/workflows/release.yml "${commented_dir}/.github/workflows/release.yml"
python3 - "${commented_dir}/.github/workflows/ci.yml" \
  ".github/workflows/ci.yml" "${msrv}" <<'PY'
import re
import sys

out_path, src_path, msrv = sys.argv[1], sys.argv[2], sys.argv[3]
with open(src_path, encoding="utf-8") as handle:
    text = handle.read()

pin = f'RUSTUP_TOOLCHAIN: "{msrv}.0"'
if pin not in text:
    sys.exit(f"fixture setup failed: {pin!r} not present in {src_path}")
text = text.replace(f"    env:\n      {pin}\n", "", 1)
if pin in text:
    sys.exit("fixture setup failed: msrv pin was not removed")

match = re.search(r"^  msrv:[ \t]*$", text, re.MULTILINE)
if match is None:
    sys.exit("fixture setup failed: no msrv job header")
following = re.search(r"^  ([A-Za-z0-9_-]+):[ \t]*$", text[match.end():], re.MULTILINE)
if following is None:
    sys.exit("fixture setup failed: no job header after msrv")

start = match.end() + following.start()
end = match.end() + following.end()
# Comment the terminator header and graft the pin onto that adjacent job.
text = (
    text[:start]
    + f"  {following.group(1)}:  # trailing comment\n    env:\n      {pin}"
    + text[end:]
)

with open(out_path, "w", encoding="utf-8") as handle:
    handle.write(text)
PY
run_expect_failure "relocated pin with a commented terminator header" \
  env CARGO_TOML="${commented_dir}/Cargo.toml" \
  CI_WORKFLOW="${commented_dir}/.github/workflows/ci.yml" \
  RELEASE_WORKFLOW="${commented_dir}/.github/workflows/release.yml" \
  bash scripts/check-msrv-consistency.sh

# A trailing comment on the `msrv:` header itself must still locate the job.
commented_msrv_dir="${work}/commented-msrv-header"
mkdir -p "${commented_msrv_dir}/.github/workflows"
cp Cargo.toml "${commented_msrv_dir}/Cargo.toml"
cp .github/workflows/release.yml "${commented_msrv_dir}/.github/workflows/release.yml"
python3 - "${commented_msrv_dir}/.github/workflows/ci.yml" \
  ".github/workflows/ci.yml" <<'PY'
import re
import sys

out_path, src_path = sys.argv[1], sys.argv[2]
with open(src_path, encoding="utf-8") as handle:
    text = handle.read()
text, count = re.subn(r"^  msrv:[ \t]*$", "  msrv:  # MSRV lane", text, count=1, flags=re.MULTILINE)
if count != 1:
    sys.exit("fixture setup failed: no msrv job header")
with open(out_path, "w", encoding="utf-8") as handle:
    handle.write(text)
PY
run_expect_success "commented msrv header still locates the job" \
  env CARGO_TOML="${commented_msrv_dir}/Cargo.toml" \
  CI_WORKFLOW="${commented_msrv_dir}/.github/workflows/ci.yml" \
  RELEASE_WORKFLOW="${commented_msrv_dir}/.github/workflows/release.yml" \
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
