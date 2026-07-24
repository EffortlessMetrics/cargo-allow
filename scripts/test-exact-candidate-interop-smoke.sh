#!/usr/bin/env bash
# Characterization checks for scripts/exact-candidate-interop-smoke.sh.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

command -v bash >/dev/null 2>&1 || {
  printf 'bash is required\n' >&2
  exit 1
}

[[ -f scripts/exact-candidate-interop-smoke.sh ]] || {
  printf 'missing scripts/exact-candidate-interop-smoke.sh\n' >&2
  exit 1
}

python3 - "${ROOT}/docs/dogfood/receipts/exact-candidate-interop-pass.example.json" <<'PY'
import json, sys
from pathlib import Path

receipt = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
assert receipt.get("schema_id") == "cargo-allow.exact-candidate-interop.v1"
assert receipt.get("result") == "Passed"
journeys = receipt.get("journeys") or []
assert len(journeys) == 5
assert [entry["id"] for entry in journeys] == ["A", "B", "C", "D", "E"]
print("ok exact-candidate-interop characterization")
PY
