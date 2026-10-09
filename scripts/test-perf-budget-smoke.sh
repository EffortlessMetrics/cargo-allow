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

    # Kernel lookup of symlink/.. must keep selecting the original file,
    # even when a different executable exists at the logical parent.
    (root / "alternate/inner").mkdir(parents=True)
    selected = root / "alternate/selected/cargo-allow"
    selected.parent.mkdir()
    selected.write_text(
        "#!/usr/bin/env bash\nprintf '%s\\n' 'selected-symlink-parent'\n",
        encoding="utf-8",
    )
    selected.chmod(0o755)
    logical = root / "target/selected/cargo-allow"
    logical.parent.mkdir()
    logical.write_text(
        "#!/usr/bin/env bash\nprintf '%s\\n' 'wrong-logical-parent'\n",
        encoding="utf-8",
    )
    logical.chmod(0o755)
    (root / "target/link-to-dir").symlink_to(root / "alternate/inner", target_is_directory=True)
    symlink_override = "target/link-to-dir/../selected/cargo-allow"
    assert (root / symlink_override).samefile(selected)
    assert not (root / symlink_override).samefile(logical)

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
        ("target/release/cargo-allow", marker),
        ("./target/release/cargo-allow", marker),
        ("target/bin with spaces/cargo-allow", marker),
        (str(root / "target/release/cargo-allow"), marker),
        ("target/windows/cargo-allow", marker),
        ("target/windows/cargo-allow.exe", marker),
        (symlink_override, "selected-symlink-parent"),
        (str(root / symlink_override), "selected-symlink-parent"),
    )
    for override, expected_marker in cases:
        for _ in range(2):
            result = run_probe(override)
            assert result.returncode == 0, (
                override, result.returncode, result.stdout, result.stderr,
            )
            assert result.stdout.strip() == expected_marker, (override, result.stdout)
            assert not list((root / "target").glob("cargo-allow-operator-latency.*"))
        print(f"ok binary cwd control: {override}")

    missing = run_probe("target/absent/cargo-allow")
    assert missing.returncode == 1, (missing.stdout, missing.stderr)
    assert "cargo-allow binary is not executable: target/absent/cargo-allow" in missing.stderr
    assert marker not in missing.stdout
    assert not list((root / "target").glob("cargo-allow-operator-latency.*"))
    print("ok missing binary refuses before cwd probe")

PY

# The receipt consumer belongs to this same operator-harness contract.
"${py}" - <<'PY_RECEIPT'
"""Exercise the actual inline CI receipt consumer with synthetic v3 receipts.

These fixtures test duration admission, not latency or artifact integrity.
No second runtime validator or third-party dependency is introduced.
"""
import json
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

ROOT = Path.cwd()
SCHEMA_PATH = "docs/schemas/operator-latency.v3.schema.json"
RECEIPT_PATH = "target/perf-budget/operator-latency.receipt.json"


def consumer_source():
    workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
    marker = "\n      - name: Validate operator latency receipt contract\n"
    assert workflow.count(marker) == 1, "missing or ambiguous CI receipt consumer"
    block = workflow.split(marker, 1)[1].split("\n      - ", 1)[0]
    _, start, body = block.partition("          python3 - <<'PY'\n")
    assert start, "missing CI consumer Python boundary"
    source, end, _ = body.partition("\n          PY")
    assert end, "missing CI consumer heredoc end"
    return textwrap.dedent(source) + "\n"


def receipt_fixture():
    names = [
        ("cache_cold_on", "cold", "on"),
        ("cache_warm_on", "warm", "on"),
        ("cache_disabled_off", "targeted", "off"),
        ("first_audit", "cold", "not_applicable"),
        ("warm_check", "warm", "not_applicable"),
        ("why_fast_path", "targeted", "not_applicable"),
        ("worklist", "targeted", "not_applicable"),
        ("diff_base", "targeted", "not_applicable"),
        ("warm_audit", "warm", "not_applicable"),
        ("worklist_summary", "targeted", "not_applicable"),
        ("check_summary", "targeted", "not_applicable"),
        ("agent_loop_worklist", "agent_loop", "not_applicable"),
        ("agent_loop_why_plan", "agent_loop", "not_applicable"),
        ("agent_loop_add", "agent_loop", "not_applicable"),
        ("agent_loop_check", "agent_loop", "not_applicable"),
        ("hooks_wrapped_check", "agent_loop", "not_applicable"),
        ("hooks_bare_check", "agent_loop", "not_applicable"),
    ]
    samples = [
        {
            "name": name, "phase": phase, "cache_mode": mode,
            "argv": ["cargo-allow", "check"], "elapsed_ms": 1,
            "status": "passed", "payload_bytes": 0,
            "artifact": {"path": f"{name}.json", "sha256": "0" * 64},
            "semantic_artifact": {"path": f"{name}.semantic.json", "sha256": "0" * 64},
        }
        for name, phase, mode in names
    ]
    return {
        "schema_version": 3, "schema_id": "cargo-allow.operator-latency.v3",
        "tool": "cargo-allow", "command": "operator-latency", "result": "pass",
        "binary": {"path": "target/release/cargo-allow", "sha256": "0" * 64, "profile": "release"},
        "host": {"os": "Linux", "release": "fixture", "machine": "x86_64", "rustc": "fixture"},
        "repository": {"commit": "fixture", "tracked_files": 0, "policy_entries": 0},
        "sample_policy": {
            "cold_process_samples": 2, "warm_process_samples": 3,
            "targeted_samples": 6, "agent_loop_samples": 6,
            "cache_mode_samples": {"on": 2, "off": 1, "not_applicable": 14},
        },
        "budget": {
            "name": "operator_loop_hard_ceiling", "kind": "catastrophic_regression",
            "ceiling_ms": 60000, "disposition": "passed",
        },
        "agent_loop": {
            "composite": {
                "name": "agent_loop", "added_allow_id": "fixture-allow",
                "steps": [
                    {"sample": name, "elapsed_ms": 1, "payload_bytes": 0}
                    for name in (
                        "agent_loop_worklist", "agent_loop_why_plan",
                        "agent_loop_add", "agent_loop_check",
                    )
                ],
                "total_elapsed_ms": 4,
            },
            "hooks_overhead": {
                "wrapped_sample": "hooks_wrapped_check", "bare_sample": "hooks_bare_check",
                "wrapped_elapsed_ms": 1, "bare_elapsed_ms": 1, "overhead_ms": 0,
            },
        },
        "samples": samples,
        "claim_boundary": ["synthetic receipt duration-admission fixture"],
        "limitations": ["not a latency or artifact-integrity observation"],
    }


def duration_paths():
    return (
        [("samples", i, "elapsed_ms") for i in range(17)]
        + [("agent_loop", "composite", "steps", i, "elapsed_ms") for i in range(4)]
        + [
            ("agent_loop", "composite", "total_elapsed_ms"),
            ("agent_loop", "hooks_overhead", "wrapped_elapsed_ms"),
            ("agent_loop", "hooks_overhead", "bare_elapsed_ms"),
        ]
    )


def set_duration(receipt, path, value):
    node = receipt
    for key in path[:-1]:
        node = node[key]
    node[path[-1]] = value
    # Keep unrelated arithmetic consistent so it cannot mask an admission bug.
    composite = receipt["agent_loop"]["composite"]
    if path[:3] == ("agent_loop", "composite", "steps") and isinstance(value, (int, float)):
        composite["total_elapsed_ms"] = sum(step["elapsed_ms"] for step in composite["steps"])
    elif path == ("agent_loop", "composite", "total_elapsed_ms") and isinstance(value, (int, float)):
        for step in composite["steps"]:
            step["elapsed_ms"] = 0
    elif path[:2] == ("agent_loop", "hooks_overhead") and isinstance(value, (int, float)):
        hooks = receipt["agent_loop"]["hooks_overhead"]
        hooks["overhead_ms"] = hooks["wrapped_elapsed_ms"] - hooks["bare_elapsed_ms"]


def duration_diagnostic(path):
    # Literal consumer diagnostics distinguish duration admission from arithmetic.
    if path[0] == "samples":
        return "samples.elapsed_ms must be nonnegative integers within the ceiling"
    if path[:3] == ("agent_loop", "composite", "steps"):
        return "agent_loop.composite.steps.elapsed_ms must be nonnegative integers"
    if path == ("agent_loop", "composite", "total_elapsed_ms"):
        return "agent_loop.composite.total_elapsed_ms must be a nonnegative integer"
    if path[:2] == ("agent_loop", "hooks_overhead"):
        return "agent_loop.hooks_overhead elapsed values must be nonnegative integers"
    raise AssertionError(f"uncovered duration path: {path}")


class ReceiptDurationControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = consumer_source()
        cls.schema_text = (ROOT / SCHEMA_PATH).read_text(encoding="utf-8")

    def invoke(self, receipt, *, write_receipt=True):
        with tempfile.TemporaryDirectory(prefix="operator-duration-") as directory:
            root = Path(directory)
            schema_path = root / SCHEMA_PATH
            schema_path.parent.mkdir(parents=True)
            schema_path.write_text(self.schema_text, encoding="utf-8")
            if write_receipt:
                receipt_path = root / RECEIPT_PATH
                receipt_path.parent.mkdir(parents=True)
                receipt_path.write_text(json.dumps(receipt), encoding="utf-8")
            return subprocess.run(
                [sys.executable, "-"], input=self.source, cwd=root,
                capture_output=True, text=True, timeout=20,
            )

    def assert_accepted(self, receipt):
        result = self.invoke(receipt)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), "validated 17 operator-latency samples")

    def assert_rejected(self, receipt, label, *, diagnostic=None):
        result = self.invoke(receipt)
        self.assertEqual(
            result.returncode, 1,
            f"invalid receipt accepted: {label}; stdout={result.stdout!r}; stderr={result.stderr!r}",
        )
        self.assertNotIn("validated 17 operator-latency samples", result.stdout)
        self.assertTrue(result.stderr, f"missing rejection diagnostic: {label}")
        if diagnostic is not None:
            self.assertIn(diagnostic, result.stderr)

    def test_schema_duration_contract(self):
        schema = json.loads(self.schema_text)
        sample = schema["$defs"]["sample"]
        self.assertEqual(sample["properties"]["elapsed_ms"]["minimum"], 0)
        self.assertEqual(sample["allOf"][0]["then"]["properties"]["elapsed_ms"], {"type": "integer", "minimum": 0})
        composite = schema["$defs"]["agent_loop"]["properties"]["composite"]["properties"]
        self.assertEqual(composite["steps"]["items"]["properties"]["elapsed_ms"], {"type": "integer", "minimum": 0})
        self.assertEqual(composite["total_elapsed_ms"], {"type": "integer", "minimum": 0})
        hooks = schema["$defs"]["agent_loop"]["properties"]["hooks_overhead"]["properties"]
        for field in ("wrapped_elapsed_ms", "bare_elapsed_ms"):
            self.assertEqual(hooks[field], {"type": "integer", "minimum": 0})
        self.assertEqual(hooks["overhead_ms"], {"type": "integer"})

    def test_valid_duration_boundaries(self):
        for path in duration_paths():
            for value in (0, 60000):
                with self.subTest(path=path, value=value):
                    receipt = receipt_fixture()
                    set_duration(receipt, path, value)
                    self.assert_accepted(receipt)

    def test_negative_durations_refuse(self):
        for path in duration_paths():
            with self.subTest(path=path):
                receipt = receipt_fixture()
                set_duration(receipt, path, -1)
                self.assert_rejected(receipt, path, diagnostic=duration_diagnostic(path))

    def test_duration_types_refuse(self):
        for path in duration_paths():
            for value in (True, 1.5, None, "1"):
                with self.subTest(path=path, value=value):
                    receipt = receipt_fixture()
                    set_duration(receipt, path, value)
                    self.assert_rejected(receipt, (path, value), diagnostic=duration_diagnostic(path))

    def test_signed_hook_overhead_is_valid(self):
        for wrapped, bare in ((0, 1), (1, 0)):
            receipt = receipt_fixture()
            hooks = receipt["agent_loop"]["hooks_overhead"]
            hooks.update(
                wrapped_elapsed_ms=wrapped, bare_elapsed_ms=bare,
                overhead_ms=wrapped - bare,
            )
            for _ in range(2):
                self.assert_accepted(receipt)

    def test_existing_refusals(self):
        invalid = []
        for field, value in (("schema_id", "wrong"), ("schema_version", 4), ("result", "failed")):
            receipt = receipt_fixture()
            receipt[field] = value
            invalid.append((field, receipt))
        receipt = receipt_fixture()
        receipt["binary"]["profile"] = "debug"
        invalid.append(("profile", receipt))
        receipt = receipt_fixture()
        receipt["samples"].pop()
        invalid.append(("sample_count", receipt))
        receipt = receipt_fixture()
        receipt["samples"][0]["elapsed_ms"] = 60001
        invalid.append(("sample_ceiling", receipt))
        receipt = receipt_fixture()
        receipt["agent_loop"]["composite"]["total_elapsed_ms"] = 0
        invalid.append(("composite_arithmetic", receipt))
        receipt = receipt_fixture()
        receipt["agent_loop"]["hooks_overhead"]["overhead_ms"] = 99
        invalid.append(("hooks_arithmetic", receipt))
        for label, receipt in invalid:
            with self.subTest(label=label):
                self.assert_rejected(receipt, label)

    def test_missing_receipt_diagnostic(self):
        result = self.invoke(receipt_fixture(), write_receipt=False)
        self.assertEqual(result.returncode, 1)
        self.assertIn("missing receipt:", result.stderr)
        self.assertIn("see the harness step output", result.stderr)


if __name__ == "__main__":
    unittest.main(verbosity=2)
PY_RECEIPT

printf 'ok operator-latency harness characterization\n'
