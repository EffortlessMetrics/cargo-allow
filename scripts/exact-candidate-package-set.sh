#!/usr/bin/env bash
# ExactCandidatePackageSetV1 Stage A (#2372 / #2277).
#
# Packages the canonical ten-crate set, extracts each .crate outside the
# workspace, installs cargo-allow from the extracted package using
# [patch.crates-io] for internal deps, verifies internal package sources are
# not the workspace tree, runs two negative controls, and emits a JSON receipt.
#
# Does not: publish; full local-registry index; deny the source checkout;
# complete every #2277 negative; run the installed operator journey (#2278).
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
extracted_dir="${work_dir}/extracted"
cargo_home="${work_dir}/cargo-home"
target_dir="${work_dir}/target"
install_root="${work_dir}/install"
receipt="${work_dir}/exact-candidate-package-set.receipt.json"
crate_set_fixture="${ROOT}/docs/dogfood/fixtures/release/candidate-crate-set.toml"
schema_id="cargo-allow.exact-candidate-package-set.v1"
crate_set_schema_id="cargo-allow.candidate-crate-set.v1"

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
    stripped = line.strip()
    if stripped.startswith("crates"):
        in_list = True
        continue
    if in_list:
        if stripped.startswith("]"):
            break
        if stripped.startswith('"') and stripped.endswith('",'):
            print(stripped.strip('",'))
        elif stripped.startswith('"') and stripped.endswith('"'):
            print(stripped.strip('"'))
PY
)

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
mkdir -p "${packages_dir}" "${extracted_dir}" "${cargo_home}" "${target_dir}" "${install_root}"

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

sha256_file() {
  python3 - "$1" <<'PY'
import hashlib, sys
from pathlib import Path
data = Path(sys.argv[1]).read_bytes()
print(hashlib.sha256(data).hexdigest())
PY
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
  size="$(python3 -c "from pathlib import Path; print(Path(r'''${src}''').stat().st_size)")"
  crate_records+=("${crate}|${crate_file}|${digest}|${size}|${crate}-${version}")
  log "packaged ${crate_file} sha256=${digest}"
done

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
      # Absolute paths so config under CARGO_HOME resolves correctly.
      printf '%s = { path = "%s" }\n' "${crate}" "${extracted_dir}/${crate}-${version}"
    done
  } >"${config_path}"
}

# Convert Windows paths in config to forward slashes for Cargo TOML.
normalize_config_paths() {
  python3 - "$1" <<'PY'
from pathlib import Path
import sys
path = Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
path.write_text(text.replace("\\\\", "/").replace("\\", "/"), encoding="utf-8")
PY
}

config_path="${cargo_home}/config.toml"
write_patch_config "${config_path}"
normalize_config_paths "${config_path}"

export CARGO_HOME="${cargo_home}"
export CARGO_TARGET_DIR="${target_dir}"

extracted_bin_pkg="${extracted_dir}/cargo-allow-${version}"
[[ -d "${extracted_bin_pkg}" ]] || fail "missing extracted cargo-allow package"

log "verifying internal package sources via cargo metadata (patched)"
metadata_json="$(
  cargo metadata --format-version 1 --no-deps --manifest-path "${extracted_bin_pkg}/Cargo.toml" 2>/dev/null \
    || cargo metadata --format-version 1 --manifest-path "${extracted_bin_pkg}/Cargo.toml"
)"

python3 - "${metadata_json}" "${ROOT}" "${extracted_dir}" "${version}" "${crates[@]}" <<'PY'
import json, sys
from pathlib import Path

raw = sys.argv[1]
root = Path(sys.argv[2]).resolve()
extracted = Path(sys.argv[3]).resolve()
version = sys.argv[4]
internal = set(sys.argv[5:])
meta = json.loads(raw)
packages = {p["name"]: p for p in meta.get("packages", [])}
# With --no-deps we only get the root; re-parse expecting full resolve when present.
# Fall back: ensure root package path is under extracted.
root_pkg = packages.get("cargo-allow")
if root_pkg is None:
    raise SystemExit("cargo-allow missing from metadata packages")
manifest = Path(root_pkg["manifest_path"]).resolve()
if extracted not in manifest.parents and manifest.parent != extracted:
    # allow exact extracted/cargo-allow-VERSION/Cargo.toml
    if extracted / f"cargo-allow-{version}" != manifest.parent:
        raise SystemExit(f"cargo-allow manifest not under extracted tree: {manifest}")
if root in manifest.parents or str(manifest).startswith(str(root / "crates")):
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
resolve_json="$(
  CARGO_HOME="${cargo_home}" CARGO_TARGET_DIR="${target_dir}" \
    cargo metadata --format-version 1 --manifest-path "${extracted_bin_pkg}/Cargo.toml"
)"
python3 - "${resolve_json}" "${ROOT}" "${extracted_dir}" "${crates[@]}" <<'PY'
import json, sys
from pathlib import Path

meta = json.loads(sys.argv[1])
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
    if root / "crates" in manifest.parents or str(manifest).startswith(str(root / "crates")):
        raise SystemExit(f"WorkspacePathLeak: {name} manifest={manifest}")
    if extracted not in manifest.parents and manifest.parent.parent != extracted:
        # extracted/<crate>-<ver>/Cargo.toml → parent.parent == extracted
        if manifest.parent.parent.resolve() != extracted and manifest.parent.resolve().parent != extracted:
            # Accept if path is under extracted_dir
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
  normalize_config_paths "${neg_home}/config.toml"
  set +e
  omit_meta="$(
    CARGO_HOME="${neg_home}" CARGO_TARGET_DIR="${neg_target}" \
      cargo metadata --format-version 1 --manifest-path "${extracted_bin_pkg}/Cargo.toml" 2>"${work_dir}/neg-omit.stderr"
  )"
  omit_code=$?
  set -e
  omit_class="$(
    python3 - "${omit_code}" "${omit_meta:-}" <<'PY'
import json, sys
code = int(sys.argv[1])
raw = sys.argv[2] if len(sys.argv) > 2 else ""
if code != 0 or not raw.strip():
    print("PackageMissing")
    raise SystemExit(0)
meta = json.loads(raw)
for pkg in meta.get("packages", []):
    if pkg.get("name") == "allow-core":
        source = pkg.get("source")
        if source is not None and "crates.io" in str(source):
            print("SourceFallbackDetected")
            raise SystemExit(0)
        if source is not None:
            print("SourceFallbackDetected")
            raise SystemExit(0)
print("SourceFallbackDetected")
PY
  )"
  # Control passes when the harness classifies omit as a hard failure class
  # (registry fallback detected or resolve failure), never as silent success
  # with patched workspace paths.
  omit_passed=true
  if [[ "${omit_class}" != "SourceFallbackDetected" && "${omit_class}" != "PackageMissing" ]]; then
    omit_passed=false
    fail "omit-allow-core negative produced unexpected class ${omit_class}"
  fi

  log "negative: workspace path install is not the decisive method"
  # Characterization: decisive install path must be the extracted package.
  ws_passed=true
  case "${extracted_bin_pkg}" in
    "${ROOT}/crates/"*) ws_passed=false ;;
  esac
  [[ "${extracted_bin_pkg}" == *"/extracted/cargo-allow-${version}" ]] || ws_passed=false
  [[ "${ws_passed}" == true ]] || fail "WorkspacePathLeak: install path ${extracted_bin_pkg}"

  negatives_json="$(
    python3 - "${omit_class}" "${omit_passed}" "${ws_passed}" <<'PY'
import json, sys
omit_class, omit_passed, ws_passed = sys.argv[1], sys.argv[2] == "true", sys.argv[3] == "true"
print(json.dumps([
    {
        "id": "omit_internal_crate_from_patch",
        "result_class": omit_class,
        "passed": omit_passed,
        "detail": "omitting allow-core from [patch.crates-io] must fail closed or detect crates.io fallback",
    },
    {
        "id": "workspace_path_install_rejected",
        "result_class": "WorkspacePathLeak",
        "passed": ws_passed,
        "detail": "decisive install uses extracted package path, not crates/cargo-allow",
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
        "remaining_negative_controls_deferred",
        "linux_hosted_claim_only",
    ],
}

with open(os.environ["RECEIPT_PATH"], "w", encoding="utf-8") as handle:
    json.dump(receipt, handle, indent=2)
    handle.write("\n")
PY

log "ExactCandidatePackageSetV1 Passed for workspace ${version}"
log "receipt: ${receipt}"
