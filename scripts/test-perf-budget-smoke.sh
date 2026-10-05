#!/usr/bin/env bash
# Cheap characterization for the operator-latency harness contract (#2468,
# #4366). The hosted operator-latency job supplies the actual binary
# execution proof.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

py="${PYTHON3:-python3}"
if ! command -v "${py}" >/dev/null 2>&1; then
  if command -v python >/dev/null 2>&1 && python -c 'import sys; sys.exit(0 if sys.version_info[0] >= 3 else 1)' >/dev/null 2>&1; then
    py=python
  else
    echo "a Python 3 interpreter is required" >&2
    exit 1
  fi
fi

bash -n scripts/perf-budget-smoke.sh
"${py}" - <<'PY'
import json
from pathlib import Path

script = Path("scripts/perf-budget-smoke.sh").read_text(encoding="utf-8")
v1 = json.loads(Path("docs/schemas/operator-latency.schema.json").read_text(encoding="utf-8"))
v2 = json.loads(Path("docs/schemas/operator-latency.v2.schema.json").read_text(encoding="utf-8"))
schema = json.loads(Path("docs/schemas/operator-latency.v3.schema.json").read_text(encoding="utf-8"))

assert v1["$id"].endswith("operator-latency.v1.schema.json")
assert v1["properties"]["schema_id"]["const"] == "cargo-allow.operator-latency.v1"
assert "cache_mode_samples" not in v1["$defs"]["sample_policy"]["properties"]
assert v2["$id"].endswith("operator-latency.v2.schema.json")
assert v2["properties"]["schema_version"]["const"] == 2
assert v2["properties"]["schema_id"]["const"] == "cargo-allow.operator-latency.v2"
assert "cache_mode_samples" in v2["$defs"]["sample_policy"]["required"]
assert "cache_mode" in v2["$defs"]["sample"]["required"]
assert set(v2["$defs"]["sample"]["properties"]["cache_mode"]["enum"]) == {"on", "off", "not_applicable"}
assert "agent_loop" not in v2["properties"]
assert schema["$id"].endswith("operator-latency.v3.schema.json")
assert schema["properties"]["schema_version"]["const"] == 3
assert schema["properties"]["schema_id"]["const"] == "cargo-allow.operator-latency.v3"
assert "cache_mode_samples" in schema["$defs"]["sample_policy"]["required"]
assert "cache_mode" in schema["$defs"]["sample"]["required"]
assert "samples" in schema["required"]
# v3 is an additive bump over v2: agent_loop phase, optional per-sample
# payload_bytes, optional agent_loop sample count, and an optional
# agent_loop budget object.
assert set(schema["$defs"]["sample"]["properties"]["phase"]["enum"]) == {
    "cold", "warm", "targeted", "agent_loop",
}
assert "agent_loop_samples" in schema["$defs"]["sample_policy"]["properties"]
assert "agent_loop_samples" not in schema["$defs"]["sample_policy"]["required"]
assert schema["$defs"]["sample"]["properties"]["payload_bytes"]["type"] == ["integer", "null"]
assert "payload_bytes" not in schema["$defs"]["sample"]["required"]
assert "agent_loop" in schema["properties"]
assert "agent_loop" not in schema["required"]
agent_loop = schema["$defs"]["agent_loop"]
assert set(agent_loop["required"]) == {"composite", "hooks_overhead"}
assert set(agent_loop["properties"]["composite"]["required"]) == {
    "name", "steps", "total_elapsed_ms",
}
assert set(agent_loop["properties"]["hooks_overhead"]["required"]) == {
    "wrapped_sample", "bare_sample", "wrapped_elapsed_ms", "bare_elapsed_ms", "overhead_ms",
}
for marker in (
    "HARD_CEILING_MS",
    "semantic_artifact",
    "cache_cold_on",
    "cache_warm_on",
    "cache_disabled_off",
    "--persistent-cache",
    "normalize_json",
    'output_dir="${output_dir%/}"',
    'mkdir -p "${ROOT}/target"',
    '[[ "${binary,,}" == *.exe ]]',
    "operator-latency.receipt.json",
    'write_receipt "pass" ""',
    # #4366 agentic-surface extension.
    'schema_id": "cargo-allow.operator-latency.v3"',
    "payload_bytes",
    "agent_loop_worklist",
    "agent_loop_why_plan",
    "agent_loop_add",
    "agent_loop_check",
    "cargo-allow.add-finding-plan.v1",
    "add-plan-application",
    "new_unreceipted_finding",
    "hooks_wrapped_check",
    "hooks_bare_check",
    "--mode explicit-tool-under-test",
    "executable_digest",
    "worklist_summary",
    "check_summary",
    "--command-summary-output",
    "cargo-allow.core-command-summary.v1",
    "git clone --shared",
    "agent_loop_probe.rs",
    "payload_ceilings",
    "524288",
    "8388608",
    "16384",
    "12288",
    "4096",
    "sample_field",
):
    assert marker in script, marker
PY

printf 'ok operator-latency harness characterization\n'
