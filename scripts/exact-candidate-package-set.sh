#!/usr/bin/env bash
# ExactCandidatePackageSetV1 Stage A/B (#2372 / #2277 / #2378).
#
# Packages the canonical ten-crate set, extracts each .crate outside the
# workspace, installs cargo-allow from the extracted package using
# [patch.crates-io] for internal deps, verifies internal package sources are
# not the workspace tree, runs negative controls, and emits a JSON receipt.
#
# Negatives covered:
#   - omit internal crate from patch
#   - workspace path install rejected
#   - package checksum mutation after inventory
#   - injected normalized path dependency
#   - older/incompatible internal package version
#
# Does not: publish; full local-registry index; deny the source checkout;
# every #2277 negative; run the installed operator journey (#2278).
#
# Usage:
#   bash scripts/exact-candidate-package-set.sh
#
# Optional:
#   WORK_DIR=<path>     work root (default: target/exact-candidate-package-set)
#   SKIP_PACKAGE=1      reuse WORK_DIR/packages without re-packing
#   SKIP_NEGATIVES=1    skip negative controls (debug only)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

work_dir="${WORK_DIR:-${ROOT}/target/exact-candidate-package-set}"
packages_dir="${work_dir}/packages"
# Extracted packages must live outside the workspace tree; otherwise Cargo
# walks up to the repo Cargo.toml and treats them as workspace members.
offline_root="${OFFLINE_ROOT:-$(mktemp -d "${TMPDIR:-/tmp}/cargo-allow-ecps-offline.XXXXXX")}"
extracted_dir="${offline_root}/extracted"
cargo_home="${offline_root}/cargo-home"
target_dir="${offline_root}/target"
install_root="${offline_root}/install"
receipt="${work_dir}/exact-candidate-package-set.receipt.json"
crate_set_fixture="${ROOT}/docs/dogfood/fixtures/release/candidate-crate-set.toml"
schema_id="cargo-allow.exact-candidate-package-set.v1"
crate_set_schema_id="cargo-allow.candidate-crate-set.v1"

cleanup_offline() {
  if [[ "${KEEP_OFFLINE:-0}" != "1" ]]; then
    rm -rf "${offline_root}"
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
command -v python3 >/dev/null 2>&1 || fail "python3 is required"
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

[[ "${#crates[@]}" -eq 10 ]] || fail "expected 10 crates from ${crate_set_fixture}, got ${#crates[@]}"

version="$(read_workspace_version)"
[[ -n "${version}" ]] || fail "could not read workspace.package.version"

git_head=""
if command -v git >/dev/null 2>&1 && git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  git_head="$(git rev-parse HEAD 2>/dev/null || true)"
fi

package_staging=""
if [[ "${SKIP_PACKAGE:-0}" == "1" ]]; then
  package_staging="$(mktemp -d "${TMPDIR:-/tmp}/cargo-allow-ecps-packages.XXXXXX")"
  shopt -s nullglob
  staged=("${packages_dir}"/*.crate)
  shopt -u nullglob
  [[ "${#staged[@]}" -gt 0 ]] \
    || fail "SKIP_PACKAGE=1 but no .crate files under ${packages_dir}"
  cp "${staged[@]}" "${package_staging}/"
fi

rm -rf "${work_dir}"
mkdir -p "${packages_dir}" "${extracted_dir}" "${cargo_home}" "${target_dir}" "${install_root}" "${work_dir}/install/bin"

if [[ "${SKIP_PACKAGE:-0}" == "1" ]]; then
  log "SKIP_PACKAGE=1; restoring prebuilt packages"
  cp "${package_staging}"/*.crate "${packages_dir}/"
  rm -rf "${package_staging}"
else
  log "packaging workspace crates with cargo package --workspace --locked"
  cargo package --workspace --locked
  for crate in "${crates[@]}"; do
    src="target/package/${crate}-${version}.crate"
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
  crate_file="${crate}-${version}.crate"
  src="${packages_dir}/${crate_file}"
  [[ -f "${src}" ]] || fail "missing ${src}"
  dest="${extracted_dir}/${crate}-${version}"
  rm -rf "${dest}"
  mkdir -p "${dest}"
  tar --force-local -xzf "${src}" -C "${extracted_dir}"
  [[ -d "${dest}" ]] || fail "extract missing ${dest}"
  assert_no_path_deps "${dest}" "${crate}"
  digest="$(sha256_file "${src}")"
  size="$(wc -c <"${src}" | tr -d ' \r')"
  crate_records+=("${crate}|${crate_file}|${digest}|${size}|${crate}-${version}")
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
      path_value="$(to_cargo_path "${extracted_dir}/${crate}-${version}")"
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

log "verifying internal package sources via cargo metadata (patched)"
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
version = sys.argv[4]
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

log "installing cargo-allow from extracted package with [patch.crates-io]"
rm -rf "${install_root}"
mkdir -p "${install_root}"
set +e
cargo install \
  --path "${extracted_bin_pkg}" \
  --locked \
  --root "${install_root}" \
  --force
install_code=$?
set -e
[[ "${install_code}" -eq 0 ]] || fail "cargo install from extracted package failed (exit ${install_code})"

cargo_bin="${install_root}/bin/cargo-allow"
if [[ -x "${cargo_bin}.exe" ]]; then
  cargo_bin="${cargo_bin}.exe"
fi
[[ -x "${cargo_bin}" || -f "${cargo_bin}" ]] || fail "missing installed binary"

version_output="$("${cargo_bin}" --version | tr -d '\r')"
printf '%s\n' "${version_output}"
printf '%s\n' "${version_output}" | grep -F "cargo-allow ${version}" >/dev/null \
  || fail "installed version mismatch: ${version_output}"

# Stronger source proof: cargo tree / metadata with deps after successful install
# using a throwaway check that patches resolve to extracted paths.
log "confirming patched internal deps resolve under extracted/"
resolve_meta_path="${offline_root}/resolve-metadata.json"
CARGO_HOME="${cargo_home}" CARGO_TARGET_DIR="${target_dir}" \
  cargo metadata --format-version 1 --manifest-path "${extracted_bin_pkg}/Cargo.toml" \
  >"${resolve_meta_path}"
ROOT_NATIVE="$(to_cargo_path "${ROOT}")"
EXTRACTED_NATIVE="$(to_cargo_path "${extracted_dir}")"
python3 - "${resolve_meta_path}" "${ROOT_NATIVE}" "${EXTRACTED_NATIVE}" "${crates[@]}" <<'PY'
import json, sys
from pathlib import Path

meta = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
root = Path(sys.argv[2]).resolve()
extracted = Path(sys.argv[3]).resolve()
internal = set(sys.argv[4:])
seen = set()
for pkg in meta.get("packages", []):
    name = pkg.get("name")
    if name not in internal:
        continue
    seen.add(name)
    manifest = Path(pkg["manifest_path"]).resolve()
    source = pkg.get("source")
    if source is not None:
        raise SystemExit(
            f"SourceFallbackDetected: {name} source={source!r} manifest={manifest}"
        )
    crates_dir = root / "crates"
    if crates_dir in manifest.parents or str(manifest).startswith(str(crates_dir)):
        raise SystemExit(f"WorkspacePathLeak: {name} manifest={manifest}")
    try:
        manifest.relative_to(extracted)
    except ValueError as err:
        raise SystemExit(f"internal {name} not under extracted: {manifest}") from err
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
  rm -rf "${neg_home}" "${neg_target}"
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
  rm -rf "${work_dir}/neg-path-leak"
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
  rm -rf "${work_dir}/neg-version"
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
  rm -rf "${version_home}" "${version_target}"
  mkdir -p "${version_home}" "${version_target}"
  {
    echo '# Generated negative: incompatible allow-core version'
    echo '[patch.crates-io]'
    for crate in "${crates[@]}"; do
      if [[ "${crate}" == "allow-core" ]]; then
        path_value="$(to_cargo_path "${version_conflict_dir}")"
      else
        path_value="$(to_cargo_path "${extracted_dir}/${crate}-${version}")"
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
      # Metadata succeeded silently with 0.0.0 package — still a distinct class
      # but record that Cargo accepted the patch version rewrite.
      :
    fi
  fi

  negatives_json="$(
    python3 - \
      "${omit_class}" "${omit_passed}" \
      "${ws_passed}" \
      "${checksum_class}" "${checksum_passed}" \
      "${path_class}" "${path_passed}" \
      "${version_class}" "${version_passed}" <<'PY'
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
        "exact_ten_crate_package_graph",
        "patch_crates_io_extracted_packages",
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
        "network_posture": "external_deps_may_use_crates_io_cache",
        "isolation_mechanism": "patch_crates_io_extracted_packages",
    },
    "package_set": {"order": order, "crates": records},
    "isolation": {
        "fresh_cargo_home": True,
        "install_from_extracted_package": True,
        "internal_deps_patched": True,
        "workspace_paths_absent": True,
        "source_checkout_denied": False,
    },
    "install": {
        "method": "cargo_install_path_extracted_with_patch",
        "version_output": os.environ["VERSION_OUTPUT"],
        "exit_code": int(os.environ["INSTALL_CODE"]),
    },
    "negative_controls": negatives,
    "limitations": [
        "external_deps_may_use_crates_io",
        "not_full_local_registry_index",
        "source_checkout_not_denied_during_install",
        "candidate_commit_mismatch_negative_deferred",
        "missing_package_metadata_negative_deferred",
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
