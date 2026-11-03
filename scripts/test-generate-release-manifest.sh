#!/usr/bin/env bash
# Contract tests for scripts/generate-release-manifest.sh (#3761).
#
# Proves, against a hand-authored published topology receipt:
#   1. the happy path embeds the typed ReleaseManifestV2 envelope (schema
#      identity, published posture, equality-proven per-row digests, and
#      consumed evidence retained per receipt with producer identity);
#   2. a package version that is a valid typed release identity but is not
#      the selected release identity is refused, naming the refusal.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

work="$(mktemp -d "${TMPDIR:-/tmp}/cargo-allow-manifest-contract.XXXXXX")"
cleanup() {
  rm -rf "${work:-}"
}
trap cleanup EXIT

expect_failure() {
  local label="$1"
  shift
  if "$@" >"${work}/out.log" 2>"${work}/err.log"; then
    printf 'expected failure did not occur: %s\n' "${label}" >&2
    exit 1
  fi
}

receipt="${work}/topology-publish.receipt.json"
python3 - "${receipt}" <<'PY'
import json
import pathlib
import sys

# A published cargo-allow topology receipt in the production shape: ten
# uniform-version rows, each registry-verified with registry bytes equal to
# the candidate package.
digests = [
    "62b99437bc93528f3da331c6cafa654ab669faaf76a15ec590c680ffdf6e6275",
    "74f46a15622f26108c97923cd8b9fe46234412ab24a2da388c515dfa49583684",
    "d3e668bf3e78734654aba4da919a7881c78b3465db46db0a06d86820293a2cfe",
    "269d4a06282f9ef5ebc6d234999c785c3f0670cb6649193b1117dd9a503e8dc4",
    "6cfb93c0eafcda2b21234b15f18446d25360e7fd9d0ec568f10aef7657a89733",
    "74a3acff8819928c7318e7a73b2c116810fe9dca75d5fa9dd4df25c6710c5997",
    "d57fc76161b8dee498e10b09416f9f44f297b4831674f0c9c3e6a4c8f08629a7",
    "84f47a7267691a260f715523eec39c701fe5d985fc1abca175a33f2792896b57",
    "7c83ab9d7152b0bf030ffc4940794cf53c58db47c167dc74ea07e79a611bc396",
    "e11aa3742785b2669f1311d89726fbf85bb395a464c048584e13198d16271e0f",
]
names = [
    "allow-core", "allow-policy", "allow-inventory", "allow-files",
    "allow-rust", "allow-match", "allow-report", "allow-policy-legacy",
    "allow-diff", "cargo-allow",
]
rows = [
    {
        "logical_id": name,
        "name": name,
        "version": "0.2.0",
        "release_order": 10 * (index + 1),
        "state": "published_verified",
        "local_checksum": "sha256:" + digest,
        "registry_checksum": "sha256:" + digest,
    }
    for index, (name, digest) in enumerate(zip(names, digests))
]
receipt = {
    "schema_id": "cargo-allow.topology-publish-receipt.v1",
    "schema_version": 1,
    "mode": "cargo-allow",
    "publish": True,
    "authorization": "issue:3760",
    "topology_id": "manifest-contract-fixture",
    "topology_sha256": "b" * 64,
    "cargo_lock_sha256": "c" * 64,
    "commit": "fixture-commit",
    "tree": "fixture-tree",
    "rows": rows,
    "complete": True,
    "incident_state": "none",
    "first_irreversible_row": 100,
}
pathlib.Path(sys.argv[1]).write_text(
    json.dumps(receipt, indent=2) + "\n", encoding="utf-8"
)
PY

manifest="${work}/release-manifest-v2.json"
VERSION=0.2.0 REPOSITORY=EffortlessMetrics/cargo-allow TAG=v0.2.0 \
  COMMIT=fixture-commit TREE=fixture-tree AUTH_SOURCE=crates_io_api_token \
  MSRV=1.95 PLATFORMS=linux WORKFLOW_RUN_ID=33000000001 \
  TOPOLOGY_RECEIPT="${receipt}" OUTPUT="${manifest}" \
  bash scripts/generate-release-manifest.sh >"${work}/generate.log" 2>&1

python3 - "${manifest}" <<'PY'
import json
import sys

manifest = json.loads(open(sys.argv[1], encoding="utf-8").read())
payload = manifest["payload"]

# The typed ReleaseManifestV2 envelope is embedded.
assert payload["schema_id"] == "cargo-allow.release-manifest.v2", payload["schema_id"]
assert payload["schema_version"] == 2, payload["schema_version"]
assert payload["operation"] == "cargo_allow_release", payload["operation"]
assert payload["publication_posture"] == "published", payload["publication_posture"]
assert payload["authentication"] == "crates_io_api_token", payload["authentication"]

# Every row carries the selected identity and the equality-proven digest.
rows = payload["package_rows"]
assert len(rows) == 10, len(rows)
for row in rows:
    assert row["package_version"] == "0.2.0", row
    assert row["crate_digest"] == row["registry_checksum"], row
assert [row["release_order"] for row in rows] == [10 * i for i in range(1, 11)]

# Consumed evidence is retained per receipt with its producer identity.
evidence = payload["consumed_evidence"]
assert evidence, "consumed_evidence must be present"
required = {"schema_id", "path", "sha256", "producer", "result_class"}
for entry in evidence:
    assert set(entry) == required, entry
    assert entry["producer"], entry
    assert entry["result_class"], entry
    assert entry["sha256"].startswith("sha256:") and len(entry["sha256"]) == 71, entry
assert any(
    entry["schema_id"] == "cargo-allow.topology-publish-receipt.v1"
    and entry["producer"] == "scripts/release-topology-publisher.py"
    and entry["result_class"] == "complete"
    for entry in evidence
), evidence
PY

# A row version that is a valid typed release identity but is not the
# selected release identity must be refused, naming the refusal.
mismatch="${work}/version-mismatch.receipt.json"
cp "${receipt}" "${mismatch}"
python3 - "${mismatch}" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
receipt = json.loads(path.read_text(encoding="utf-8"))
receipt["rows"][-1]["version"] = "0.1.11"
path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
PY
expect_failure row-version-not-the-selected-identity \
  env VERSION=0.2.0 REPOSITORY=EffortlessMetrics/cargo-allow TAG=v0.2.0 \
      COMMIT=fixture-commit TREE=fixture-tree AUTH_SOURCE=crates_io_api_token \
      MSRV=1.95 TOPOLOGY_RECEIPT="${mismatch}" \
      OUTPUT="${work}/rejected-manifest.json" \
      bash scripts/generate-release-manifest.sh
grep -q "not the selected release identity" "${work}/err.log" \
  || {
    printf 'refusal did not name the selected release identity\n' >&2
    cat "${work}/err.log" >&2
    exit 1
  }
[[ ! -e "${work}/rejected-manifest.json" ]] \
  || { printf 'a refused run must not write the manifest\n' >&2; exit 1; }

printf 'ok generate-release-manifest contract: typed envelope embedded, non-selected row version refused\n'
