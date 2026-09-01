#!/usr/bin/env bash
# Fixture contract tests for the Linux release binary scripts (#2464).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
work="$(mktemp -d "${TMPDIR:-/tmp}/cargo-allow-release-test.XXXXXX")"
receipt_path="${RECEIPT_PATH:-${ROOT}/target/release-binary-contract/release-binary-test.receipt.json}"
mkdir -p "$(dirname "${receipt_path}")"
package_fixture_paths=()
cleanup() {
  rm -rf "${work:-}"
  for path in "${package_fixture_paths[@]}"; do
    rm -f "${path}"
  done
}
trap cleanup EXIT

fixture_bin="${work}/fixture-cargo-allow"
cat >"${fixture_bin}" <<'EOF'
#!/usr/bin/env bash
case "${1:-}" in
  --version) printf 'cargo-allow 9.9.9\n' ;;
  doctor|audit|--help) ;;
  init) mkdir -p policy; printf '# fixture\n' > policy/allow.toml ;;
  check) ;;
  *) printf 'unsupported fixture command\n' >&2; exit 2 ;;
esac
EOF
chmod 0755 "${fixture_bin}"

output="${work}/assets"
CARGO_ALLOW_BIN="${fixture_bin}" VERSION=9.9.9 RELEASE_TAG=v9.9.9 \
  RELEASE_COMMIT=fixture-commit RELEASE_TREE=fixture-tree \
  bash scripts/package-release-binary.sh --output-dir "${output}" >/dev/null
archive="${output}/cargo-allow-v9.9.9-x86_64-unknown-linux-gnu.tar.gz"
for mask in 022 077; do
  reproducible_output="${work}/umask-${mask}"
  (
    umask "${mask}"
    CARGO_ALLOW_BIN="${fixture_bin}" VERSION=9.9.9 RELEASE_TAG=v9.9.9 \
      RELEASE_COMMIT=fixture-commit RELEASE_TREE=fixture-tree \
      bash scripts/package-release-binary.sh --output-dir "${reproducible_output}" >/dev/null
  )
done
umask_archive_022="${work}/umask-022/$(basename "${archive}")"
umask_archive_077="${work}/umask-077/$(basename "${archive}")"
[[ "$(sha256sum "${umask_archive_022}" | awk '{print $1}')" == \
   "$(sha256sum "${umask_archive_077}" | awk '{print $1}')" ]] \
  || { printf 'archive changed with umask\n' >&2; exit 1; }
bash scripts/verify-release-binary.sh --version 9.9.9 "${archive}" >/dev/null
rm -f "${receipt_path}"
RELEASE_TAG=v9.9.9 RELEASE_COMMIT=fixture-commit RELEASE_TREE=fixture-tree \
  bash scripts/verify-release-binary.sh --version 9.9.9 --receipt "${receipt_path}" "${archive}" >/dev/null

manifest_output="${work}/release-manifest-v2.json"
expect_failure() {
  if "$@" >/dev/null 2>&1; then
    printf 'expected failure did not occur: %s\n' "$*" >&2
    exit 1
  fi
}

python3 scripts/test-release-topology-publisher.py
python3 scripts/test-final-package-docs.py

topology_receipt="${work}/topology-publish.receipt.json"
python3 - "${topology_receipt}" "${ROOT}/docs/dogfood/fixtures/release/candidate-crate-set.toml" "${ROOT}/scripts/release-topology-publisher.py" <<'PY'
import json
import importlib.util
import sys
import tomllib

spec = importlib.util.spec_from_file_location("release_topology_publisher", sys.argv[3])
if spec is None or spec.loader is None:
    raise SystemExit("could not load release topology publisher")
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)

rows = []
with open(sys.argv[2], "rb") as source:
    candidate = tomllib.load(source)["crates"]
cargo_rows = [name for name in candidate if not name.startswith("effortless-")]
for order, name in enumerate(cargo_rows, 1):
    rows.append({
        "logical_id": name,
        "name": name,
        "version": "9.9.9",
        "release_order": order,
        "local_checksum": publisher.receipt_checksum("a" * 64, field="fixture local checksum"),
        "registry_checksum": None,
    })
json.dump({
    "schema_id": "cargo-allow.topology-publish-receipt.v1",
    "schema_version": 1,
    "mode": "cargo-allow",
    "publish": False,
    "authorization": "fixture-rehearsal",
    "topology_id": "fixture-topology",
    "topology_sha256": "b" * 64,
    "cargo_lock_sha256": "c" * 64,
    "commit": "fixture-commit",
    "tree": "fixture-tree",
    "rows": rows,
    "complete": True,
    "incident_state": "none",
    "first_irreversible_row": None,
}, open(sys.argv[1], "w", encoding="utf-8"), indent=2)
PY
VERSION=9.9.9 REPOSITORY=EffortlessMetrics/cargo-allow TAG=v9.9.9 \
  COMMIT=fixture-commit TREE=fixture-tree AUTH_SOURCE=crates_io_api_token MSRV=1.95 \
  PLATFORMS=linux WORKFLOW_RUN_ID=123 RUST_TOOLCHAIN=stable RUNNER=ubuntu-latest \
  BINARY_PACKAGE_RECEIPT="${output}/release-binary.receipt.json" \
  BINARY_INSTALL_RECEIPT="${receipt_path}" TOPOLOGY_RECEIPT="${topology_receipt}" OUTPUT="${manifest_output}" \
  bash scripts/generate-release-manifest.sh >/dev/null
python3 - "${manifest_output}" "${ROOT}/docs/dogfood/fixtures/release/candidate-crate-set.toml" <<'PY'
import json
import sys
import tomllib

manifest = json.loads(open(sys.argv[1], encoding="utf-8").read())
with open(sys.argv[2], "rb") as source:
    candidate = tomllib.load(source)["crates"]
assert manifest["payload"]["schema_id"] == "cargo-allow.release-manifest.v2"
assert manifest["payload"]["authentication"] == "crates_io_api_token"
rows = manifest["payload"]["package_rows"]
cargo_candidate = [name for name in candidate if not name.startswith("effortless-")]
assert [row["logical_id"] for row in rows] == cargo_candidate
assert [row["package_version"] for row in rows] == ["9.9.9" for name in cargo_candidate]
assert manifest["instrument_diagnostics"]
PY

published_receipt="${work}/published-topology.receipt.json"
cp "${topology_receipt}" "${published_receipt}"
python3 - "${published_receipt}" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
receipt = json.loads(path.read_text(encoding="utf-8"))
receipt["publish"] = True
receipt["complete"] = True
receipt["incident_state"] = "none"
receipt["first_irreversible_row"] = receipt["rows"][0]["release_order"]
for row in receipt["rows"]:
    row["state"] = "published_verified"
    if not row["local_checksum"].startswith("sha256:"):
        row["local_checksum"] = "sha256:" + row["local_checksum"]
    row["registry_checksum"] = row["local_checksum"]
path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
PY
VERSION=9.9.9 REPOSITORY=EffortlessMetrics/cargo-allow TAG=v9.9.9 \
  COMMIT=fixture-commit TREE=fixture-tree AUTH_SOURCE=crates_io_api_token MSRV=1.95 \
  TOPOLOGY_RECEIPT="${published_receipt}" OUTPUT="${work}/published-manifest.json" \
  bash scripts/generate-release-manifest.sh >/dev/null
python3 - "${work}/published-manifest.json" <<'PY'
import json
import sys

manifest = json.loads(open(sys.argv[1], encoding="utf-8").read())
assert manifest["payload"]["publication_posture"] == "published"
PY

# The typed release-identity authority owns version grammar (#3752): a
# SemVer form the old embedded regex accepted (build metadata) must now
# fail manifest generation.
build_metadata_receipt="${work}/build-metadata-topology.receipt.json"
cp "${topology_receipt}" "${build_metadata_receipt}"
python3 - "${build_metadata_receipt}" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
receipt = json.loads(path.read_text(encoding="utf-8"))
receipt["rows"][0]["version"] = "9.9.9+build.7"
path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
PY
expect_failure env VERSION=9.9.9 REPOSITORY=EffortlessMetrics/cargo-allow TAG=v9.9.9 \
  COMMIT=fixture-commit TREE=fixture-tree AUTH_SOURCE=crates_io_api_token MSRV=1.95 \
  TOPOLOGY_RECEIPT="${build_metadata_receipt}" OUTPUT="${work}/build-metadata-manifest.json" \
  bash scripts/generate-release-manifest.sh
printf 'ok typed authority rejects semver build metadata in manifest rows\n'

# A verified_existing row whose registry bytes differ from the candidate's
# local package must not masquerade as exact in the manifest (#3758/#3761).
verified_existing_receipt="${work}/verified-existing-topology.receipt.json"
cp "${published_receipt}" "${verified_existing_receipt}"
python3 - "${verified_existing_receipt}" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
receipt = json.loads(path.read_text(encoding="utf-8"))
row = receipt["rows"][0]
row["state"] = "verified_existing"
row["registry_checksum"] = "sha256:" + ("d" * 64)
path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
PY
expect_failure env VERSION=9.9.9 REPOSITORY=EffortlessMetrics/cargo-allow   TAG=v9.9.9 COMMIT=fixture-commit TREE=fixture-tree AUTH_SOURCE=crates_io_api_token MSRV=1.95   TOPOLOGY_RECEIPT="${verified_existing_receipt}" OUTPUT="${work}/verified-existing-manifest.json"   bash scripts/generate-release-manifest.sh
printf 'ok verified_existing registry disagreement is rejected\n'

# Every manifest row must carry the selected release identity version.
version_mismatch_receipt="${work}/version-mismatch-topology.receipt.json"
cp "${published_receipt}" "${version_mismatch_receipt}"
python3 - "${version_mismatch_receipt}" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
receipt = json.loads(path.read_text(encoding="utf-8"))
receipt["rows"][0]["version"] = "9.9.8"
path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
PY
expect_failure env VERSION=9.9.9 REPOSITORY=EffortlessMetrics/cargo-allow   TAG=v9.9.9 COMMIT=fixture-commit TREE=fixture-tree AUTH_SOURCE=crates_io_api_token MSRV=1.95   TOPOLOGY_RECEIPT="${version_mismatch_receipt}" OUTPUT="${work}/version-mismatch-manifest.json"   bash scripts/generate-release-manifest.sh
printf 'ok manifest rows must carry the selected release identity version\n'

checksum_conflict_receipt="${work}/checksum-conflict-topology.receipt.json"
cp "${published_receipt}" "${checksum_conflict_receipt}"
python3 - "${checksum_conflict_receipt}" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
receipt = json.loads(path.read_text(encoding="utf-8"))
receipt["rows"][0]["registry_checksum"] = "sha256:" + ("0" * 64)
path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
PY
expect_failure env VERSION=9.9.9 REPOSITORY=EffortlessMetrics/cargo-allow \
  TAG=v9.9.9 COMMIT=fixture-commit TREE=fixture-tree AUTH_SOURCE=crates_io_api_token MSRV=1.95 \
  TOPOLOGY_RECEIPT="${checksum_conflict_receipt}" OUTPUT="${work}/checksum-conflict-manifest.json" \
  bash scripts/generate-release-manifest.sh

partial_receipt="${work}/partial-topology.receipt.json"
cp "${published_receipt}" "${partial_receipt}"
python3 - "${partial_receipt}" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
receipt = json.loads(path.read_text(encoding="utf-8"))
receipt["complete"] = False
receipt["incident_state"] = "partial"
receipt["first_irreversible_row"] = receipt["rows"][0]["release_order"]
receipt["rows"][0]["state"] = "uploaded_waiting_for_registry"
path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
PY
expect_failure env VERSION=9.9.9 REPOSITORY=EffortlessMetrics/cargo-allow \
  TAG=v9.9.9 COMMIT=fixture-commit TREE=fixture-tree AUTH_SOURCE=crates_io_api_token MSRV=1.95 \
  TOPOLOGY_RECEIPT="${partial_receipt}" OUTPUT="${work}/partial-manifest.json" \
  bash scripts/generate-release-manifest.sh

bad_identity_receipt="${work}/bad-identity.receipt.json"
cp "${topology_receipt}" "${bad_identity_receipt}"
python3 - "${bad_identity_receipt}" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
receipt = json.loads(path.read_text(encoding="utf-8"))
receipt["commit"] = "other-commit"
path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
PY
expect_failure env VERSION=9.9.9 REPOSITORY=EffortlessMetrics/cargo-allow \
  TAG=v9.9.9 COMMIT=fixture-commit TREE=fixture-tree AUTH_SOURCE=crates_io_api_token MSRV=1.95 \
  BINARY_PACKAGE_RECEIPT="${output}/release-binary.receipt.json" \
  BINARY_INSTALL_RECEIPT="${receipt_path}" TOPOLOGY_RECEIPT="${bad_identity_receipt}" \
  OUTPUT="${work}/identity-conflict-manifest.json" \
  bash scripts/generate-release-manifest.sh

bad_publish_receipt="${work}/bad-publish.receipt.json"
cp "${topology_receipt}" "${bad_publish_receipt}"
python3 - "${bad_publish_receipt}" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
receipt = json.loads(path.read_text(encoding="utf-8"))
receipt["publish"] = "false"
path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
PY
expect_failure env VERSION=9.9.9 REPOSITORY=EffortlessMetrics/cargo-allow \
  TAG=v9.9.9 COMMIT=fixture-commit TREE=fixture-tree AUTH_SOURCE=crates_io_api_token MSRV=1.95 \
  TOPOLOGY_RECEIPT="${bad_publish_receipt}" OUTPUT="${work}/bad-publish-manifest.json" \
  bash scripts/generate-release-manifest.sh

ATTESTATION_VERIFIED=true RELEASE_TAG=v9.9.9 RELEASE_COMMIT=fixture-commit \
  RELEASE_TREE=fixture-tree \
  bash scripts/verify-release-binary.sh --version 9.9.9 --receipt "${receipt_path}" "${archive}" >/dev/null
VERSION=9.9.9 REPOSITORY=EffortlessMetrics/cargo-allow TAG=v9.9.9 \
  COMMIT=fixture-commit TREE=fixture-tree AUTH_SOURCE=crates_io_api_token MSRV=1.95 \
  PLATFORMS=linux WORKFLOW_RUN_ID=123 RUST_TOOLCHAIN=stable RUNNER=ubuntu-latest \
  BINARY_PACKAGE_RECEIPT="${output}/release-binary.receipt.json" \
  BINARY_INSTALL_RECEIPT="${receipt_path}" TOPOLOGY_RECEIPT="${topology_receipt}" OUTPUT="${work}/complete-manifest.json" \
  bash scripts/generate-release-manifest.sh >/dev/null
python3 - "${work}/complete-manifest.json" "${ROOT}/docs/dogfood/fixtures/release/candidate-crate-set.toml" <<'PY'
import json
import sys
import tomllib

manifest = json.loads(open(sys.argv[1], encoding="utf-8").read())
with open(sys.argv[2], "rb") as source:
    candidate = [
        name
        for name in tomllib.load(source)["crates"]
        if not name.startswith("effortless-")
    ]
assert manifest["payload"]["publication_posture"] == "unpublished"
assert len(manifest["payload"]["package_rows"]) == len(candidate)
PY

manifest_checksum="${work}/complete-manifest.sha256"
sha256sum "${work}/complete-manifest.json" | awk '{print $1 "  complete-manifest.json"}' >"${manifest_checksum}"
(
  cd "${work}"
  sha256sum -c "$(basename "${manifest_checksum}")" >/dev/null
)

expect_failure env VERSION=9.9.9 REPOSITORY=EffortlessMetrics/cargo-allow \
  TAG=v9.9.9 COMMIT=fixture-commit TREE=fixture-tree AUTH_SOURCE=crates_io_api_token MSRV=1.95 \
  BINARY_PACKAGE_RECEIPT="${output}/release-binary.receipt.json" \
  OUTPUT="${work}/missing-install-manifest.json" \
  bash scripts/generate-release-manifest.sh

cp "${archive}.sha256" "${work}/missing.sha256"
rm "${archive}.sha256"
expect_failure bash scripts/verify-release-binary.sh "${archive}"
mv "${work}/missing.sha256" "${archive}.sha256"
cp "${archive}.executable.sha256" "${work}/missing-executable.sha256"
rm "${archive}.executable.sha256"
expect_failure bash scripts/verify-release-binary.sh "${archive}"
mv "${work}/missing-executable.sha256" "${archive}.executable.sha256"

expect_failure bash scripts/verify-release-binary.sh --version 8.8.8 "${archive}"

tampered_archive="${work}/tampered-archive.tar.gz"
cp "${archive}" "${tampered_archive}"
printf '%s  %s\n' "$(sha256sum "${archive}" | awk '{print $1}')" "$(basename "${tampered_archive}")" >"${tampered_archive}.sha256"
cp "${archive}.executable.sha256" "${tampered_archive}.executable.sha256"
printf 'tampered\n' >>"${tampered_archive}"
expect_failure bash scripts/verify-release-binary.sh "${tampered_archive}"

wrong_sidecar_archive="${work}/wrong-sidecar.tar.gz"
cp "${archive}" "${wrong_sidecar_archive}"
cp "${archive}.sha256" "${wrong_sidecar_archive}.sha256"
cp "${archive}.executable.sha256" "${wrong_sidecar_archive}.executable.sha256"
expect_failure bash scripts/verify-release-binary.sh "${wrong_sidecar_archive}"

mkdir -p "${work}/extra"
extra_archive="${work}/extra/cargo-allow-v9.9.9-x86_64-unknown-linux-gnu.tar.gz"
python3 - "${archive}" "${extra_archive}" <<'PY'
import io
import sys
import tarfile

source, destination = sys.argv[1:]
with tarfile.open(source, "r:gz") as source_tar, tarfile.open(destination, "w:gz") as destination_tar:
    for member in source_tar.getmembers():
        data = source_tar.extractfile(member).read() if member.isfile() else None
        destination_tar.addfile(member, io.BytesIO(data) if data is not None else None)
    extra = tarfile.TarInfo("cargo-allow-v9.9.9-x86_64-unknown-linux-gnu/unexpected.txt")
    extra.size = 0
    destination_tar.addfile(extra)
PY
printf '%s  %s\n' "$(sha256sum "${extra_archive}" | awk '{print $1}')" "$(basename "${extra_archive}")" >"${extra_archive}.sha256"
cp "${archive}.executable.sha256" "${extra_archive}.executable.sha256"
expect_failure bash scripts/verify-release-binary.sh "${extra_archive}"

mkdir -p "${work}/duplicate"
duplicate_archive="${work}/duplicate/cargo-allow-v9.9.9-x86_64-unknown-linux-gnu.tar.gz"
python3 - "${archive}" "${duplicate_archive}" <<'PY'
import io
import sys
import tarfile

source, destination = sys.argv[1:]
with tarfile.open(source, "r:gz") as source_tar, tarfile.open(destination, "w:gz") as destination_tar:
    for member in source_tar.getmembers():
        data = source_tar.extractfile(member).read() if member.isfile() else None
        destination_tar.addfile(member, io.BytesIO(data) if data is not None else None)
    duplicate = tarfile.TarInfo("cargo-allow-v9.9.9-x86_64-unknown-linux-gnu/README.md")
    duplicate.size = 0
    destination_tar.addfile(duplicate)
PY
printf '%s  %s\n' "$(sha256sum "${duplicate_archive}" | awk '{print $1}')" "$(basename "${duplicate_archive}")" >"${duplicate_archive}.sha256"
cp "${archive}.executable.sha256" "${duplicate_archive}.executable.sha256"
expect_failure bash scripts/verify-release-binary.sh "${duplicate_archive}"

mkdir -p "${work}/unsafe"
unsafe_archive="${work}/unsafe/cargo-allow-v9.9.9-x86_64-unknown-linux-gnu.tar.gz"
python3 - "${archive}" "${unsafe_archive}" <<'PY'
import io
import sys
import tarfile

source, destination = sys.argv[1:]
with tarfile.open(source, "r:gz") as source_tar, tarfile.open(destination, "w:gz") as destination_tar:
    for member in source_tar.getmembers():
        data = source_tar.extractfile(member).read() if member.isfile() else None
        destination_tar.addfile(member, io.BytesIO(data) if data is not None else None)
    escape = tarfile.TarInfo("cargo-allow-v9.9.9-x86_64-unknown-linux-gnu/../escape")
    escape.size = 0
    destination_tar.addfile(escape)
PY
printf '%s  %s\n' "$(sha256sum "${unsafe_archive}" | awk '{print $1}')" "$(basename "${unsafe_archive}")" >"${unsafe_archive}.sha256"
cp "${archive}.executable.sha256" "${unsafe_archive}.executable.sha256"
expect_failure bash scripts/verify-release-binary.sh "${unsafe_archive}"

bad_archive="${work}/cargo-allow-v9.9.9-x86_64-unknown-linux-gnu-bad.tar.gz"
cp "${archive}" "${bad_archive}"
expect_failure bash scripts/verify-release-binary.sh "${bad_archive}"

python3 - "${receipt_path}" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
receipt = json.loads(path.read_text(encoding="utf-8"))
receipt["schema_id"] = "cargo-allow.release-binary-contract-test.v1"
receipt["negative_controls"] = [
    "missing_archive_checksum",
    "missing_executable_checksum",
    "wrong_version",
    "tampered_archive",
    "wrong_sidecar_name",
    "unexpected_archive_entry",
    "duplicate_archive_entry",
    "unsafe_archive_path",
    "invalid_archive_name",
]
receipt["claim_boundary"] = "Fixture packaging and verification controls passed; Linux release publication and GitHub attestation are not claimed."
path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
PY

printf 'ok release binary packaging and verification negative controls\n'
