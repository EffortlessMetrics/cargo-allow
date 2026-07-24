#!/usr/bin/env bash
# Characterization checks for scripts/three-product-dogfood-smoke.sh.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

python3 - "${ROOT}/docs/dogfood/receipts/three-product-dogfood-pass.example.json" <<'PY'
import json, sys
from pathlib import Path

receipt = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
assert receipt.get("schema_id") == "cargo-allow.three-product-dogfood.v1"
assert receipt.get("result") == "Passed"
assert "no_physical_repository_extraction" in receipt.get("claim_boundary", [])
print("ok three-product-dogfood characterization")
PY
