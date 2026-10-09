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
import os
import subprocess
import sys
import tempfile
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
# Execute the production bootstrap, stopping before artifact measurements.
# The marker executable and cwd are independent controls; no Cargo build or
# latency observation is supplied by this fixture.
bootstrap, boundary, _ = script.partition("\nPERF_BINARY_REL=")
assert boundary, "missing production bootstrap boundary"
with tempfile.TemporaryDirectory(prefix="perf-binary-path-") as temporary:
    root = Path(temporary)
    (root / "scripts").mkdir()
    clone = root / "fixture clone"
    clone.mkdir()
    probe = root / "scripts/perf-budget-smoke.sh"
    probe.write_text(
        bootstrap + '\ncd "$PROBE_CWD"\n"$binary"\n',
        encoding="utf-8",
    )
    marker = "selected-perf-binary"
    binaries = (
        "target/release/cargo-allow",
        "target/bin with spaces/cargo-allow",
        "target/windows/cargo-allow.exe",
    )
    for relative in binaries:
        executable = root / relative
        executable.parent.mkdir(parents=True, exist_ok=True)
        executable.write_text(
            "#!/usr/bin/env bash\nprintf '%s\\n' 'selected-perf-binary'\n",
            encoding="utf-8",
        )
        executable.chmod(0o755)

    # A hostile CDPATH must not change which relative directory is selected.
    decoy = root / "decoy"
    decoy_binary = decoy / binaries[0]
    decoy_binary.parent.mkdir(parents=True)
    decoy_binary.write_text(
        "#!/usr/bin/env bash\nprintf '%s\\n' 'wrong-perf-binary'\n",
        encoding="utf-8",
    )
    decoy_binary.chmod(0o755)

    def run_probe(override):
        environment = os.environ.copy()
        environment.update({
            "CARGO_ALLOW_BIN": override,
            "OUTPUT_DIR": str(root / "receipt"),
            "PROFILE": "debug",
            "PROBE_CWD": str(clone),
            "PYTHON3": sys.executable,
            "CDPATH": str(decoy),
        })
        return subprocess.run(
            ["bash", str(probe)], cwd=clone, env=environment,
            capture_output=True, text=True, timeout=20,
        )

    cases = (
        "target/release/cargo-allow",
        "./target/release/cargo-allow",
        "target/bin with spaces/cargo-allow",
        str(root / "target/release/cargo-allow"),
        "target/windows/cargo-allow",
        "target/windows/cargo-allow.exe",
    )
    for override in cases:
        for _ in range(2):
            result = run_probe(override)
            assert result.returncode == 0, (
                override, result.returncode, result.stdout, result.stderr,
            )
            assert result.stdout.strip() == marker, (override, result.stdout)
            assert not list((root / "target").glob("cargo-allow-operator-latency.*"))
        print(f"ok binary cwd control: {override}")

    missing = run_probe("target/absent/cargo-allow")
    assert missing.returncode == 1, (missing.stdout, missing.stderr)
    assert "cargo-allow binary is not executable: target/absent/cargo-allow" in missing.stderr
    assert marker not in missing.stdout
    assert not list((root / "target").glob("cargo-allow-operator-latency.*"))
    print("ok missing binary refuses before cwd probe")

PY

printf 'ok operator-latency harness characterization\n'
