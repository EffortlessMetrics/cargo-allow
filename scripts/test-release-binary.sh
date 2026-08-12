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

topology_receipt="${work}/topology-publish.receipt.json"
python3 - "${topology_receipt}" <<'PY'
import json
import sys

rows = []
for order, name in enumerate([
    "allow-core", "allow-policy", "allow-inventory", "allow-files",
    "allow-rust", "allow-match", "allow-policy-legacy", "allow-report",
    "allow-diff", "cargo-allow",
], 1):
    rows.append({
        "logical_id": name,
        "name": name,
        "version": "9.9.9",
        "release_order": order,
        "local_checksum": "sha256:" + ("a" * 64),
        "registry_checksum": None,
    })
json.dump({
    "schema_id": "cargo-allow.topology-publish-receipt.v1",
    "schema_version": 1,
    "mode": "cargo-allow",
    "publish": False,
    "topology_id": "fixture-topology",
    "topology_sha256": "b" * 64,
    "cargo_lock_sha256": "c" * 64,
    "commit": "fixture-commit",
    "tree": "fixture-tree",
    "rows": rows,
    "complete": True,
}, open(sys.argv[1], "w", encoding="utf-8"), indent=2)
PY
VERSION=9.9.9 REPOSITORY=EffortlessMetrics/cargo-allow TAG=v9.9.9 \
  COMMIT=fixture-commit TREE=fixture-tree AUTH_SOURCE=crates_io_api_token MSRV=1.95 \
  PLATFORMS=linux WORKFLOW_RUN_ID=123 RUST_TOOLCHAIN=stable RUNNER=ubuntu-latest \
  BINARY_PACKAGE_RECEIPT="${output}/release-binary.receipt.json" \
  BINARY_INSTALL_RECEIPT="${receipt_path}" TOPOLOGY_RECEIPT="${topology_receipt}" OUTPUT="${manifest_output}" \
  bash scripts/generate-release-manifest.sh >/dev/null
python3 - "${manifest_output}" <<'PY'
import json
import sys

manifest = json.loads(open(sys.argv[1], encoding="utf-8").read())
assert manifest["payload"]["schema_id"] == "cargo-allow.release-manifest.v2"
assert manifest["payload"]["authentication"] == "crates_io_api_token"
assert len(manifest["payload"]["package_rows"]) == 10
assert manifest["payload"]["package_rows"][0]["logical_id"] == "allow-core"
assert manifest["instrument_diagnostics"]
PY

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
python3 - "${work}/complete-manifest.json" <<'PY'
import json
import sys

manifest = json.loads(open(sys.argv[1], encoding="utf-8").read())
assert manifest["payload"]["publication_posture"] == "unpublished"
assert len(manifest["payload"]["package_rows"]) == 10
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
