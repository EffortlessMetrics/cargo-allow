#!/usr/bin/env bash
# Isolated local-registry install of the exact cargo-allow candidate
# (#2925). Consumes the typed candidate artifact produced by
# exact-candidate-package-candidate.py, builds an isolated classic local
# registry, installs `cargo-allow --locked --offline` with a fresh Cargo
# home while the workspace crates/ tree is denied, compares the actual
# resolved graph against the candidate rows, and emits the typed
# cargo-allow.isolated-install.v2 receipt.
#
# No tag, upload, publication, or live registry change occurs. The
# workspace source tree is restored on every exit path.

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
CANDIDATE_ARTIFACT="${CANDIDATE_ARTIFACT:-target/exact-candidate-package-candidate/package-candidate-v2.json}"
PACKAGES_DIR="${PACKAGES_DIR:-target/exact-candidate-package-set/packages}"
OUTPUT_DIR="${OUTPUT_DIR:-target/exact-candidate-isolated-install}"

cd "$ROOT"

python3 - "$CANDIDATE_ARTIFACT" "$PACKAGES_DIR" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

artifact_path = Path(sys.argv[1])
packages_dir = Path(sys.argv[2])
artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
rows = artifact["rows"]
if not rows:
    raise SystemExit("candidate artifact has no rows")
out = []
for row in rows:
    name = row["cargo_package_name"]
    version = row["cargo_package_version"]
    crate = packages_dir / f"{name}-{version}.crate"
    if not crate.is_file():
        raise SystemExit(f"packaged crate missing: {crate.name}")
    digest = hashlib.sha256(crate.read_bytes()).hexdigest()
    if row.get("crate_digest") and row["crate_digest"] != f"sha256:{digest}":
        raise SystemExit(
            f"crate digest drift for {name}: artifact {row['crate_digest']} actual sha256:{digest}"
        )
    out.append({
        "cargo_package_name": name,
        "cargo_package_version": version,
        "crate_digest": f"sha256:{digest}",
        "crate_path": str(crate),
    })
Path("target/exact-candidate-isolated-install").mkdir(parents=True, exist_ok=True)
Path("target/exact-candidate-isolated-install/consumed-rows.json").write_text(
    json.dumps(out, indent=2) + "\n", encoding="utf-8"
)
print(f"consumed {len(out)} candidate rows with digest agreement")
PY

CANDIDATE_VERSION="$(python3 -c "import json;print(json.load(open('$CANDIDATE_ARTIFACT'))['root_package_version'])")"

lifecycle="scripts/candidate-harness-owned-dir.py"
test_root_json="$(python3 "$lifecycle" allocate --root "${TMPDIR:-/tmp}" --purpose exact-candidate-isolated-install-test-root)"
read -r test_root test_root_token < <(
  printf '%s' "${test_root_json}" | python3 -c 'import json,sys; v=json.load(sys.stdin); print(v["path"], v["token"])'
)
work_json="$(python3 "$lifecycle" allocate --root "$test_root" --purpose exact-candidate-isolated-install)"
read -r work_parent work_token < <(
  printf '%s' "${work_json}" | python3 -c 'import json,sys; v=json.load(sys.stdin); print(v["path"], v["token"])'
)
offline_root="${work_parent}/offline"
extracted_root="${work_parent}/extracted"
registry="${work_parent}/registry"
warm_home="${work_parent}/warm-home"
install_home="${offline_root}/install-cargo-home"
install_root="${offline_root}/install"
mkdir -p "$offline_root" "$extracted_root" "$registry" "$warm_home"

crates_path="${ROOT}/crates"
crates_stash="${work_parent}/crates-source-stash"
source_denied=0
source_restored=0

restore_source_checkout() {
    if [ "$source_denied" -eq 1 ] && [ "$source_restored" -eq 0 ]; then
        python3 "$lifecycle" restore --stash "$crates_stash" --destination "$crates_path"
        source_restored=1
    fi
}
cleanup_all() {
    restore_source_checkout
    python3 "$lifecycle" remove --root "$test_root" --path "$work_parent"         --purpose exact-candidate-isolated-install --token "$work_token" >/dev/null 2>&1 || true
    python3 "$lifecycle" remove --root "${TMPDIR:-/tmp}" --path "$test_root"         --purpose exact-candidate-isolated-install-test-root --token "$test_root_token" >/dev/null 2>&1 || true
}
trap cleanup_all EXIT

echo "isolated-install: extracting candidate crates outside the workspace"
python3 - "$PACKAGES_DIR" "$extracted_root" <<'PY'
import io
import json
import sys
import tarfile
from pathlib import Path

packages_dir = Path(sys.argv[1])
extracted_root = Path(sys.argv[2])
rows = json.loads(Path("target/exact-candidate-isolated-install/consumed-rows.json").read_text(encoding="utf-8"))
for row in rows:
    crate = packages_dir / f"{row['cargo_package_name']}-{row['cargo_package_version']}.crate"
    with tarfile.open(crate, "r:gz") as archive:
        archive.extractall(extracted_root, filter="data")
print(f"extracted {len(rows)} crates into {extracted_root.name}")
PY

extracted_bin_pkg="${extracted_root}/cargo-allow-${CANDIDATE_VERSION}"
if [ ! -d "$extracted_bin_pkg" ]; then
    echo "extracted root package missing: $extracted_bin_pkg" >&2
    exit 1
fi

echo "isolated-install: warming external dependencies through a patched fetch"
saved_lock="${work_parent}/extracted-Cargo.lock"
cp "${extracted_bin_pkg}/Cargo.lock" "$saved_lock"
{
    echo '[patch.crates-io]'
    python3 - target/exact-candidate-isolated-install/consumed-rows.json "$extracted_root" <<'PY'
import json
import sys
from pathlib import Path

rows = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
extracted_root = Path(sys.argv[2])
for row in rows:
    directory = extracted_root / f"{row['cargo_package_name']}-{row['cargo_package_version']}"
    print(f"{row['cargo_package_name']} = {{ path = {json.dumps(str(directory))} }}")
PY
} > "${warm_home}/config.toml"
CARGO_HOME="$warm_home" cargo fetch --locked --manifest-path "${extracted_bin_pkg}/Cargo.toml"
cp "$saved_lock" "${extracted_bin_pkg}/Cargo.lock"

echo "isolated-install: assembling the isolated local registry"
candidate_args=()
python3 - target/exact-candidate-isolated-install/consumed-rows.json <<'PY' > target/exact-candidate-isolated-install/candidate-args.txt
import json
import sys
rows = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
for row in rows:
    print(f"--candidate {row['cargo_package_name']}={row['cargo_package_version']}")
PY
while read -r flag name_version; do
    candidate_args+=("$flag" "$name_version")
done < target/exact-candidate-isolated-install/candidate-args.txt
python3 scripts/exact-candidate-assemble-local-registry.py \
    --lockfile "$saved_lock" \
    --cargo-home "$warm_home" \
    --packages-dir "$PACKAGES_DIR" \
    --output "$registry" \
    "${candidate_args[@]}"
registry_index_digest="$(python3 - "$registry" <<'PY'
import hashlib
import sys
from pathlib import Path

registry = Path(sys.argv[1])
lines = b""
for path in sorted((registry / "index").rglob("*")):
    if path.is_file():
        lines += path.read_bytes()
print("sha256:" + hashlib.sha256(lines).hexdigest())
PY
)"
echo "isolated-install: binding registry index checksums to the consumed rows"
python3 - "$registry" <<'PY'
import json
import sys
from pathlib import Path

registry = Path(sys.argv[1])
rows_path = Path("target/exact-candidate-isolated-install/consumed-rows.json")
rows = json.loads(rows_path.read_text(encoding="utf-8"))
checksums = {}
for path in sorted((registry / "index").rglob("*")):
    if not path.is_file():
        continue
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        entry = json.loads(line)
        checksums[(entry["name"], entry["vers"])] = entry["cksum"]
for row in rows:
    key = (row["cargo_package_name"], row["cargo_package_version"])
    if key not in checksums:
        raise SystemExit(f"registry index has no row for {key[0]} {key[1]}")
    row["index_checksum"] = checksums[key]
rows_path.write_text(json.dumps(rows, indent=2) + "
", encoding="utf-8")
print(f"bound {len(rows)} index checksums")
PY

echo "isolated-install: denying the workspace source checkout"
mv "$crates_path" "$crates_stash"
source_denied=1
if [ -d "$crates_path" ]; then
    echo "workspace crates/ tree is still present after denial" >&2
    exit 1
fi

echo "isolated-install: installing from the isolated registry with a fresh Cargo home"
mkdir -p "$install_home"
{
    echo '[source.crates-io]'
    echo 'replace-with = "candidate-local-registry"'
    echo ''
    echo '[source.candidate-local-registry]'
    echo "local-registry = \"$(python3 -c "import json;print(json.dumps(str('$registry').replace(chr(92), '/')))")\""
} > "${install_home}/config.toml"
set +e
CARGO_HOME="$install_home" CARGO_TARGET_DIR="${offline_root}/target" \
    cargo install --path "${extracted_bin_pkg}" --locked --root "${install_root}" --force --offline
install_exit=$?
set -e

mv "$crates_stash" "$crates_path"
source_restored=1
source_denied=0
if [ -d "$crates_path" ] && [ ! -d "$crates_stash" ]; then
    :
else
    echo "workspace source checkout was not restored" >&2
    exit 1
fi
if [ "$install_exit" -ne 0 ]; then
    echo "isolated install failed with exit $install_exit" >&2
    exit 1
fi

installed_bin="${install_root}/bin/cargo-allow"
if [ ! -f "$installed_bin" ]; then
    installed_bin="${install_root}/bin/cargo-allow.exe"
fi
installed_version_output="$(CARGO_HOME="$install_home" "$installed_bin" --version)"
echo "isolated-install: installed binary reports ${installed_version_output}"
case "$installed_version_output" in
    *"cargo-allow ${CANDIDATE_VERSION}"*) ;;
    *) echo "installed binary identity mismatch: ${installed_version_output}" >&2; exit 1 ;;
esac
installed_executable_digest="$(python3 -c "import hashlib,sys;print('sha256:'+hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" "$installed_bin")"

echo "isolated-install: comparing the actual resolved graph with the candidate rows"
CARGO_HOME="$install_home" cargo metadata --format-version 1 --offline \
    --manifest-path "${extracted_bin_pkg}/Cargo.toml" \
    > "${offline_root}/resolved-metadata.json"
python3 scripts/exact-candidate-isolated-install.py --mode compare \
    --metadata "${offline_root}/resolved-metadata.json" \
    --candidate-artifact "$CANDIDATE_ARTIFACT" \
    > "${offline_root}/graph-comparison.json"
comparison_digest_input="${offline_root}/graph-comparison.json"

echo "isolated-install: negative controls"
NEGATIVE_FAILURES=0

# Negative: missing selected .crate (registry copy without the crate file)
rm -rf "${work_parent}/registry-missing-crate" && cp -r "$registry" "${work_parent}/registry-missing-crate"
missing_crate="$(find "${work_parent}/registry-missing-crate" -name "allow-core-${CANDIDATE_VERSION}.crate" | head -1)"
rm -f "$missing_crate"
set +e
CARGO_HOME="$install_home" cargo metadata --format-version 1 --offline \
    --manifest-path "${extracted_bin_pkg}/Cargo.toml" \
    --config "source.crates-io.replace-with='candidate-local-registry'" \
    --config "source.candidate-local-registry.local-registry='${work_parent}/registry-missing-crate'" \
    > /dev/null 2>&1
missing_crate_exit=$?
set -e
if [ "$missing_crate_exit" -eq 0 ]; then
    echo "negative missing-crate unexpectedly succeeded" >&2
    NEGATIVE_FAILURES=$((NEGATIVE_FAILURES + 1))
fi

# Negative: candidate checksum mutated
rm -rf "${work_parent}/registry-mutated" && cp -r "$registry" "${work_parent}/registry-mutated"
mutated_crate="$(find "${work_parent}/registry-mutated" -name "allow-core-${CANDIDATE_VERSION}.crate" | head -1)"
printf 'x' >> "$mutated_crate"
set +e
CARGO_HOME="$install_home" cargo metadata --format-version 1 --offline \
    --manifest-path "${extracted_bin_pkg}/Cargo.toml" \
    --config "source.crates-io.replace-with='candidate-local-registry'" \
    --config "source.candidate-local-registry.local-registry='${work_parent}/registry-mutated'" \
    > /dev/null 2>&1
mutated_exit=$?
set -e
if [ "$mutated_exit" -eq 0 ]; then
    echo "negative checksum-mutation unexpectedly succeeded" >&2
    NEGATIVE_FAILURES=$((NEGATIVE_FAILURES + 1))
fi

# Negative: index checksum mismatch
rm -rf "${work_parent}/registry-index" && cp -r "$registry" "${work_parent}/registry-index"
index_row="$(find "${work_parent}/registry-index/index" -type f | head -1)"
python3 - "$index_row" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
lines = [line for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
entries = [json.loads(line) for line in lines]
for entry in entries:
    if entry.get("cksum"):
        entry["cksum"] = "sha256:" + "0" * 64
        break
path.write_text("\n".join(json.dumps(entry) for entry in entries) + "\n", encoding="utf-8")
PY
set +e
CARGO_HOME="$install_home" cargo metadata --format-version 1 --offline \
    --manifest-path "${extracted_bin_pkg}/Cargo.toml" \
    --config "source.crates-io.replace-with='candidate-local-registry'" \
    --config "source.candidate-local-registry.local-registry='${work_parent}/registry-index'" \
    > /dev/null 2>&1
index_exit=$?
set -e
if [ "$index_exit" -eq 0 ]; then
    echo "negative index-checksum mismatch unexpectedly succeeded" >&2
    NEGATIVE_FAILURES=$((NEGATIVE_FAILURES + 1))
fi

# Negative: offline external input incomplete
rm -rf "${work_parent}/registry-external" && cp -r "$registry" "${work_parent}/registry-external"
first_external="$(find "${work_parent}/registry-external" -name "*.crate" ! -name "allow-*" ! -name "effortless-*" ! -name "cargo-allow-*" | head -1)"
if [ -n "$first_external" ]; then
    rm -f "$first_external"
    set +e
    CARGO_HOME="$install_home" cargo metadata --format-version 1 --offline \
        --manifest-path "${extracted_bin_pkg}/Cargo.toml" \
        --config "source.crates-io.replace-with='candidate-local-registry'" \
        --config "source.candidate-local-registry.local-registry='${work_parent}/registry-external'" \
        > /dev/null 2>&1
    external_exit=$?
    set -e
    if [ "$external_exit" -eq 0 ]; then
        echo "negative incomplete-external-input unexpectedly succeeded" >&2
        NEGATIVE_FAILURES=$((NEGATIVE_FAILURES + 1))
    fi
fi

# Negative: a stale ambient cargo-allow on PATH cannot satisfy the identity
# check (the installed binary is invoked by absolute path).
shadow_dir="${work_parent}/shadow-bin"
mkdir -p "$shadow_dir"
printf '#!/usr/bin/env bash\nexit 42\n' > "${shadow_dir}/cargo-allow"
chmod +x "${shadow_dir}/cargo-allow"
shadow_version="$(PATH="${shadow_dir}:${PATH}" "$installed_bin" --version || true)"
case "$shadow_version" in
    *"cargo-allow ${CANDIDATE_VERSION}"*) ;;
    *) echo "ambient shadow satisfied the identity check" >&2
       NEGATIVE_FAILURES=$((NEGATIVE_FAILURES + 1)) ;;
esac

# Characterized negatives (offline classification, no Cargo invocation):
# extra unselected package, wrong versions, compatible-but-unselected
# version, stale identity, receipt redaction.
python3 - "$CANDIDATE_ARTIFACT" <<'PY'
import copy
import json
import sys

sys.path.insert(0, "scripts")
import importlib.util

spec = importlib.util.spec_from_file_location(
    "isolated", "scripts/exact-candidate-isolated-install.py"
)
isolated = importlib.util.module_from_spec(spec)
spec.loader.exec_module(isolated)

artifact = json.loads(open(sys.argv[1], encoding="utf-8").read())
rows = artifact["rows"]

def metadata_with(overrides):
    packages = []
    for row in rows:
        entry = {
            "name": row["cargo_package_name"],
            "version": row["cargo_package_version"],
            "manifest_path": f"/checkout/target/package/{row['cargo_package_name']}-{row['cargo_package_version']}/Cargo.toml",
        }
        packages.append(overrides.get(entry["name"], entry))
    packages.extend(overrides.get("__extra__", []))
    return {"packages": packages, "workspace_members": []}

failures = []

# extra unselected intent package
meta = metadata_with({})
meta["packages"].append({"name": "intent-model", "version": "0.1.0", "manifest_path": "/x/Cargo.toml"})
comparison = isolated.compare_resolution(meta, rows)
if comparison["unexpected_packages"] != ["intent-model"]:
    failures.append("extra intent package not detected")

# wrong shared version and compatible-but-unselected internal version
meta = metadata_with({})
for package in meta["packages"]:
    if package["name"] == "effortless-repo-protocol":
        package["version"] = "0.1.1"
    if package["name"] == "allow-core":
        package["version"] = "0.2.0-rc.2"
comparison = isolated.compare_resolution(meta, rows)
if not comparison["version_mismatches"]:
    failures.append("version mismatch not detected")

# path source leak
meta = metadata_with({})
meta["packages"][0]["manifest_path"] = "/checkout/crates/allow-core/Cargo.toml"
comparison = isolated.compare_resolution(meta, rows)
if not comparison["path_sources"]:
    failures.append("workspace path source not detected")

# receipt classification: stale identity and redaction
payload = {
    "schema_id": "cargo-allow.isolated-install.v2",
    "schema_version": 2,
    "source_checkout_denied": True,
    "graph_comparison": {"expected_packages": 1, "matched_packages": 1},
    "cargo_lock_digest": "sha256:short",
}
if isolated.classify(payload) != "StaleInput":
    failures.append("stale identity not classified")
payload["cargo_lock_digest"] = "sha256:" + "a" * 64
payload["external_cache_identity"] = "/home/runner/work/private"
if isolated.classify(payload) != "PathLeakInReceipt":
    failures.append("private path not classified")
payload["external_cache_identity"] = "sha256:" + "b" * 64
payload["source_checkout_denied"] = False
if isolated.classify(payload) != "SourceFallbackDetected":
    failures.append("source fallback not classified")

if failures:
    for failure in failures:
        print(f"characterized negative failed: {failure}", file=sys.stderr)
    sys.exit(1)
print("characterized negatives: extra/sibling/version/path/stale/redaction/fallback all detected")
PY
if [ $? -ne 0 ]; then
    NEGATIVE_FAILURES=$((NEGATIVE_FAILURES + 1))
fi

if [ "$NEGATIVE_FAILURES" -ne 0 ]; then
    echo "${NEGATIVE_FAILURES} negative control(s) failed" >&2
    exit 1
fi

echo "isolated-install: assembling the typed receipt"
install_root_identity="$(python3 -c "import hashlib;print('sha256:'+hashlib.sha256('$install_root'.encode()).hexdigest())")"
cargo_home_identity="$(python3 -c "import hashlib;print('sha256:'+hashlib.sha256('$install_home'.encode()).hexdigest())")"
external_cache_identity="$(python3 -c "import hashlib;print('sha256:'+hashlib.sha256(open('$saved_lock','rb').read()).hexdigest())")"
commit="$(git rev-parse HEAD)"
tree="$(git rev-parse HEAD^{tree})"
cargo_lock_digest="$(python3 -c "import hashlib;print('sha256:'+hashlib.sha256(open('$saved_lock','rb').read()).hexdigest())")"
platform="$(rustc -vV | grep '^host:' | cut -d' ' -f2)"
toolchain="$(rustc -vV | grep 'release:' | cut -d' ' -f2)"

python3 scripts/exact-candidate-isolated-install.py --mode assemble \
    --candidate-artifact "$CANDIDATE_ARTIFACT" \
    --receipt-out "${OUTPUT_DIR}/isolated-install.receipt.json" \
    --params <(python3 - "$CANDIDATE_ARTIFACT" "${offline_root}/graph-comparison.json" \
        "$commit" "$tree" "$cargo_lock_digest" "$registry_index_digest" \
        "$external_cache_identity" "$install_root_identity" "$cargo_home_identity" \
        "$installed_executable_digest" "$installed_version_output" "$platform" "$toolchain" <<'PY'
import json
import sys
from pathlib import Path

artifact = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
graph = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
params = {
    "candidate_rows": json.loads(
        Path("target/exact-candidate-isolated-install/consumed-rows.json").read_text(
            encoding="utf-8"
        )
    ),
    "graph_comparison": graph,
    "repository_commit": sys.argv[3],
    "repository_tree": sys.argv[4],
    "cargo_lock_digest": sys.argv[5],
    "registry_index_digest": sys.argv[6],
    "external_cache_identity": sys.argv[7],
    "source_checkout_denied": True,
    "install_root_identity": sys.argv[8],
    "cargo_home_identity": sys.argv[9],
    "installed_executable_digest": sys.argv[10],
    "installed_version_output": sys.argv[11],
    "platform": sys.argv[12],
    "toolchain": sys.argv[13],
}
print(json.dumps(params, indent=2))
PY
)
classification="$(python3 scripts/exact-candidate-isolated-install.py --mode classify \
    --input-receipt "${OUTPUT_DIR}/isolated-install.receipt.json")"
if [ "$classification" != "Complete" ]; then
    echo "receipt classified ${classification}, expected Complete" >&2
    exit 1
fi
echo "isolated-install: receipt Complete at ${OUTPUT_DIR}/isolated-install.receipt.json"
