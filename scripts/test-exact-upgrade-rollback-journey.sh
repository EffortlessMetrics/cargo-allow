#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/cargo-allow-upgrade-rollback-test.XXXXXX")"
trap 'rm -rf "${tmp}"' EXIT
receipt="${tmp}/receipt.json"
python3 - "${ROOT}/docs/dogfood/fixtures/release/upgrade-rollback-repository.toml" "${receipt}" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

fixture, output = map(Path, sys.argv[1:])
digest = hashlib.sha256(fixture.read_bytes()).hexdigest()
def leg(name):
    return [{"leg": name, "id": f"{name}:{step}"} for step in ("doctor", "audit", "check")]
receipt = {
    "schema_version": 1, "schema_id": "cargo-allow.exact-upgrade-rollback-journey.v1", "tool": "cargo-allow", "result": "Passed", "claim_boundary": ["test"],
    "from": {"version": "cargo-allow 0.1.11", "binary_sha256": "a" * 64},
    "candidate": {"version": "cargo-allow 0.2.0", "binary_sha256": "b" * 64},
    "rollback": {"version": "cargo-allow 0.1.11", "binary_sha256": "a" * 64},
    "repository": {"fixture_sha256": digest, "unrelated_file_preserved": True}, "steps": leg("from") + leg("candidate") + leg("rollback"),
    "negative_controls": [{"id": key, "passed": True} for key in ("old_binary_exact_version", "candidate_binary_exact_version", "checkout_binary_not_used", "unrelated_file_survives_rollback")], "limitations": [],
}
output.write_text(json.dumps(receipt), encoding="utf-8")
PY
validator="${ROOT}/scripts/validate-upgrade-rollback-journey.py"
schema="${ROOT}/docs/dogfood/fixtures/release/exact-upgrade-rollback-journey.v1.schema.json"
fixture="${ROOT}/docs/dogfood/fixtures/release/upgrade-rollback-repository.toml"
python3 "${validator}" --receipt "${receipt}" --schema "${schema}" --fixture "${fixture}"
python3 - "${receipt}" <<'PY'
import json
import sys
path = sys.argv[1]
value = json.load(open(path, encoding="utf-8"))
value["rollback"]["binary_sha256"] = "c" * 64
json.dump(value, open(path, "w", encoding="utf-8"))
PY
if python3 "${validator}" --receipt "${receipt}" --schema "${schema}" --fixture "${fixture}"; then
  echo "forged rollback identity unexpectedly validated" >&2
  exit 1
fi
printf 'ok exact upgrade/rollback receipt and negative characterization\n'
