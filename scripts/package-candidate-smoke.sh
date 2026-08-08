#!/usr/bin/env bash
# Pre-publication SourceCandidateSmoke for the cargo-allow workspace (#2256 Stage A).
#
# Proves the exact reviewed source candidate can:
#   1. package every workspace crate under --locked verification
#   2. assert packaged Cargo.toml files have no workspace path dependencies
#   3. install cargo-allow from the workspace path after packaging succeeded
#   4. run the core first-hour CLI surface from that install root
#
# This is Stage A (pre-publication). Stage B remains scripts/release-install-smoke.sh
# against crates.io after a real tag publish.
#
# Usage:
#   scripts/package-candidate-smoke.sh
#
# Optional:
#   PACKAGE_DIR=<path>   work root (default: target/package-candidate-smoke)
#   INSTALL_ROOT=<path>  cargo install --root (default: PACKAGE_DIR/install)
#   SKIP_PACKAGE=1       reuse existing PACKAGE_DIR/packages without re-packing
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

package_dir="${PACKAGE_DIR:-${ROOT}/target/package-candidate-smoke}"
install_root="${INSTALL_ROOT:-${package_dir}/install}"
packages_dir="${package_dir}/packages"
receipt="${package_dir}/package-candidate-smoke.receipt.txt"

log() {
  printf 'package-candidate-smoke: %s\n' "$*"
}

fail() {
  printf 'package-candidate-smoke: error: %s\n' "$*" >&2
  exit 1
}

read_workspace_version() {
  awk '
    /^\[workspace\.package\]/ { in_ws = 1; next }
    /^\[/ { if (in_ws) exit }
    in_ws && /^version = / {
      gsub(/^version = "/, "", $0)
      gsub(/".*$/, "", $0)
      print $0
      exit
    }
  ' Cargo.toml
}

# Read the version of a specific crate from its Cargo.toml.
# Handles both `version.workspace = true` (resolves workspace version) and
# explicit `version = "X.Y.Z"` (#2885 version split).
read_crate_version() {
  local crate="$1"
  local manifest="crates/${crate}/Cargo.toml"
  local line
  line="$(grep -m1 '^version' "${manifest}" 2>/dev/null)" || true
  if [[ "${line}" == "version.workspace = true" ]]; then
    read_workspace_version
  else
    echo "${line}" | sed 's/^version = "//; s/"$//'
  fi
}

assert_no_path_deps() {
  local tree="$1"
  local label="$2"
  # Ignore package target paths such as path = "src/lib.rs"; flag dependency
  # path tables that would still point at a workspace tree.
  local hits
  hits="$(
    grep -R --include='Cargo.toml' -nE 'path = "' "${tree}" 2>/dev/null \
      | grep -v '^Binary' \
      | grep -vE 'path = "(src|benches|examples|tests)/' \
      || true
  )"
  if [[ -n "${hits}" ]]; then
    printf '%s\n' "${hits}"
    fail "packaged ${label} still contains path dependencies"
  fi
}

version="$(read_workspace_version)"
[[ -n "${version}" ]] || fail "could not read workspace.package.version"

crates=(
  allow-core
  allow-policy
  allow-inventory
  allow-files
  allow-rust
  allow-match
  allow-report
  allow-policy-legacy
  allow-diff
  effortless-repo-protocol
  effortless-repo-edit
  cargo-allow
)

# Prefer the shared fixture when present so package smoke and ExactCandidate
# harnesses cannot drift (#2277 / #2372).
crate_set_fixture="${ROOT}/docs/dogfood/fixtures/release/candidate-crate-set.toml"
if [[ -f "${crate_set_fixture}" ]] && command -v python3 >/dev/null 2>&1; then
  mapfile -t crates < <(
    python3 - "${crate_set_fixture}" <<'PY'
import sys
from pathlib import Path
text = Path(sys.argv[1]).read_text(encoding="utf-8")
in_list = False
for line in text.splitlines():
    stripped = line.strip().strip("\r")
    if stripped.startswith("crates"):
        in_list = True
        continue
    if in_list:
        if stripped.startswith("]"):
            break
        if stripped.startswith('"'):
            name = stripped.strip(",").strip('"').strip()
            if name:
                print(name)
PY
  )
  for i in "${!crates[@]}"; do
    crates[$i]="${crates[$i]//$'\r'/}"
  done
fi

mkdir -p "${package_dir}"
: >"${receipt}"
{
  echo "workspace_version=${version}"
  echo "root=${ROOT}"
  echo "started_unix=$(date +%s)"
} >>"${receipt}"

if [[ "${SKIP_PACKAGE:-0}" != "1" ]]; then
  log "packaging workspace crates with cargo package --workspace --locked"
  rm -rf "${packages_dir}"
  mkdir -p "${packages_dir}"
  # cargo-intent depends on unpublished intent-* workspace crates; exclude until #2599-C / #2604 publish posture.
  # proof-engine depends on unpublished proof-protocol workspace crate; exclude until #2604 publish posture.
  cargo package --workspace --locked --exclude cargo-intent --exclude proof-adapter-cargo-allow --exclude proof-adapter-ripr --exclude proof-adapter-hawk --exclude proof-engine --exclude cargo-proof
  for crate in "${crates[@]}"; do
    crate_version="$(read_crate_version "${crate}")"
    crate_file="target/package/${crate}-${crate_version}.crate"
    [[ -f "${crate_file}" ]] || fail "missing packaged crate ${crate_file}"
    cp "${crate_file}" "${packages_dir}/"
    echo "packaged=${crate}-${crate_version}.crate" >>"${receipt}"
  done
else
  log "SKIP_PACKAGE=1; reusing packages under ${packages_dir}"
fi

for crate in "${crates[@]}"; do
  crate_version="$(read_crate_version "${crate}")"
  crate_file="${packages_dir}/${crate}-${crate_version}.crate"
  [[ -f "${crate_file}" ]] || fail "missing ${crate_file}"
done

log "assert packaged crates have no path dependencies"
for crate in "${crates[@]}"; do
  # Resolve per crate rather than inheriting the loop above: crates no longer
  # share one version, so a leftover value would name the wrong archive.
  crate_version="$(read_crate_version "${crate}")"
  tmp="${package_dir}/inspect-${crate}"
  rm -rf "${tmp}"
  mkdir -p "${tmp}"
  # --force-local stops GNU tar from reading a Windows drive-letter prefix
  # (e.g. C:/...) as a remote "host:path" pair. Harmless on Linux/macOS.
  tar --force-local -xzf "${packages_dir}/${crate}-${crate_version}.crate" -C "${tmp}"
  assert_no_path_deps "${tmp}" "${crate}"
  echo "no_path_deps=${crate}" >>"${receipt}"
done

log "installing cargo-allow ${version} from workspace path after package verification"
rm -rf "${install_root}"
mkdir -p "${install_root}"
cargo install --path "${ROOT}/crates/cargo-allow" --locked --root "${install_root}" --force

# Invoke the isolated binary by absolute path instead of relying on PATH
# resolution. A pre-existing global cargo-allow install (e.g. ~/.cargo/bin)
# can otherwise shadow the install root, defeating the isolation guarantee.
cargo_bin="${install_root}/bin/cargo-allow"
[[ -x "${cargo_bin}" || -x "${cargo_bin}.exe" ]] \
  || fail "expected installed binary at ${cargo_bin}(\\.exe)"

log "cargo-allow --version"
installed_version="$("${cargo_bin}" --version)"
printf '%s\n' "${installed_version}"
echo "installed_version=${installed_version}" >>"${receipt}"
printf '%s\n' "${installed_version}" | grep -F "cargo-allow ${version}" >/dev/null \
  || fail "installed version mismatch: ${installed_version} (expected cargo-allow ${version})"

log "cargo-allow doctor"
"${cargo_bin}" doctor

log "cargo-allow check --help"
"${cargo_bin}" check --help >/dev/null

log "cargo-allow list --help"
"${cargo_bin}" list --help >/dev/null

log "cargo-allow why --help"
"${cargo_bin}" why --help >/dev/null

{
  echo "completed_unix=$(date +%s)"
  echo "result=pass"
} >>"${receipt}"

log "SourceCandidateSmoke passed for workspace ${version}"
log "receipt: ${receipt}"
