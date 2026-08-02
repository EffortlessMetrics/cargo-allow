#!/usr/bin/env bash
# Cheap characterization for the operator-latency harness contract (#2468).
# The hosted operator-latency job supplies the actual binary execution proof.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

bash -n scripts/perf-budget-smoke.sh
python3 - <<'PY'
import json
from pathlib import Path

script = Path("scripts/perf-budget-smoke.sh").read_text(encoding="utf-8")
schema = json.loads(Path("docs/schemas/operator-latency.schema.json").read_text(encoding="utf-8"))

assert schema["$id"].endswith("operator-latency.v1.schema.json")
assert schema["properties"]["schema_id"]["const"] == "cargo-allow.operator-latency.v1"
assert "samples" in schema["required"]
for marker in (
    "HARD_CEILING_MS",
    "semantic_artifact",
    "operator-latency.receipt.json",
    'write_receipt "pass" ""',
):
    assert marker in script, marker
PY

printf 'ok operator-latency harness characterization\n'
