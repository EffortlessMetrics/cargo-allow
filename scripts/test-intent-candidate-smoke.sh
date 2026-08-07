#!/usr/bin/env bash
# Characterization checks for scripts/intent-candidate-smoke.sh.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

work_dir="${ROOT}/target/intent-candidate-smoke-test"
rm -rf "${work_dir}"

WORK_DIR="${work_dir}" bash scripts/intent-candidate-smoke.sh

receipt="${work_dir}/intent-candidate-smoke.receipt.json"
[[ -f "${receipt}" ]] || {
  printf 'missing receipt %s\n' "${receipt}" >&2
  exit 1
}

python3 - "${receipt}" <<'PY'
import json
import sys
from pathlib import Path

receipt = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
assert receipt.get("schema_id") == "cargo-allow.intent-candidate-smoke.v1"
assert receipt.get("result") == "Passed"
order = receipt.get("package_set", {}).get("order", [])
assert order == [
    "allow-core",
    "allow-inventory",
    "effortless-repo-protocol",
    "effortless-repo-snapshot",
    "intent-protocol",
    "intent-engine",
    "cargo-intent",
]
boundary = receipt.get("claim_boundary", [])
for required in (
    "no_proof_or_test_invocation",
    "no_workspace_target_debug_binary",
    "source_checkout_denied_during_decisive_install",
):
    assert required in boundary, f"missing claim_boundary {required}"
print("ok intent-candidate-smoke characterization")
PY
