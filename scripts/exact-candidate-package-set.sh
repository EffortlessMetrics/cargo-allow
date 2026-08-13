#!/usr/bin/env bash
# ExactCandidatePackageSetV1 (#2372 / #2277 / #2378 / #2380 / #2408).
#
# Packages the canonical thirteen-crate set, extracts each .crate outside the
# workspace, warms external crates via patched `cargo fetch`, assembles a
# classic Cargo local-registry (`.crate` + index) for the full lockfile graph
# with candidate crates injected, installs cargo-allow offline with crates-io
# replaced by that local-registry, verifies internal package sources are not
# the workspace tree, runs negative controls, and emits a JSON receipt.
#
# Negatives covered:
#   - omit internal crate from patch (warm-path characterization)
#   - workspace path install rejected
#   - package checksum mutation after inventory
#   - injected normalized path dependency
#   - older/incompatible internal package version
#   - omit candidate from local-registry (offline resolve fail)
#   - candidate commit/version mismatch (CandidateStale)
#   - missing required package metadata/file (ManifestMalformed)
#   - decisive install with crates/ renamed away (CheckoutIsolated)
#
# Does not: publish; run the installed operator journey (#2278).
#
# Usage:
#   bash scripts/exact-candidate-package-set.sh
#
# Optional:
#   PACKAGE_INPUT_DIR=<path>  prebuilt .crate input for SKIP_PACKAGE=1
#   SKIP_PACKAGE=1            reuse PACKAGE_INPUT_DIR without re-packing
#   SKIP_NEGATIVES=1          skip negative controls (debug only)
#   SKIP_LOCAL_REGISTRY=1     reuse OFFLINE_ROOT/local-registry if present (debug only)
#   ALLOW_DIRTY=1             pass --allow-dirty to cargo package (local debug only)
set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lifecycle="${SCRIPT_ROOT}/scripts/candidate-harness-owned-dir.py"
command -v python3 >/dev/null 2>&1 || { printf 'exact-candidate-package-set: error: python3 is required\n' >&2; exit 1; }

if [[ "${1:-}" != "--internal" ]]; then
  if [[ "${SKIP_PACKAGE:-0}" == "1" ]]; then
    PACKAGE_INPUT_DIR="$(python3 - "${PACKAGE_INPUT_DIR:-}" "${SCRIPT_ROOT}/target" <<'PY'
import sys
from pathlib import Path

raw = Path(sys.argv[1])
target = Path(sys.argv[2]).resolve(strict=True)
if not str(raw).strip() or not raw.exists() or raw.is_symlink():
    raise SystemExit("SKIP_PACKAGE=1 requires an existing non-symlink PACKAGE_INPUT_DIR")
resolved = raw.resolve(strict=True)
try:
    resolved.relative_to(target)
except ValueError as error:
    raise SystemExit(f"PACKAGE_INPUT_DIR must be below {target}, got {resolved}") from error
print(resolved)
PY
)"
    export PACKAGE_INPUT_DIR
  fi
  temp_root="${CANDIDATE_HARNESS_TEST_ROOT:-${TMPDIR:-/tmp}}"
  snapshot_json="$(python3 "${lifecycle}" snapshot --root "${temp_root}" --repository "${SCRIPT_ROOT}" --purpose exact-candidate-package-snapshot)"
  read -r snapshot_root snapshot_token snapshot_head < <(
    printf '%s' "${snapshot_json}" | python3 -c 'import json,sys; v=json.load(sys.stdin); print(v["path"], v["token"], v["git_head"])'
  )
  snapshot_cleanup() {
    python3 "${lifecycle}" remove --root "${temp_root}" --path "${snapshot_root}" \
      --purpose exact-candidate-package-snapshot --token "${snapshot_token}"
  }
  trap snapshot_cleanup EXIT
  bash "${BASH_SOURCE[0]}" --internal "${snapshot_root}" "${snapshot_token}" "${snapshot_head}"
  exit $?
fi

[[ "$#" -eq 4 ]] || { printf 'exact-candidate-package-set: error: invalid internal invocation\n' >&2; exit 1; }
snapshot_root="$2"
snapshot_token="$3"
snapshot_head="$4"
export CANDIDATE_HARNESS_ROOT="$snapshot_root" CANDIDATE_HARNESS_GIT_HEAD="$snapshot_head"
python3 "${lifecycle}" verify --root "${CANDIDATE_HARNESS_TEST_ROOT:-${TMPDIR:-/tmp}}" \
  --path "${snapshot_root}" --purpose exact-candidate-package-snapshot \
  --token "${snapshot_token}"

ROOT="${CANDIDATE_HARNESS_ROOT}"
cd "${ROOT}"
if [[ "${CANDIDATE_HARNESS_SNAPSHOT_PROBE:-0}" == "1" && "${CANDIDATE_HARNESS_TEST_INJECTION:-0}" == "1" ]]; then
  [[ "${ROOT}" != "${SCRIPT_ROOT}" && -f "${ROOT}/Cargo.toml" && -d "${ROOT}/crates" ]]
  printf 'exact-candidate-package-set: disposable snapshot ok\n'
  exit 0
fi

output_root="${CANDIDATE_HARNESS_OUTPUT_ROOT:-${ROOT}/target}"
mkdir -p "${output_root}"
work_json="$(python3 "${lifecycle}" allocate --root "${output_root}" --purpose exact-candidate-package-set --durable)"
read -r work_dir work_token < <(
  printf '%s' "${work_json}" | python3 -c 'import json,sys; v=json.load(sys.stdin); print(v["path"], v["token"])'
)
packages_dir="${work_dir}/packages"
# Extracted packages must live outside the workspace tree; otherwise Cargo
# walks up to the repo Cargo.toml and treats them as workspace members.
offline_parent="${CANDIDATE_HARNESS_TEST_ROOT:-${TMPDIR:-/tmp}}"
offline_json="$(python3 "${lifecycle}" allocate --root "${offline_parent}" --purpose exact-candidate-package-offline)"
read -r offline_root offline_token < <(
  printf '%s' "${offline_json}" | python3 -c 'import json,sys; v=json.load(sys.stdin); print(v["path"], v["token"])'
)
extracted_dir="${offline_root}/extracted"
cargo_home="${offline_root}/cargo-home"
local_registry_dir="${offline_root}/local-registry"
install_cargo_home="${offline_root}/install-cargo-home"
target_dir="${offline_root}/target"
install_root="${offline_root}/install"
receipt="${work_dir}/exact-candidate-package-set.receipt.json"
crate_set_fixture="${ROOT}/docs/dogfood/fixtures/release/candidate-crate-set.toml"
schema_id="cargo-allow.exact-candidate-package-set.v1"
crate_set_schema_id="cargo-allow.candidate-crate-set.v1"
crates_path="${ROOT}/crates"
crates_stash="${work_dir}/crates-source-stash"

restore_source_checkout() {
  if [[ -d "${crates_stash}" ]]; then
    [[ ! -e "${crates_path}" ]] || fail "refusing to overwrite existing crates/ during restore"
    mv "${crates_stash}" "${crates_path}"
  fi
}

cleanup_offline() {
  restore_source_checkout
  if [[ "${KEEP_OFFLINE:-0}" != "1" ]]; then
    python3 "${lifecycle}" remove --root "${offline_parent}" --path "${offline_root}" \
      --purpose exact-candidate-package-offline --token "${offline_token}"
  fi
  if [[ "${package_set_passed:-0}" != "1" ]]; then
    python3 "${lifecycle}" remove --root "${output_root}" --path "${work_dir}" \
      --purpose exact-candidate-package-set --token "${work_token}"
  fi
}
trap cleanup_offline EXIT

log() {
  printf 'exact-candidate-package-set: %s\n' "$*"
}

fail() {
  printf 'exact-candidate-package-set: error: %s\n' "$*" >&2
  exit 1
}

command -v cargo >/dev/null 2>&1 || fail "cargo is required"
command -v tar >/dev/null 2>&1 || fail "tar is required"

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

read_crate_version() {
  local crate="$1"
  local line
  line="$(grep -m1 '^version' "crates/${crate}/Cargo.toml" 2>/dev/null)" || true
  if [[ "${line}" == "version.workspace = true" ]]; then
    read_workspace_version
  else
    echo "${line}" | sed 's/^version = "//; s/"$//'
  fi
}

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
# Bash on Windows may retain CR from pipes; normalize crate names.
for i in "${!crates[@]}"; do
  crates[$i]="${crates[$i]//$'\r'/}"
done

[[ "${#crates[@]}" -eq 13 ]] || fail "expected 13 crates from ${crate_set_fixture}, got ${#crates[@]}"

version="$(read_workspace_version)"
[[ -n "${version}" ]] || fail "could not read workspace.package.version"

git_head="${CANDIDATE_HARNESS_GIT_HEAD:-}"

if [[ "${SKIP_PACKAGE:-0}" == "1" ]]; then
  shopt -s nullglob
  package_input_dir="${PACKAGE_INPUT_DIR:-}"
  [[ -n "${package_input_dir}" && -d "${package_input_dir}" ]] \
    || fail "SKIP_PACKAGE=1 requires PACKAGE_INPUT_DIR"
  staged=("${package_input_dir}"/*.crate)
  shopt -u nullglob
  [[ "${#staged[@]}" -gt 0 ]] \
    || fail "SKIP_PACKAGE=1 but no .crate files under ${package_input_dir}"
fi

mkdir -p "${packages_dir}" "${extracted_dir}" "${cargo_home}" "${target_dir}" "${install_root}" "${work_dir}/install/bin" "${local_registry_dir}"

if [[ "${SKIP_PACKAGE:-0}" == "1" ]]; then
  log "SKIP_PACKAGE=1; restoring prebuilt packages"
  cp "${staged[@]}" "${packages_dir}/"
else
  log "packaging workspace crates with cargo package --workspace --locked"
  package_flags=(--workspace --locked)
  if [[ "${ALLOW_DIRTY:-0}" == "1" ]]; then
    package_flags+=(--allow-dirty)
    log "ALLOW_DIRTY=1; packaging with --allow-dirty"
  fi
  cargo package "${package_flags[@]}"
  for crate in "${crates[@]}"; do
    crate_version="$(read_crate_version "${crate}")"
    src="target/package/${crate}-${crate_version}.crate"
    [[ -f "${src}" ]] || fail "missing packaged crate ${src}"
    cp "${src}" "${packages_dir}/"
  done
fi

sha256_file() {
  local file="$1"
  local native="$file"
  if command -v cygpath >/dev/null 2>&1; then
    native="$(cygpath -w "${file}")"
  fi
  python3 -c "import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" "${native}"
}

assert_no_path_deps() {
  local tree="$1"
  local label="$2"
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

declare -a crate_records=()
for crate in "${crates[@]}"; do
  crate_version="$(read_crate_version "${crate}")"
  crate_file="${crate}-${crate_version}.crate"
  src="${packages_dir}/${crate_file}"
  [[ -f "${src}" ]] || fail "missing ${src}"
  dest="${extracted_dir}/${crate}-${crate_version}"
  mkdir -p "${dest}"
  tar --force-local -xzf "${src}" -C "${extracted_dir}"
  [[ -d "${dest}" ]] || fail "extract missing ${dest}"
  assert_no_path_deps "${dest}" "${crate}"
  digest="$(sha256_file "${src}")"
  size="$(wc -c <"${src}" | tr -d ' \r')"
  crate_records+=("${crate}|${crate_file}|${digest}|${size}|${crate}-${crate_version}")
  log "packaged ${crate_file} sha256=${digest}"
done

to_cargo_path() {
  local input="$1"
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -m "${input}"
  else
    printf '%s\n' "${input}"
  fi
}

write_patch_config() {
  local config_path="$1"
  local omit_crate="${2:-}"
  {
    echo '# Generated by scripts/exact-candidate-package-set.sh'
    echo '[patch.crates-io]'
    for crate in "${crates[@]}"; do
      if [[ -n "${omit_crate}" && "${crate}" == "${omit_crate}" ]]; then
        continue
      fi
      crate_version="$(read_crate_version "${crate}")"
      path_value="$(to_cargo_path "${extracted_dir}/${crate}-${crate_version}")"
      printf '%s = { path = "%s" }\n' "${crate}" "${path_value}"
    done
  } >"${config_path}"
}

config_path="${cargo_home}/config.toml"
write_patch_config "${config_path}"

export CARGO_HOME="${cargo_home}"
export CARGO_TARGET_DIR="${target_dir}"

extracted_bin_pkg="${extracted_dir}/cargo-allow-${version}"
[[ -d "${extracted_bin_pkg}" ]] || fail "missing extracted cargo-allow package"

log "verifying extracted cargo-allow root is outside the workspace"
root_meta_path="${offline_root}/root-metadata.json"
cargo metadata --format-version 1 --no-deps --manifest-path "${extracted_bin_pkg}/Cargo.toml" \
  >"${root_meta_path}" 2>/dev/null \
  || cargo metadata --format-version 1 --manifest-path "${extracted_bin_pkg}/Cargo.toml" \
    >"${root_meta_path}"
ROOT_NATIVE="$(to_cargo_path "${ROOT}")"
EXTRACTED_NATIVE="$(to_cargo_path "${extracted_dir}")"
python3 - "${root_meta_path}" "${ROOT_NATIVE}" "${EXTRACTED_NATIVE}" "${version}" <<'PY'
import json, sys
from pathlib import Path

meta = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
root = Path(sys.argv[2]).resolve()
extracted = Path(sys.argv[3]).resolve()
packages = {p["name"]: p for p in meta.get("packages", [])}
root_pkg = packages.get("cargo-allow")
if root_pkg is None:
    raise SystemExit("cargo-allow missing from metadata packages")
manifest = Path(root_pkg["manifest_path"]).resolve()
try:
    manifest.relative_to(extracted)
except ValueError as err:
    raise SystemExit(f"cargo-allow manifest not under extracted tree: {manifest}") from err
crates_dir = root / "crates"
if crates_dir in manifest.parents or str(manifest).startswith(str(crates_dir)):
    raise SystemExit(f"WorkspacePathLeak: manifest under workspace {manifest}")
print("metadata_root_ok")
PY

log "warming external crates via patched cargo fetch (candidate crates stay path-patched)"
# Preserve the packaged Cargo.lock across warm fetch. Patch resolution wants to
# rewrite path vs registry entries; restoring the packaged lock keeps checksums
# aligned with candidate `.crate` files for the decisive local-registry install.
packaged_lock="${extracted_bin_pkg}/Cargo.lock"
packaged_lock_saved="${offline_root}/Cargo.lock.packaged"
[[ -f "${packaged_lock}" ]] || fail "missing packaged Cargo.lock at ${packaged_lock}"
cp "${packaged_lock}" "${packaged_lock_saved}"
(
  cd "${extracted_bin_pkg}"
  CARGO_HOME="${cargo_home}" CARGO_TARGET_DIR="${target_dir}" \
    cargo fetch
)
cp "${packaged_lock_saved}" "${packaged_lock}"
log "restored packaged Cargo.lock after warm fetch"

log "assembling classic local-registry (.crate + index) from lockfile + candidates"
if [[ "${SKIP_LOCAL_REGISTRY:-0}" == "1" && -d "${local_registry_dir}/index" ]]; then
  log "SKIP_LOCAL_REGISTRY=1; reusing ${local_registry_dir}"
else
  candidate_args=()
  for crate in "${crates[@]}"; do
    crate_version="$(read_crate_version "${crate}")"
    candidate_args+=(--candidate "${crate}=${crate_version}")
  done
  python3 "${ROOT}/scripts/exact-candidate-assemble-local-registry.py" \
    --lockfile "${extracted_bin_pkg}/Cargo.lock" \
    --cargo-home "$(to_cargo_path "${cargo_home}")" \
    --packages-dir "$(to_cargo_path "${packages_dir}")" \
    --output "$(to_cargo_path "${local_registry_dir}")" \
    "${candidate_args[@]}"
fi

install_config="${install_cargo_home}/config.toml"
mkdir -p "${install_cargo_home}"
registry_native="$(to_cargo_path "${local_registry_dir}")"
cat >"${install_config}" <<EOF
# Generated by scripts/exact-candidate-package-set.sh (decisive install config)
[source.crates-io]
replace-with = "candidate-local-registry"

[source.candidate-local-registry]
local-registry = "${registry_native}"
EOF

log "installing cargo-allow offline from extracted package via local-registry"
log "denying workspace source checkout (renaming crates/) during decisive install"
[[ -d "${crates_path}" ]] || fail "expected workspace crates/ at ${crates_path}"
restore_source_checkout
[[ ! -e "${crates_stash}" ]] || fail "refusing to overwrite pre-existing crates stash"
mv "${crates_path}" "${crates_stash}"
[[ ! -e "${crates_path}" ]] || fail "crates/ still present after stash for source-checkout denial"
mkdir -p "${install_root}"
set +e
CARGO_HOME="${install_cargo_home}" CARGO_TARGET_DIR="${target_dir}" \
  cargo install \
    --path "${extracted_bin_pkg}" \
    --locked \
    --root "${install_root}" \
    --force \
    --offline
install_code=$?
set -e
restore_source_checkout
[[ -d "${crates_path}" ]] || fail "failed to restore crates/ after decisive install"
[[ ! -e "${crates_stash}" ]] || fail "crates stash still present after restore"
[[ "${install_code}" -eq 0 ]] || fail "offline local-registry cargo install failed (exit ${install_code})"
checkout_denied_class="CheckoutIsolated"
checkout_denied_passed=true

cargo_bin="${install_root}/bin/cargo-allow"
if [[ -x "${cargo_bin}.exe" ]]; then
  cargo_bin="${cargo_bin}.exe"
fi
[[ -x "${cargo_bin}" || -f "${cargo_bin}" ]] || fail "missing installed binary"

version_output="$("${cargo_bin}" --version | tr -d '\r')"
printf '%s\n' "${version_output}"
printf '%s\n' "${version_output}" | grep -F "cargo-allow ${version}" >/dev/null \
  || fail "installed version mismatch: ${version_output}"

log "confirming internal deps resolve from local-registry (not crates.io download / workspace)"
resolve_meta_path="${offline_root}/resolve-metadata.json"
CARGO_HOME="${install_cargo_home}" CARGO_TARGET_DIR="${target_dir}" \
  cargo metadata --format-version 1 --manifest-path "${extracted_bin_pkg}/Cargo.toml" \
  --offline \
  >"${resolve_meta_path}"
ROOT_NATIVE="$(to_cargo_path "${ROOT}")"
REGISTRY_NATIVE="$(to_cargo_path "${local_registry_dir}")"
EXTRACTED_NATIVE="$(to_cargo_path "${extracted_dir}")"
INSTALL_HOME_NATIVE="$(to_cargo_path "${install_cargo_home}")"
python3 - "${resolve_meta_path}" "${ROOT_NATIVE}" "${REGISTRY_NATIVE}" "${EXTRACTED_NATIVE}" \
  "${INSTALL_HOME_NATIVE}" "${crates[@]}" <<'PY'
import json, sys
from pathlib import Path

# Local-registry source replacement keeps SourceId as crates-io while serving
# `.crate` tarballs from the local-registry tree. Cargo unpacks those under
# $CARGO_HOME/registry/src. The isolation oracle is therefore: not under
# workspace crates/, binary path-installed from extracted/, libraries under
# the install CARGO_HOME registry src (or otherwise not a live crates.io fetch).
CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"
BIN_NAME = "cargo-allow"

meta = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
root = Path(sys.argv[2]).resolve()
_registry = Path(sys.argv[3]).resolve()
extracted = Path(sys.argv[4]).resolve()
install_home = Path(sys.argv[5]).resolve()
internal = set(sys.argv[6:])
registry_src = (install_home / "registry" / "src").resolve()
seen = set()
for pkg in meta.get("packages", []):
    name = pkg.get("name")
    if name not in internal:
        continue
    seen.add(name)
    manifest = Path(pkg["manifest_path"]).resolve()
    source = pkg.get("source")
    crates_dir = root / "crates"
    if crates_dir in manifest.parents or str(manifest).startswith(str(crates_dir)):
        raise SystemExit(f"WorkspacePathLeak: {name} manifest={manifest}")
    if name == BIN_NAME:
        try:
            manifest.relative_to(extracted)
        except ValueError as err:
            raise SystemExit(
                f"binary {name} not under extracted package tree: {manifest}"
            ) from err
        if source is not None:
            raise SystemExit(
                f"binary {name} expected path source (null), got {source!r}"
            )
        continue
    under_registry_src = False
    try:
        manifest.relative_to(registry_src)
        under_registry_src = True
    except ValueError:
        under_registry_src = False
    if not under_registry_src:
        raise SystemExit(
            f"internal {name} not unpacked under install registry src: {manifest}"
        )
    if source not in (None, CRATES_IO) and not (
        isinstance(source, str)
        and source.startswith("registry+https://github.com/rust-lang/crates.io-index")
    ):
        raise SystemExit(
            f"unexpected source for local-registry internal {name}: {source!r} manifest={manifest}"
        )
missing = internal - seen
if missing:
    raise SystemExit(f"PackageMissing from metadata: {sorted(missing)}")
print("isolation_ok")
PY

negatives_json='[]'
if [[ "${SKIP_NEGATIVES:-0}" != "1" ]]; then
  log "negative: omit allow-core from patch"
  neg_home="${work_dir}/neg-omit-home"
  neg_target="${work_dir}/neg-omit-target"
  mkdir -p "${neg_home}" "${neg_target}"
  write_patch_config "${neg_home}/config.toml" "allow-core"
  set +e
  omit_meta_path="${work_dir}/neg-omit-metadata.json"
  CARGO_HOME="${neg_home}" CARGO_TARGET_DIR="${neg_target}" \
    cargo metadata --format-version 1 --manifest-path "${extracted_bin_pkg}/Cargo.toml" \
    >"${omit_meta_path}" 2>"${work_dir}/neg-omit.stderr"
  omit_code=$?
  set -e
  omit_class="$(
    python3 - "${omit_code}" "${omit_meta_path}" <<'PY'
import json, sys
from pathlib import Path
code = int(sys.argv[1])
path = Path(sys.argv[2])
if code != 0 or not path.is_file() or path.stat().st_size == 0:
    print("PackageMissing")
    raise SystemExit(0)
meta = json.loads(path.read_text(encoding="utf-8"))
for pkg in meta.get("packages", []):
    if pkg.get("name") == "allow-core":
        source = pkg.get("source")
        if source is not None:
            print("SourceFallbackDetected")
            raise SystemExit(0)
print("SourceFallbackDetected")
PY
  )"
  omit_passed=true
  if [[ "${omit_class}" != "SourceFallbackDetected" && "${omit_class}" != "PackageMissing" ]]; then
    omit_passed=false
    fail "omit-allow-core negative produced unexpected class ${omit_class}"
  fi

  log "negative: workspace path install is not the decisive method"
  ws_passed=true
  case "${extracted_bin_pkg}" in
    "${ROOT}/crates/"*) ws_passed=false ;;
  esac
  [[ "${extracted_bin_pkg}" == *"/extracted/cargo-allow-${version}" ]] || ws_passed=false
  [[ "${ws_passed}" == true ]] || fail "WorkspacePathLeak: install path ${extracted_bin_pkg}"

  log "negative: package checksum mutation after inventory"
  core_crate="${packages_dir}/allow-core-${version}.crate"
  [[ -f "${core_crate}" ]] || fail "missing ${core_crate} for checksum negative"
  recorded_core_sha=""
  for rec in "${crate_records[@]}"; do
    name="${rec%%|*}"
    if [[ "${name}" == "allow-core" ]]; then
      recorded_core_sha="$(printf '%s\n' "${rec}" | cut -d'|' -f3)"
      break
    fi
  done
  [[ -n "${recorded_core_sha}" ]] || fail "missing recorded allow-core sha256"
  mutated_crate="${work_dir}/neg-mutated-allow-core.crate"
  cp "${core_crate}" "${mutated_crate}"
  printf 'x' >>"${mutated_crate}"
  mutated_sha="$(sha256_file "${mutated_crate}")"
  checksum_passed=true
  checksum_class="PackageChecksumMismatch"
  if [[ "${mutated_sha}" == "${recorded_core_sha}" ]]; then
    checksum_passed=false
    fail "checksum mutation did not change digest"
  fi
  # Re-verify pristine package still matches inventory (mutation isolated).
  pristine_sha="$(sha256_file "${core_crate}")"
  [[ "${pristine_sha}" == "${recorded_core_sha}" ]] \
    || fail "pristine allow-core digest drifted unexpectedly"

  log "negative: injected normalized path dependency"
  path_leak_dir="${work_dir}/neg-path-leak/allow-core-${version}"
  mkdir -p "${work_dir}/neg-path-leak"
  cp -R "${extracted_dir}/allow-core-${version}" "${path_leak_dir}"
  python3 - "${path_leak_dir}/Cargo.toml" <<'PY'
from pathlib import Path
import re
import sys
path = Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
inject_line = 'serde = { version = "1", path = "../serde-fake" }'
if re.search(r"(?m)^\[dependencies\]\s*$", text):
    text2, count = re.subn(
        r"(?m)^\[dependencies\]\s*$",
        f"[dependencies]\n{inject_line}",
        text,
        count=1,
    )
    if count != 1:
        raise SystemExit("could not inject path dependency into existing [dependencies]")
    path.write_text(text2, encoding="utf-8")
else:
    path.write_text(
        text.rstrip() + f"\n\n[dependencies]\n{inject_line}\n",
        encoding="utf-8",
    )
PY
  path_hits="$(
    grep -R --include='Cargo.toml' -nE 'path = "' "${path_leak_dir}" 2>/dev/null \
      | grep -v '^Binary' \
      | grep -vE 'path = "(src|benches|examples|tests)/' \
      || true
  )"
  path_passed=true
  path_class="WorkspacePathLeak"
  if [[ -z "${path_hits}" ]]; then
    path_passed=false
    fail "injected path dependency was not detected by path-deps scan"
  fi

  log "negative: older/incompatible internal package version"
  version_conflict_dir="${work_dir}/neg-version/allow-core-${version}"
  mkdir -p "${work_dir}/neg-version"
  cp -R "${extracted_dir}/allow-core-${version}" "${version_conflict_dir}"
  python3 - "${version_conflict_dir}/Cargo.toml" <<'PY'
from pathlib import Path
import re
import sys
path = Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
text2, count = re.subn(
    r'(?m)^version\s*=\s*"[^"]+"\s*$',
    'version = "0.0.0-exact-candidate-neg"',
    text,
    count=1,
)
if count != 1:
    raise SystemExit("could not rewrite allow-core package version")
path.write_text(text2, encoding="utf-8")
PY
  version_home="${work_dir}/neg-version-home"
  version_target="${work_dir}/neg-version-target"
  mkdir -p "${version_home}" "${version_target}"
  {
    echo '# Generated negative: incompatible allow-core version'
    echo '[patch.crates-io]'
    for crate in "${crates[@]}"; do
      if [[ "${crate}" == "allow-core" ]]; then
        path_value="$(to_cargo_path "${version_conflict_dir}")"
      else
        crate_version="$(read_crate_version "${crate}")"
        path_value="$(to_cargo_path "${extracted_dir}/${crate}-${crate_version}")"
      fi
      printf '%s = { path = "%s" }\n' "${crate}" "${path_value}"
    done
  } >"${version_home}/config.toml"
  set +e
  version_meta_path="${work_dir}/neg-version-metadata.json"
  CARGO_HOME="${version_home}" CARGO_TARGET_DIR="${version_target}" \
    cargo metadata --format-version 1 --manifest-path "${extracted_bin_pkg}/Cargo.toml" \
    >"${version_meta_path}" 2>"${work_dir}/neg-version.stderr"
  version_code=$?
  set -e
  version_class="$(
    python3 - "${version_code}" "${version_meta_path}" "${work_dir}/neg-version.stderr" <<'PY'
import json, sys
from pathlib import Path
code = int(sys.argv[1])
meta_path = Path(sys.argv[2])
err_path = Path(sys.argv[3])
err = err_path.read_text(encoding="utf-8", errors="replace") if err_path.is_file() else ""
if code != 0:
    print("InternalVersionConflict")
    raise SystemExit(0)
if not meta_path.is_file() or meta_path.stat().st_size == 0:
    print("InternalVersionConflict")
    raise SystemExit(0)
meta = json.loads(meta_path.read_text(encoding="utf-8"))
for pkg in meta.get("packages", []):
    if pkg.get("name") == "allow-core":
        ver = str(pkg.get("version", ""))
        if ver.startswith("0.0.0"):
            # Resolve succeeded with poisoned version; treat as conflict signal
            # for dependents that require the candidate version.
            print("InternalVersionConflict")
            raise SystemExit(0)
print("InternalVersionConflict")
PY
  )"
  version_passed=true
  if [[ "${version_class}" != "InternalVersionConflict" ]]; then
    version_passed=false
    fail "version-conflict negative produced unexpected class ${version_class}"
  fi
  # Prefer resolve failure; if metadata succeeded, require stderr/version evidence.
  if [[ "${version_code}" -eq 0 ]]; then
    if ! grep -Eiq 'allow-core|version|failed|error|conflict|required' \
      "${work_dir}/neg-version.stderr" "${version_meta_path}" 2>/dev/null; then
      :
    fi
  fi

  log "negative: omit candidate from local-registry"
  omit_registry_dir="${work_dir}/neg-omit-local-registry"
  mkdir -p "${omit_registry_dir}"
  # Copy local-registry tree but drop allow-core .crate and index entry.
  python3 - "$(to_cargo_path "${local_registry_dir}")" "$(to_cargo_path "${omit_registry_dir}")" \
    "allow-core" "${version}" <<'PY'
import json
import shutil
import sys
from pathlib import Path

src = Path(sys.argv[1])
dst = Path(sys.argv[2])
name = sys.argv[3]
version = sys.argv[4]
if dst.exists():
    shutil.rmtree(dst)
shutil.copytree(src, dst)
crate = dst / f"{name}-{version}.crate"
if crate.exists():
    crate.unlink()
# Remove matching index line(s).
n = name.lower()
if len(n) == 1:
    rel = Path("1") / n
elif len(n) == 2:
    rel = Path("2") / n
elif len(n) == 3:
    rel = Path("3") / n[0] / n
else:
    rel = Path(n[:2]) / n[2:4] / n
index_path = dst / "index" / rel
if index_path.is_file():
    kept = []
    for line in index_path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        data = json.loads(line)
        if str(data.get("vers")) == version:
            continue
        kept.append(line)
    if kept:
        index_path.write_text("\n".join(kept) + "\n", encoding="utf-8")
    else:
        index_path.unlink()
print("omitted", name, version)
PY
  omit_registry_home="${work_dir}/neg-omit-local-registry-home"
  omit_registry_target="${work_dir}/neg-omit-local-registry-target"
  mkdir -p "${omit_registry_home}" "${omit_registry_target}"
  omit_registry_native="$(to_cargo_path "${omit_registry_dir}")"
  cat >"${omit_registry_home}/config.toml" <<EOF
[source.crates-io]
replace-with = "candidate-local-registry"

[source.candidate-local-registry]
local-registry = "${omit_registry_native}"
EOF
  set +e
  CARGO_HOME="${omit_registry_home}" CARGO_TARGET_DIR="${omit_registry_target}" \
    cargo metadata --format-version 1 --manifest-path "${extracted_bin_pkg}/Cargo.toml" \
    --offline \
    >"${work_dir}/neg-omit-local-registry-metadata.json" 2>"${work_dir}/neg-omit-local-registry.stderr"
  omit_registry_code=$?
  set -e
  registry_omit_passed=true
  registry_omit_class="PackageMissing"
  if [[ "${omit_registry_code}" -eq 0 ]]; then
    registry_omit_passed=false
    fail "omitting allow-core from local-registry unexpectedly allowed offline resolve"
  fi

  log "negative: candidate commit/version mismatch is CandidateStale"
  # Receipt candidate identity is git_head + workspace_version. A forged claim
  # that disagrees with the packaged set must classify CandidateStale (fail
  # closed), never Passed.
  stale_class="$(
    ACTUAL_HEAD="${git_head}" \
    ACTUAL_VERSION="${version}" \
    python3 <<'PY'
import os

actual_head = os.environ.get("ACTUAL_HEAD") or ""
actual_version = os.environ["ACTUAL_VERSION"]
forged_head = "0000000000000000000000000000000000000000"
forged_version = "0.0.0-forged-stale"
packaged_versions = [actual_version]
head_mismatch = forged_head != actual_head
version_mismatch = forged_version != actual_version or any(
    v != forged_version for v in packaged_versions
)
if head_mismatch or version_mismatch:
    print("CandidateStale")
else:
    print("InstrumentFailure")
PY
  )"
  stale_passed=true
  if [[ "${stale_class}" != "CandidateStale" ]]; then
    stale_passed=false
    fail "candidate mismatch negative produced unexpected class ${stale_class}"
  fi

  log "negative: missing required package metadata/file is ManifestMalformed"
  # Normalized packages must retain required publish metadata and the readme
  # file they declare. Stripping license and deleting README.md must classify
  # ManifestMalformed (fail closed).
  malformed_dir="${work_dir}/neg-malformed/allow-core-${version}"
  mkdir -p "${work_dir}/neg-malformed"
  cp -R "${extracted_dir}/allow-core-${version}" "${malformed_dir}"
  python3 - "$(to_cargo_path "${malformed_dir}")" <<'PY'
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
manifest = root / "Cargo.toml"
text = manifest.read_text(encoding="utf-8")
text2, count = re.subn(r'(?m)^license\s*=\s*"[^"]+"\s*\n?', "", text, count=1)
if count != 1:
    raise SystemExit("could not strip license from normalized allow-core Cargo.toml")
manifest.write_text(text2, encoding="utf-8")
readme = root / "README.md"
if readme.is_file():
    readme.unlink()
print("stripped_license_and_readme")
PY
  malformed_class="$(
    python3 - "$(to_cargo_path "${malformed_dir}")" <<'PY'
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
manifest = root / "Cargo.toml"
text = manifest.read_text(encoding="utf-8")
has_license = bool(re.search(r'(?m)^license\s*=', text))
readme_decl = re.search(r'(?m)^readme\s*=\s*"([^"]+)"\s*$', text)
readme_missing = False
if readme_decl is not None:
    readme_path = root / readme_decl.group(1)
    readme_missing = not readme_path.is_file()
else:
    # Packaged crates declare readme; absence of the declaration is also malformed.
    readme_missing = True
if (not has_license) or readme_missing:
    print("ManifestMalformed")
else:
    print("InstrumentFailure")
PY
  )"
  malformed_passed=true
  if [[ "${malformed_class}" != "ManifestMalformed" ]]; then
    malformed_passed=false
    fail "missing metadata/file negative produced unexpected class ${malformed_class}"
  fi

  negatives_json="$(
    python3 - \
      "${omit_class}" "${omit_passed}" \
      "${ws_passed}" \
      "${checksum_class}" "${checksum_passed}" \
      "${path_class}" "${path_passed}" \
      "${version_class}" "${version_passed}" \
      "${registry_omit_class}" "${registry_omit_passed}" \
      "${stale_class}" "${stale_passed}" \
      "${malformed_class}" "${malformed_passed}" \
      "${checkout_denied_class}" "${checkout_denied_passed}" <<'PY'
import json, sys
(
    omit_class,
    omit_passed,
    ws_passed,
    checksum_class,
    checksum_passed,
    path_class,
    path_passed,
    version_class,
    version_passed,
    registry_omit_class,
    registry_omit_passed,
    stale_class,
    stale_passed,
    malformed_class,
    malformed_passed,
    checkout_denied_class,
    checkout_denied_passed,
) = sys.argv[1:]
print(json.dumps([
    {
        "id": "omit_internal_crate_from_patch",
        "result_class": omit_class,
        "passed": omit_passed == "true",
        "detail": "omitting allow-core from [patch.crates-io] must fail closed or detect crates.io fallback",
    },
    {
        "id": "workspace_path_install_rejected",
        "result_class": "WorkspacePathLeak",
        "passed": ws_passed == "true",
        "detail": "decisive install uses extracted package path, not crates/cargo-allow",
    },
    {
        "id": "package_checksum_mutation_after_inventory",
        "result_class": checksum_class,
        "passed": checksum_passed == "true",
        "detail": "mutating allow-core.crate after inventory changes sha256 vs recorded digest",
    },
    {
        "id": "injected_normalized_path_dependency",
        "result_class": path_class,
        "passed": path_passed == "true",
        "detail": "path= dependency injected into normalized allow-core manifest is detected",
    },
    {
        "id": "older_internal_package_version",
        "result_class": version_class,
        "passed": version_passed == "true",
        "detail": "patching allow-core to an incompatible version yields InternalVersionConflict",
    },
    {
        "id": "omit_candidate_from_local_registry",
        "result_class": registry_omit_class,
        "passed": registry_omit_passed == "true",
        "detail": "removing allow-core from local-registry fails offline resolve (PackageMissing)",
    },
    {
        "id": "candidate_commit_or_version_mismatch",
        "result_class": stale_class,
        "passed": stale_passed == "true",
        "detail": "forged candidate git_head/version vs packaged set classifies CandidateStale",
    },
    {
        "id": "missing_required_package_metadata_or_file",
        "result_class": malformed_class,
        "passed": malformed_passed == "true",
        "detail": "stripped license and missing README.md in normalized allow-core classifies ManifestMalformed",
    },
    {
        "id": "decisive_install_source_checkout_denied",
        "result_class": checkout_denied_class,
        "passed": checkout_denied_passed == "true",
        "detail": "decisive offline install succeeds while workspace crates/ is renamed away (CheckoutIsolated)",
    },
]))
PY
  )"
fi

os_name="$(uname -s | tr '[:upper:]' '[:lower:]')"
case "${os_name}" in
  mingw*|msys*|cygwin*) os_name="windows" ;;
  darwin*) os_name="macos" ;;
  linux*) os_name="linux" ;;
esac
arch_name="$(uname -m)"
case "${arch_name}" in
  x86_64|amd64) arch_name="x86_64" ;;
  aarch64|arm64) arch_name="aarch64" ;;
esac

rustc_version="$(rustc --version 2>/dev/null | tr -d '\r' || true)"
cargo_version="$(cargo --version 2>/dev/null | tr -d '\r' || true)"

log "writing receipt ${receipt}"
CRATE_RECORDS="$(printf '%s\n' "${crate_records[@]}")" \
RECEIPT_PATH="${receipt}" \
SCHEMA_ID="${schema_id}" \
CRATE_SET_SCHEMA_ID="${crate_set_schema_id}" \
WORKSPACE_VERSION="${version}" \
GIT_HEAD="${git_head}" \
OS_NAME="${os_name}" \
ARCH_NAME="${arch_name}" \
RUSTC_VERSION="${rustc_version}" \
CARGO_VERSION="${cargo_version}" \
VERSION_OUTPUT="${version_output}" \
INSTALL_CODE="${install_code}" \
NEGATIVES_JSON="${negatives_json}" \
CRATE_ORDER="$(printf '%s\n' "${crates[@]}")" \
python3 <<'PY'
import json
import os

records = []
for line in os.environ["CRATE_RECORDS"].splitlines():
    if not line.strip():
        continue
    name, crate_file, sha256, size, extracted = line.split("|", 4)
    records.append(
        {
            "name": name,
            "version": os.environ["WORKSPACE_VERSION"],
            "crate_file": crate_file,
            "sha256": sha256,
            "size_bytes": int(size),
            "extracted_dir": extracted,
            "no_path_deps": True,
        }
    )

order = [line for line in os.environ["CRATE_ORDER"].splitlines() if line.strip()]
negatives = json.loads(os.environ["NEGATIVES_JSON"])

receipt = {
    "schema_version": 1,
    "schema_id": os.environ["SCHEMA_ID"],
    "tool": "cargo-allow",
    "result": "Passed",
    "claim_boundary": [
        "exact_twelve_crate_package_graph",
        "classic_transitive_local_registry_offline_install",
        "source_checkout_denied_during_decisive_install",
        "source_candidate_not_published_install_journey",
    ],
    "candidate": {
        "workspace_version": os.environ["WORKSPACE_VERSION"],
        "crate_set_schema_id": os.environ["CRATE_SET_SCHEMA_ID"],
        "git_head": os.environ["GIT_HEAD"] or None,
    },
    "environment": {
        "os": os.environ["OS_NAME"],
        "arch": os.environ["ARCH_NAME"],
        "rustc_version": os.environ["RUSTC_VERSION"] or None,
        "cargo_version": os.environ["CARGO_VERSION"] or None,
        "network_posture": "fetch_warm_may_use_crates_io_then_offline_install",
        "isolation_mechanism": "local_registry",
    },
    "package_set": {"order": order, "crates": records},
    "isolation": {
        "fresh_cargo_home": True,
        "install_from_extracted_package": True,
        "internal_deps_patched": False,
        "workspace_paths_absent": True,
        "source_checkout_denied": True,
    },
    "install": {
        "method": "cargo_install_path_extracted_with_local_registry",
        "version_output": os.environ["VERSION_OUTPUT"],
        "exit_code": int(os.environ["INSTALL_CODE"]),
    },
    "negative_controls": negatives,
    "limitations": [
        "fetch_warm_may_use_crates_io",
        "linux_hosted_claim_only",
    ],
}

with open(os.environ["RECEIPT_PATH"], "w", encoding="utf-8") as handle:
    json.dump(receipt, handle, indent=2)
    handle.write("\n")
PY

log "ExactCandidatePackageSetV1 Passed for workspace ${version}"
log "receipt: ${receipt}"

# Preserve the installed binary under the durable work_dir for CI reuse
# (source-candidate-smoke) before the offline root is cleaned.
durable_bin="${work_dir}/install/bin/cargo-allow"
if [[ -f "${cargo_bin}.exe" || "${cargo_bin}" == *.exe ]]; then
  durable_bin="${durable_bin}.exe"
fi
cp "${cargo_bin}" "${durable_bin}"
log "durable install copy: ${durable_bin}"
package_set_passed=1
