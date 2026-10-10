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
v3 = json.loads(Path("docs/schemas/operator-latency.v3.schema.json").read_text(encoding="utf-8"))
schema = json.loads(Path("docs/schemas/operator-latency.v4.schema.json").read_text(encoding="utf-8"))

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
# v3 remains a historical compatibility schema after the v4 bump.
assert v3["$id"].endswith("operator-latency.v3.schema.json")
assert v3["properties"]["schema_version"]["const"] == 3
assert v3["properties"]["schema_id"]["const"] == "cargo-allow.operator-latency.v3"
assert "semantic_payload_bytes" not in v3["$defs"]["sample"]["properties"]
assert schema["$id"].endswith("operator-latency.v4.schema.json")
assert schema["properties"]["schema_version"]["const"] == 4
assert schema["properties"]["schema_id"]["const"] == "cargo-allow.operator-latency.v4"
assert "cache_mode_samples" in schema["$defs"]["sample_policy"]["required"]
assert "cache_mode" in schema["$defs"]["sample"]["required"]
assert "samples" in schema["required"]
# v4 is an additive bump over v3: per-sample and per-composite-step
# semantic_payload_bytes recording the semantic artifact's byte count.
assert set(schema["$defs"]["sample"]["properties"]["phase"]["enum"]) == {
    "cold", "warm", "targeted", "agent_loop",
}
assert "agent_loop_samples" in schema["$defs"]["sample_policy"]["properties"]
assert "agent_loop_samples" not in schema["$defs"]["sample_policy"]["required"]
assert schema["$defs"]["sample"]["properties"]["payload_bytes"]["type"] == ["integer", "null"]
assert schema["$defs"]["sample"]["properties"]["semantic_payload_bytes"]["type"] == ["integer", "null"]
assert schema["$defs"]["sample"]["properties"]["semantic_payload_bytes"]["minimum"] == 0
assert "payload_bytes" not in schema["$defs"]["sample"]["required"]
assert "semantic_payload_bytes" not in schema["$defs"]["sample"]["required"]
composite_steps = (
    schema["$defs"]["agent_loop"]["properties"]["composite"]["properties"]["steps"]["items"]
)
assert composite_steps["properties"]["semantic_payload_bytes"]["type"] == ["integer", "null"]
assert composite_steps["properties"]["semantic_payload_bytes"]["minimum"] == 0
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
    'schema_id": "cargo-allow.operator-latency.v4"',
    "payload_bytes",
    "semantic_payload_bytes",
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

# Binary selection is admission, before identity capture or sample dispatch.
# Native Unix probes execute independent marker files. Windows-family probes
# simulate only uname-based selection policy; they do not execute Windows code.
"${py}" - <<'PY_BINARY_SELECTION'
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

script = Path("scripts/perf-budget-smoke.sh").read_text(encoding="utf-8")
bootstrap, boundary, _ = script.partition("\nPERF_BINARY_REL=")
assert boundary, "missing production bootstrap boundary"


class BinarySelectionControls(unittest.TestCase):
    def invoke(self, entries, *, override="target/selected/cargo-allow",
               platform_name="Linux", execute=True):
        with tempfile.TemporaryDirectory(prefix="perf-binary-selection-") as temporary:
            root = Path(temporary)
            (root / "scripts").mkdir()
            clone = root / "fixture clone"
            clone.mkdir()
            probe = root / "scripts/perf-budget-smoke.sh"
            probe.write_text(
                bootstrap
                + '\nprintf "dispatch-ready:%s\\n" "$binary"\n'
                + 'if [[ "$PROBE_EXECUTE" == yes ]]; then "$binary"; fi\n',
                encoding="utf-8",
            )
            for relative, kind, marker in entries:
                candidate = root / relative
                candidate.parent.mkdir(parents=True, exist_ok=True)
                if kind == "directory":
                    candidate.mkdir()
                else:
                    candidate.write_text(
                        "#!/usr/bin/env bash\nprintf '%s\\n' '" + marker + "'\n",
                        encoding="utf-8",
                    )
                    candidate.chmod(0o755 if kind == "executable" else 0o644)

            shims = root / "platform-shims"
            shims.mkdir()
            uname = shims / "uname"
            uname.write_text(
                "#!/bin/sh\nprintf '%s\\n' '" + platform_name + "'\n",
                encoding="utf-8",
            )
            uname.chmod(0o755)
            environment = os.environ.copy()
            environment.update({
                "PATH": str(shims) + os.pathsep + environment["PATH"],
                "CARGO_ALLOW_BIN": override,
                "OUTPUT_DIR": str(root / "receipt"),
                "PROFILE": "debug",
                "PROBE_CWD": str(clone),
                "PROBE_EXECUTE": "yes" if execute else "no",
                "PYTHON3": sys.executable,
                "CDPATH": "",
            })
            result = subprocess.run(
                ["bash", str(probe)], cwd=clone, env=environment,
                capture_output=True, text=True, timeout=20,
            )
            self.assertFalse(
                list((root / "target").glob("cargo-allow-operator-latency.*")),
                "binary selection left a disposable run directory",
            )
            return result, root

    def assert_selected(self, entries, expected, marker, **options):
        result, root = self.invoke(entries, **options)
        self.assertEqual(result.returncode, 0, result.stderr)
        lines = [f"dispatch-ready:{root / expected}"]
        if options.get("execute", True):
            lines.append(marker)
        self.assertEqual(result.stdout.splitlines(), lines, result.stdout)

    def test_unix_literal_and_fallback_selection(self):
        literal = "target/selected/cargo-allow"
        suffix = literal + ".exe"
        cases = (
            ("both_executable", (
                (literal, "executable", "literal-marker"),
                (suffix, "executable", "suffix-marker"),
            ), literal, "literal-marker", literal),
            ("nonexecutable_sibling", (
                (literal, "executable", "literal-marker"),
                (suffix, "nonexecutable", "unusable-suffix"),
            ), literal, "literal-marker", literal),
            ("directory_sibling", (
                (literal, "executable", "literal-marker"),
                (suffix, "directory", ""),
            ), literal, "literal-marker", literal),
            ("nonexecutable_literal_fallback", (
                (literal, "nonexecutable", "unusable-literal"),
                (suffix, "executable", "suffix-marker"),
            ), suffix, "suffix-marker", literal),
            ("missing_literal_fallback", (
                (suffix, "executable", "suffix-marker"),
            ), suffix, "suffix-marker", literal),
            ("explicit_exe", (
                (suffix, "executable", "suffix-marker"),
            ), suffix, "suffix-marker", suffix),
        )
        for label, entries, expected, marker, override in cases:
            for repetition in range(2):
                with self.subTest(case=label, repetition=repetition):
                    self.assert_selected(entries, expected, marker, override=override)
                    print(f"ok Unix binary selection: {label}, repetition {repetition}")

    def test_unix_unusable_candidates_refuse_before_dispatch(self):
        literal = "target/selected/cargo-allow"
        suffix = literal + ".exe"
        cases = (
            ("nonexecutable_literal", ((literal, "nonexecutable", "unusable"),)),
            ("nonexecutable_suffix", ((suffix, "nonexecutable", "unusable"),)),
            ("both_nonexecutable", (
                (literal, "nonexecutable", "unusable-literal"),
                (suffix, "nonexecutable", "unusable-suffix"),
            )),
            ("directory_literal", ((literal, "directory", ""),)),
            ("directory_suffix", ((suffix, "directory", ""),)),
            ("both_directories", (
                (literal, "directory", ""), (suffix, "directory", ""),
            )),
        )
        for label, entries in cases:
            for repetition in range(2):
                with self.subTest(case=label, repetition=repetition):
                    result, _ = self.invoke(entries, execute=False)
                    self.assertEqual(result.returncode, 1, (result.stdout, result.stderr))
                    self.assertIn(
                        "cargo-allow binary is not executable: " + literal,
                        result.stderr,
                    )
                    self.assertNotIn("dispatch-ready:", result.stdout)
                    self.assertNotIn("unusable", result.stdout)
                    print(f"ok Unix binary refusal: {label}, repetition {repetition}")

    def test_windows_family_literal_exe_policy_without_dispatch(self):
        literal = "target/selected/cargo-allow"
        suffix = literal + ".exe"
        for platform_name in ("MINGW64_NT-fixture", "MSYS_NT-fixture", "CYGWIN_NT-fixture"):
            for explicit in (False, True):
                for repetition in range(2):
                    with self.subTest(platform=platform_name, explicit=explicit,
                                      repetition=repetition):
                        entries = (
                            (literal, "executable", "literal-marker"),
                            (suffix, "nonexecutable", "native-exe-fixture"),
                        )
                        self.assert_selected(
                            entries, suffix, "native-exe-fixture",
                            override=suffix if explicit else literal,
                            platform_name=platform_name, execute=False,
                        )
                        print(f"ok Windows-family selection policy: {platform_name}, "
                              f"explicit {explicit}, repetition {repetition}")


if __name__ == "__main__":
    unittest.main(verbosity=2)
PY_BINARY_SELECTION

# The receipt consumer belongs to this same operator-harness contract.
"${py}" - <<'PY_RECEIPT'
"""Exercise the actual inline CI receipt consumer with synthetic v4 receipts.

These fixtures test duration, byte-count and retained-digest admission, not
latency or real command output. No second runtime validator or third-party dependency
is introduced.
"""
import hashlib
import json
import math
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

ROOT = Path.cwd()
SCHEMA_PATH = "docs/schemas/operator-latency.v4.schema.json"
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


def retained_sizes(index):
    # Artifact and semantic files carry different known UTF-8 byte lengths
    # so each recorded count is an independent control. Sizes depend only on
    # the fixture index: mutating a receipt count cannot move its file.
    return 16 + index, 64 + index


def receipt_fixture():
    names = [
        ("cache_cold_on", "cold", "on"),
        ("cache_warm_on", "warm", "on"),
        ("cache_disabled_off", "targeted", "off"),
        ("first_audit", "cold", "not_applicable"),
        ("warm_check", "warm", "not_applicable"),
        ("full_check_json", "warm", "not_applicable"),
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
    samples = []
    for index, (name, phase, mode) in enumerate(names):
        artifact_size, semantic_size = retained_sizes(index)
        samples.append({
            "name": name, "phase": phase, "cache_mode": mode,
            "argv": ["cargo-allow", "check"], "elapsed_ms": 1,
            "status": "passed", "payload_bytes": artifact_size,
            "semantic_payload_bytes": semantic_size,
            "artifact": {
                "path": f"{name}.json", "sha256": hashlib.sha256(b"a" * artifact_size).hexdigest(),
            },
            "semantic_artifact": {
                "path": f"{name}.semantic.json", "sha256": hashlib.sha256(b"s" * semantic_size).hexdigest(),
            },
        })
        if name == "full_check_json":
            samples[-1]["artifact"]["path"] = "artifacts/full-check.json"
            samples[-1]["semantic_artifact"]["path"] = "artifacts/full-check.receipt.json"
            samples[-1]["argv"] = [
                "check", "--mode", "no-new", "--format", "json", "--receipt",
                "target/perf-budget/artifacts/full-check.receipt.json", "--output",
                "target/perf-budget/artifacts/full-check.json",
            ]
    return {
        "schema_version": 4, "schema_id": "cargo-allow.operator-latency.v4",
        "tool": "cargo-allow", "command": "operator-latency", "result": "pass",
        "binary": {"path": "target/release/cargo-allow", "sha256": "0" * 64, "profile": "release"},
        "host": {"os": "Linux", "release": "fixture", "machine": "x86_64", "rustc": "fixture"},
        "repository": {"commit": "fixture", "tracked_files": 0, "policy_entries": 0},
        "sample_policy": {
            "cold_process_samples": 2, "warm_process_samples": 4,
            "targeted_samples": 6, "agent_loop_samples": 6,
            "cache_mode_samples": {"on": 2, "off": 1, "not_applicable": 15},
        },
        "budget": {
            "name": "operator_loop_hard_ceiling", "kind": "catastrophic_regression",
            "ceiling_ms": 60000, "disposition": "passed",
        },
        "agent_loop": {
            "composite": {
                "name": "agent_loop", "added_allow_id": "fixture-allow",
                "steps": [
                    {
                        "sample": name, "elapsed_ms": 1,
                        "payload_bytes": 16 + index,
                        "semantic_payload_bytes": 64 + index,
                    }
                    for index, name in enumerate((
                        "agent_loop_worklist", "agent_loop_why_plan",
                        "agent_loop_add", "agent_loop_check",
                    ))
                ],
                "total_elapsed_ms": 4,
            },
            "hooks_overhead": {
                "wrapped_sample": "hooks_wrapped_check", "bare_sample": "hooks_bare_check",
                "wrapped_elapsed_ms": 1, "bare_elapsed_ms": 1, "overhead_ms": 0,
            },
        },
        "samples": samples,
        "claim_boundary": ["synthetic receipt admission fixture"],
        "limitations": ["not a latency or real command-output observation"],
    }


def duration_paths():
    return (
        [("samples", i, "elapsed_ms") for i in range(18)]
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
    # Keep unrelated durations admissible and arithmetic valid so they cannot mask this field.
    composite = receipt["agent_loop"]["composite"]
    if path[:3] == ("agent_loop", "composite", "steps") and isinstance(value, (int, float)):
        composite["total_elapsed_ms"] = max(
            0, math.ceil(sum(step["elapsed_ms"] for step in composite["steps"]))
        )
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


class ReceiptAdmissionControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = consumer_source()
        cls.schema_text = (ROOT / SCHEMA_PATH).read_text(encoding="utf-8")

    def invoke(self, receipt, *, write_receipt=True, mutate_retained=None):
        with tempfile.TemporaryDirectory(prefix="operator-duration-") as directory:
            root = Path(directory)
            schema_path = root / SCHEMA_PATH
            schema_path.parent.mkdir(parents=True)
            schema_path.write_text(self.schema_text, encoding="utf-8")
            if write_receipt:
                receipt_path = root / RECEIPT_PATH
                receipt_path.parent.mkdir(parents=True)
                receipt_path.write_text(json.dumps(receipt), encoding="utf-8")
                # Retain one file per recorded digest at exactly the
                # fixture's deterministic byte counts; mutated counts must
                # not be able to pass by moving the files instead.
                for index, sample in enumerate(receipt["samples"]):
                    artifact_size, semantic_size = retained_sizes(index)
                    for key, size, prefix in (
                        ("artifact", artifact_size, b"a"),
                        ("semantic_artifact", semantic_size, b"s"),
                    ):
                        retained = receipt_path.parent / sample[key]["path"]
                        retained.parent.mkdir(parents=True, exist_ok=True)
                        retained.write_bytes(prefix * size)
                if mutate_retained is not None:
                    mutate_retained(receipt_path.parent)
            return subprocess.run(
                [sys.executable, "-"], input=self.source, cwd=root,
                capture_output=True, text=True, timeout=20,
            )

    def assert_accepted(self, receipt):
        result = self.invoke(receipt)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), "validated 18 operator-latency samples")

    def assert_rejected(self, receipt, label, *, diagnostic=None):
        result = self.invoke(receipt)
        self.assertEqual(
            result.returncode, 1,
            f"invalid receipt accepted: {label}; stdout={result.stdout!r}; stderr={result.stderr!r}",
        )
        self.assertNotIn("validated 18 operator-latency samples", result.stdout)
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

    def test_schema_byte_count_contract(self):
        schema = json.loads(self.schema_text)
        sample = schema["$defs"]["sample"]
        self.assertEqual(sample["properties"]["payload_bytes"], {"type": ["integer", "null"], "minimum": 0})
        self.assertEqual(sample["properties"]["semantic_payload_bytes"], {"type": ["integer", "null"], "minimum": 0})
        steps = schema["$defs"]["agent_loop"]["properties"]["composite"]["properties"]["steps"]["items"]["properties"]
        self.assertEqual(steps["payload_bytes"], {"type": ["integer", "null"], "minimum": 0})
        self.assertEqual(steps["semantic_payload_bytes"], {"type": ["integer", "null"], "minimum": 0})

    def test_byte_counts_match_retained_files(self):
        receipt = receipt_fixture()
        for repetition in range(2):
            with self.subTest(repetition=repetition):
                self.assert_accepted(receipt)

    def test_full_json_row_is_required_with_its_actual_command_and_artifacts(self):
        cases = (
            ("missing", lambda r: r["samples"][5].__setitem__("name", "replacement"),
             "expected exactly one full_check_json sample"),
            ("duplicate", lambda r: r["samples"][4].__setitem__("name", "full_check_json"),
             "expected exactly one full_check_json sample"),
            ("wrong_format", lambda r: r["samples"][5]["argv"].__setitem__(4, "markdown"),
             "full_check_json must measure the full JSON check command"),
            ("wrong_output", lambda r: r["samples"][5]["argv"].__setitem__(8, "other.json"), None),
            ("wrong_receipt", lambda r: r["samples"][5]["argv"].__setitem__(6, "other.json"), None),
            ("wrong_phase", lambda r: r["samples"][5].__setitem__("phase", "targeted"), None),
        )
        for label, mutate, diagnostic in cases:
            with self.subTest(label=label):
                receipt = receipt_fixture()
                mutate(receipt)
                self.assert_rejected(receipt, label, diagnostic=diagnostic)

    def test_retained_digests_and_file_refusals(self):
        # Each negative starts from the same accepted bytes and correct sizes.
        # A same-size substitution exercises hashing independently of size checks.
        self.assert_accepted(receipt_fixture())
        for index in (3, 5):
            for key in ("artifact", "semantic_artifact"):
                with self.subTest(index=index, key=key, mutation="digest"):
                    receipt = receipt_fixture()
                    receipt["samples"][index][key]["sha256"] = "0" * 64
                    self.assert_rejected(receipt, key, diagnostic=f"{key} sha256 must match")
                for mutation in ("same_size", "truncated", "missing"):
                    with self.subTest(index=index, key=key, mutation=mutation):
                        receipt = receipt_fixture()
                        path = receipt["samples"][index][key]["path"]
                        def mutate(root):
                            retained = root / path
                            if mutation == "missing":
                                retained.unlink()
                            else:
                                data = retained.read_bytes()
                                retained.write_bytes(b"x" + data[1:] if mutation == "same_size" else data[:-1])
                        result = self.invoke(receipt, mutate_retained=mutate)
                        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                        self.assertNotIn("validated 18 operator-latency samples", result.stdout)
                        diagnostic = {
                            "same_size": f"{key} sha256 must match",
                            "truncated": "must match the retained",
                            "missing": "FileNotFoundError",
                        }[mutation]
                        self.assertIn(diagnostic, result.stderr)

    def test_byte_count_admission_refusals(self):
        sample_index = 3
        cases = (
            ("artifact_negative",
             lambda r: r["samples"][sample_index].__setitem__("payload_bytes", -1),
             "must be nonnegative integers"),
            ("semantic_negative",
             lambda r: r["samples"][sample_index].__setitem__("semantic_payload_bytes", -1),
             "must be nonnegative integers"),
            ("artifact_type",
             lambda r: r["samples"][sample_index].__setitem__("payload_bytes", True),
             "must be nonnegative integers"),
            ("semantic_type",
             lambda r: r["samples"][sample_index].__setitem__("semantic_payload_bytes", 1.5),
             "must be nonnegative integers"),
            ("semantic_null",
             lambda r: r["samples"][sample_index].__setitem__("semantic_payload_bytes", None),
             "must be nonnegative integers"),
            ("artifact_file_mismatch",
             lambda r: r["samples"][sample_index].__setitem__(
                 "payload_bytes", r["samples"][sample_index]["payload_bytes"] + 1),
             "must match the retained artifact size"),
            ("semantic_file_mismatch",
             lambda r: r["samples"][sample_index].__setitem__(
                 "semantic_payload_bytes", r["samples"][sample_index]["semantic_payload_bytes"] + 1),
             "must match the retained semantic artifact size"),
            ("steps_semantic_negative",
             lambda r: r["agent_loop"]["composite"]["steps"][1].__setitem__(
                 "semantic_payload_bytes", -1),
             "byte counts must be nonnegative integers"),
            ("steps_semantic_missing",
             lambda r: r["agent_loop"]["composite"]["steps"][1].pop("semantic_payload_bytes"),
             "byte counts must be nonnegative integers"),
        )
        for label, mutate, diagnostic in cases:
            with self.subTest(label=label):
                receipt = receipt_fixture()
                mutate(receipt)
                self.assert_rejected(receipt, label, diagnostic=diagnostic)

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
        for field, value in (("schema_id", "wrong"), ("schema_version", 5), ("result", "failed")):
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

# Missing-interpreter refusal must precede disposable fixture allocation.
# Restrict the child PATH independently of the Python running these controls.
"${py}" - <<'PY_INTERPRETER_ALLOCATION'
import contextlib
import json
import os
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

script = Path("scripts/perf-budget-smoke.sh").read_text(encoding="utf-8")
bootstrap, boundary, _ = script.partition("\nPERF_BINARY_REL=")
assert boundary, "missing production bootstrap boundary"
bash = shutil.which("bash")
assert bash, "bash is required for interpreter-allocation controls"


class InterpreterAllocationControls(unittest.TestCase):
    @contextlib.contextmanager
    def fixture(self, mode):
        with tempfile.TemporaryDirectory(prefix="perf-python-allocation-") as temporary:
            root = Path(temporary)
            (root / "scripts").mkdir()
            owned = root / "target/cargo-allow-operator-latency.pre-existing"
            owned.mkdir(parents=True)
            (owned / "keep").write_text("unrelated fixture\n", encoding="utf-8")
            candidate = root / "target/selected/cargo-allow"
            candidate.parent.mkdir()
            candidate.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            candidate.chmod(0o755)
            probe = root / "scripts/perf-budget-smoke.sh"
            probe.write_text(
                bootstrap
                + '\nprintf "bootstrap-ready:%s\\n" "$py"\n'
                + '"${py}" -c \'import sys; assert sys.version_info[0] >= 3; print("selected-python3")\'\n',
                encoding="utf-8",
            )

            tools = root / "controlled bin"
            tools.mkdir()
            for command in ("dirname", "mkdir", "rm", "uname"):
                resolved = shutil.which(command)
                self.assertIsNotNone(resolved, command)
                (tools / command).symlink_to(resolved)
            # Match production admission on GNU and shasum-only hosts.
            # The custom-path positive also exercises fallback admission.
            hash_commands = ("shasum", "sha256sum") if mode == "custom" else (
                "sha256sum", "shasum",
            )
            digest_tool = next(
                (command for command in hash_commands if shutil.which(command)), None,
            )
            self.assertIsNotNone(digest_tool, "a SHA-256 utility is required")
            (tools / digest_tool).symlink_to(shutil.which(digest_tool))
            mktemp = shutil.which("mktemp")
            self.assertIsNotNone(mktemp)
            allocator = tools / "mktemp"
            allocator.write_text(
                '#!/bin/sh\nprintf "allocated\\n" >> "$ALLOCATOR_LOG"\n'
                + "exec " + shlex.quote(mktemp) + ' "$@"\n',
                encoding="utf-8",
            )
            allocator.chmod(0o755)

            preferred = "python3"
            selected = None
            if mode in ("default", "fallback", "custom"):
                name = {"default": "python3", "fallback": "python",
                        "custom": "preferred python"}[mode]
                interpreter = tools / name
                interpreter.write_text(
                    '#!/bin/sh\nprintf "%s\\n" "$*" >> "$INTERPRETER_LOG"\n'
                    + "exec " + shlex.quote(sys.executable) + ' "$@"\n',
                    encoding="utf-8",
                )
                interpreter.chmod(0o755)
                if mode == "custom":
                    preferred = selected = str(interpreter)
                elif mode == "fallback":
                    preferred, selected = "missing-preferred-python", "python"
                else:
                    selected = "python3"
            elif mode == "host":
                preferred = selected = sys.executable
            elif mode == "reject-fallback":
                preferred = "missing-preferred-python"
                interpreter = tools / "python"
                interpreter.write_text(
                    '#!/bin/sh\nprintf "%s\\n" "$*" >> "$INTERPRETER_LOG"\nexit 1\n',
                    encoding="utf-8",
                )
                interpreter.chmod(0o755)
            elif mode != "none":
                self.fail("unknown interpreter fixture mode: " + mode)

            yield {
                "root": root, "probe": probe, "tools": tools,
                "owned": owned, "preferred": preferred, "selected": selected,
                "digest_tool": digest_tool,
                "allocator_log": root / "allocator.calls",
                "interpreter_log": root / "interpreter.calls",
            }

    def invoke(self, fixture, *, profile="debug"):
        environment = os.environ.copy()
        environment.update({
            "PATH": str(fixture["tools"]),
            "PYTHON3": fixture["preferred"],
            "CARGO_ALLOW_BIN": "target/selected/cargo-allow",
            "OUTPUT_DIR": str(fixture["root"] / "receipt"),
            "PROFILE": profile,
            "ALLOCATOR_LOG": str(fixture["allocator_log"]),
            "INTERPRETER_LOG": str(fixture["interpreter_log"]),
            "CDPATH": "",
        })
        return subprocess.run(
            [bash, str(fixture["probe"])], cwd=fixture["root"],
            env=environment, capture_output=True, text=True, timeout=20,
        )

    def calls(self, path):
        return path.read_text(encoding="utf-8").splitlines() if path.exists() else []

    def new_fixtures(self, fixture):
        return sorted(path.name for path in (fixture["root"] / "target").glob(
            "cargo-allow-operator-latency.*"
        ) if path != fixture["owned"])

    def assert_clean(self, fixture):
        self.assertEqual(self.new_fixtures(fixture), [])
        self.assertEqual(
            (fixture["owned"] / "keep").read_text(encoding="utf-8"),
            "unrelated fixture\n",
        )

    def test_missing_or_rejected_interpreter_never_allocates(self):
        for mode in ("none", "reject-fallback"):
            with self.fixture(mode) as fixture:
                for repetition in range(2):
                    with self.subTest(mode=mode, repetition=repetition):
                        result = self.invoke(fixture)
                        print(
                            f"interpreter refusal {mode} repeat{repetition}: "
                            f"exit={result.returncode} "
                            f"allocations={len(self.calls(fixture['allocator_log']))} "
                            f"new_fixtures={self.new_fixtures(fixture)}",
                        )
                        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                        self.assertIn(
                            "a Python 3 interpreter is required to write the JSON receipt",
                            result.stderr,
                        )
                        self.assertNotIn("bootstrap-ready:", result.stdout)
                        self.assert_clean(fixture)
                        self.assertEqual(self.calls(fixture["allocator_log"]), [])
                        expected_calls = repetition + 1 if mode == "reject-fallback" else 0
                        self.assertEqual(
                            len(self.calls(fixture["interpreter_log"])), expected_calls,
                        )

    def test_usable_selection_allocates_and_cleans(self):
        for mode in ("default", "custom", "fallback", "host"):
            with self.fixture(mode) as fixture:
                for repetition in range(2):
                    with self.subTest(mode=mode, repetition=repetition):
                        result = self.invoke(fixture)
                        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                        self.assertEqual(
                            result.stdout.splitlines(),
                            ["bootstrap-ready:" + fixture["selected"], "selected-python3"],
                        )
                        print(f"usable interpreter {mode} repeat{repetition}: "
                              f"hash_admission={fixture['digest_tool']}")
                        self.assert_clean(fixture)
                        self.assertEqual(
                            len(self.calls(fixture["allocator_log"])), repetition + 1,
                        )

    def test_failure_after_setup_keeps_receipt_and_cleanup(self):
        with self.fixture("host") as fixture:
            for repetition in range(2):
                with self.subTest(repetition=repetition):
                    result = self.invoke(fixture, profile="invalid")
                    self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                    self.assertIn("PROFILE must be debug or release", result.stderr)
                    self.assertNotIn("bootstrap-ready:", result.stdout)
                    receipt = json.loads((fixture["root"] / "receipt" /
                                          "operator-latency.receipt.json").read_text(
                        encoding="utf-8",
                    ))
                    self.assertEqual(receipt["result"], "failed")
                    self.assertEqual(receipt["failure"], {
                        "kind": "instrument_failure",
                        "message": "PROFILE must be debug or release",
                    })
                    self.assert_clean(fixture)
                    self.assertEqual(
                        len(self.calls(fixture["allocator_log"])), repetition + 1,
                    )


if __name__ == "__main__":
    unittest.main(verbosity=2)
PY_INTERPRETER_ALLOCATION

# Execute the real hooks measurement with independently controlled stdout.
# The child emits UTF-8 bytes; the limit is a byte budget, not a character count.
"${py}" - <<'PY_HOOK_PAYLOAD'
import contextlib
import hashlib
import json
import os
import shlex
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

script = Path("scripts/perf-budget-smoke.sh").read_text(encoding="utf-8")
bootstrap, boundary, _ = script.partition("\nPERF_BINARY_REL=")
assert boundary, "missing production bootstrap boundary"
_, boundary, tail = script.partition("\nmeasure_hooks_sample() {\n")
assert boundary, "missing production hooks measurement"
body, boundary, _ = tail.partition("\n}\n")
assert boundary, "missing hooks measurement end"
measurement = "\nmeasure_hooks_sample() {\n" + body + "\n}\n"
CEILING = 524288
SAMPLES = ("hooks_wrapped_check", "hooks_bare_check")


class HookPayloadControls(unittest.TestCase):
    @contextlib.contextmanager
    def fixture(self):
        with tempfile.TemporaryDirectory(prefix="perf-hooks-payload-") as temporary:
            root = Path(temporary)
            (root / "scripts").mkdir()
            clone = root / "fixture clone"
            clone.mkdir()
            candidate = root / "target/selected/cargo-allow"
            candidate.parent.mkdir(parents=True)
            child = root / "controlled-child.py"
            child.write_text(
                "import sys\n"
                "size, marker, status = int(sys.argv[1]), sys.argv[2], int(sys.argv[3])\n"
                "prefix = (marker + '\\n').encode('utf-8')\n"
                "assert size >= len(prefix)\n"
                "remaining = size - len(prefix)\n"
                "sys.stdout.buffer.write(prefix + b'\\xc3\\xa9' * (remaining // 2)"
                " + b'x' * (remaining % 2))\n"
                "sys.stdout.flush()\nsys.exit(status)\n",
                encoding="utf-8",
            )
            candidate.write_text(
                "#!/bin/sh\nexec " + shlex.quote(sys.executable) + " "
                + shlex.quote(str(child)) + ' "$@"\n',
                encoding="utf-8",
            )
            candidate.chmod(0o755)
            probe = root / "scripts/perf-budget-smoke.sh"
            probe.write_text(
                bootstrap + measurement
                + '\nagent_loop_root="$PROBE_CWD"\n'
                + 'if [[ "$PROBE_CLOCK_MODE" == over ]]; then\n'
                + '  now_ms() {\n'
                + '    if [[ -e "$PROBE_CLOCK" ]]; then printf 60001; '
                + 'else : >"$PROBE_CLOCK"; printf 0; fi\n'
                + '  }\nfi\n'
                + 'measure_hooks_sample "$PROBE_SAMPLE" '
                + '"artifacts/$PROBE_SAMPLE.md" "Result: passed" '
                + '"$PROBE_SIZE" "$PROBE_MARKER" "$PROBE_EXIT"\n'
                + 'write_receipt "pass" ""\n'
                + 'printf "hooks-probe-complete\\n"\n',
                encoding="utf-8",
            )
            yield root, clone, probe

    def invoke(self, fixture, sample, size, *, marker="Result: passed",
               status=0, clock="normal"):
        root, clone, probe = fixture
        environment = os.environ.copy()
        environment.update({
            "CARGO_ALLOW_BIN": "target/selected/cargo-allow",
            "OUTPUT_DIR": str(root / "receipt"),
            "PROFILE": "debug", "HARD_CEILING_MS": "60000",
            "PYTHON3": sys.executable, "CDPATH": "",
            "PROBE_CWD": str(clone), "PROBE_SAMPLE": sample,
            "PROBE_SIZE": str(size), "PROBE_MARKER": marker,
            "PROBE_EXIT": str(status), "PROBE_CLOCK_MODE": clock,
            "PROBE_CLOCK": str(root / "controlled-clock"),
            "PERF_AGENT_LOOP_SUMMARY": "",
        })
        (root / "controlled-clock").unlink(missing_ok=True)
        result = subprocess.run(
            ["bash", str(probe)], cwd=clone, env=environment,
            capture_output=True, text=True, timeout=20,
        )
        artifact = root / "receipt/artifacts" / (sample + ".md")
        metrics = (root / "receipt/.operator-latency.samples.tsv").read_text(encoding="utf-8")
        receipt = json.loads((root / "receipt/operator-latency.receipt.json")
                             .read_text(encoding="utf-8"))
        self.assertEqual(len(artifact.read_bytes()), size)
        self.assertFalse(list((root / "target").glob("cargo-allow-operator-latency.*")))
        return result, artifact, metrics, receipt

    def test_valid_byte_boundaries_record_real_payload(self):
        for sample in SAMPLES:
            with self.fixture() as fixture:
                for size in (64, CEILING - 1, CEILING):
                    with self.subTest(sample=sample, size=size):
                        result, artifact, metrics, receipt = self.invoke(fixture, sample, size)
                        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                        self.assertIn("hooks-probe-complete", result.stdout)
                        fields = metrics.strip().split("\t")
                        self.assertEqual(len(fields), 11)
                        self.assertEqual(fields[1], sample)
                        self.assertEqual(fields[7], "passed")
                        self.assertEqual(int(fields[9]), size)
                        self.assertEqual(int(fields[10]), size)
                        self.assertEqual(fields[4], hashlib.sha256(artifact.read_bytes()).hexdigest())
                        self.assertEqual(fields[3:5], fields[5:7])
                        self.assertEqual(receipt["result"], "pass")
                        self.assertEqual(receipt["samples"][0]["payload_bytes"], size)
                        self.assertEqual(receipt["samples"][0]["semantic_payload_bytes"], size)
                        print(f"hooks payload positive {sample}: {size}B accepted")

    def test_oversized_byte_payload_refuses_repeatedly(self):
        for sample in SAMPLES:
            with self.fixture() as fixture:
                for repetition in range(2):
                    with self.subTest(sample=sample, repetition=repetition):
                        result, artifact, metrics, receipt = self.invoke(
                            fixture, sample, CEILING + 1,
                        )
                        print(f"hooks payload refusal {sample} repeat{repetition}: "
                              f"exit={result.returncode} bytes={len(artifact.read_bytes())}")
                        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                        self.assertIn(
                            f"{sample} payload {sample}.md at {CEILING + 1}B exceeded "
                            f"the {CEILING}B catastrophic payload ceiling", result.stderr,
                        )
                        self.assertNotIn("hooks-probe-complete", result.stdout)
                        self.assertEqual(metrics, "")
                        self.assertEqual(receipt["result"], "failed")
                        self.assertEqual(receipt["failure"]["kind"], "instrument_failure")
                        self.assertIn("catastrophic payload ceiling", receipt["failure"]["message"])

    def test_existing_failure_gates_remain_independent(self):
        for sample in SAMPLES:
            with self.fixture() as fixture:
                controls = (
                    ({"marker": "Wrong result"}, "semantic result did not contain expected marker"),
                    ({"status": 2}, "hooks sample failed (exit 2)"),
                    ({"clock": "over"}, "exceeded the 60000ms catastrophic ceiling (60001ms)"),
                )
                for options, diagnostic in controls:
                    with self.subTest(sample=sample, options=options):
                        result, _, metrics, receipt = self.invoke(fixture, sample, 64, **options)
                        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                        self.assertIn(diagnostic, result.stderr)
                        self.assertNotIn("hooks-probe-complete", result.stdout)
                        self.assertEqual(metrics, "")
                        self.assertEqual(receipt["result"], "failed")
                        self.assertIn(diagnostic, receipt["failure"]["message"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
PY_HOOK_PAYLOAD

# Execute the new matrix row and production measurement/validation functions.
# Controlled child artifacts exercise refusal and retention, not real latency.
"${py}" - <<'PY_FULL_CHECK_JSON'
import ast
import copy
import hashlib
import json
import os
import shlex
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

script = Path("scripts/perf-budget-smoke.sh").read_text(encoding="utf-8")
bootstrap, boundary, _ = script.partition("\nPERF_BINARY_REL=")
assert boundary, "missing production bootstrap boundary"
_, boundary, tail = script.partition("\nmeasure() {\n")
assert boundary, "missing production measurement"
body, boundary, _ = tail.partition("\n}\n")
assert boundary, "missing production measurement end"
measurement = "\nmeasure() {\n" + body + "\n}\n"
_, boundary, tail = script.partition('log "measuring warm full check JSON report and receipt"\n')
assert boundary, "missing full JSON matrix row"
matrix_row, boundary, _ = tail.partition('\nlog "measuring targeted why"')
assert boundary, "missing full JSON matrix row end"
REPORT_LIMIT, RECEIPT_LIMIT = 8388608, 16384
STATUSES = (
    "matched", "new", "expired", "review_due", "location_drift", "stale",
    "ambiguous", "invalid_selector", "evidence_missing", "missing_required_field", "baseline_debt",
)
REPORT_SCHEMA = Path("docs/schemas/report.schema.json").read_bytes()


def command_artifacts():
    counts = {status: 0 for status in STATUSES}
    counts.update(matched=2, stale=1, weak_evidence_references=2)
    shared = {
        "schema_version": 1, "tool": "cargo-allow", "command": "check",
        "status": "passed", "failed": False,
        "claim_boundary": ["source_tree_inventory", "source_syntax_only"],
        "scanner_limitations": ["cargo_metadata_not_invoked"],
        "inventory": {"scope": "source_tree", "scanner": "source_syntax",
                      "source": "git_tracked", "completeness": "scoped", "files_scanned": 2},
        "source_inventory": {
            "findings": 2,
            "by_kind": [{"kind": kind, "total": 1, "matched": 1, "new": 0, "review_items": 0}
                        for kind in ("non_rust_file", "panic")],
            "by_family": [{"kind": kind, "family": family, "label": f"{kind}.{family}",
                           "total": 1, "matched": 1, "new": 0, "review_items": 0}
                          for kind, family in (("non_rust_file", "documentation"), ("panic", "unwrap"))],
        },
        "evidence_repair_queues": [],
    }
    report = copy.deepcopy(shared)
    report.update({
        "schema_id": "cargo-allow.report.v1",
        "rust_scanner": {"completeness": "scoped", "files_considered": 1, "files_scanned": 1,
                         "files_skipped": 0, "files_with_parse_errors": 0,
                         "skipped_by_reason": {"read_failed_or_unsupported": 0}},
        "summary": {"findings": 2, "outcomes": 3, **counts},
        "findings": [
            {"kind": "non_rust_file", "family": "documentation", "path": "docs/café.md",
             "line": None, "container": None, "ast_kind": "file"},
            {"kind": "panic", "family": "unwrap", "path": "src/lib.rs",
             "line": 1, "container": None, "ast_kind": "method_call"},
        ],
        "outcomes": [
            {"status": status, "allow_id": f"fixture-{index}", "candidate_ids": [],
             "finding_index": index if index < 2 else None, "score": 0, "message": "fixture"}
            for index, status in enumerate(("matched", "matched", "stale"))
        ],
    })
    receipt = copy.deepcopy(shared)
    receipt.update({
        "schema_id": "cargo-allow.receipt.v1", "mode": "no-new", "counts": counts,
        "advisory": {"review_items": 3, **{key: value for key, value in counts.items() if key != "matched"}},
        "run_id": "json-run", "started_at": "2026-10-10T00:00:01Z",
        "git_sha": "a" * 40, "policy_digest": "sha256:v1:" + "b" * 64,
    })
    reference = copy.deepcopy(receipt)
    reference.update(run_id="markdown-run", started_at="2026-10-10T00:00:00Z")
    audit = copy.deepcopy(report)
    audit.update(command="audit", audit_remediation_roadmap=[])
    return report, receipt, reference, audit


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


class FullCheckJsonControls(unittest.TestCase):
    def invoke(self, artifacts=None, *, raw=None, missing=(), sizes=None, status=0, clock="normal"):
        values = command_artifacts() if artifacts is None else artifacts
        self.assertEqual(len(values), 4, "the audit control must be supplied independently of mutated check detail")
        inputs = dict(zip(("report", "receipt", "reference", "audit"), map(encoded, values)))
        inputs.update(raw or {})
        for key, size in (sizes or {}).items():
            self.assertGreaterEqual(size, len(inputs[key]))
            inputs[key] += b" " * (size - len(inputs[key]))
        with tempfile.TemporaryDirectory(prefix="perf-full-json-") as directory:
            root = Path(directory)
            (root / "scripts").mkdir()
            (root / "docs/schemas").mkdir(parents=True)
            (root / "docs/schemas/report.schema.json").write_bytes(REPORT_SCHEMA)
            (root / "input").mkdir()
            (root / "outside caller").mkdir()
            output = root / "receipt/artifacts"
            output.mkdir(parents=True)
            for key in ("report", "receipt"):
                if key not in missing:
                    (root / "input" / key).write_bytes(inputs[key])
            if "reference" not in missing:
                (output / "warm-check.receipt.json").write_bytes(inputs["reference"])
            if "audit" not in missing:
                (output / "first-audit.json").write_bytes(inputs["audit"])
            # Missing output cannot reuse an artifact left by an earlier sample.
            for name in ("full-check.json", "full-check.receipt.json"):
                (output / name).write_text("stale output", encoding="utf-8")
            child = root / "controlled-child.py"
            child.write_text(
                "import json, os, shutil, sys\nfrom pathlib import Path\n"
                "args = sys.argv[1:]\n"
                "Path(os.environ['PROBE_DISPATCH']).write_text(json.dumps({'argv': args, 'cwd': str(Path.cwd())}))\n"
                "for key, flag in (('report', '--output'), ('receipt', '--receipt')):\n"
                "    source = Path(os.environ['PROBE_INPUT']) / key\n"
                "    if source.is_file(): shutil.copyfile(source, args[args.index(flag) + 1])\n"
                "sys.exit(int(os.environ['PROBE_EXIT']))\n",
                encoding="utf-8",
            )
            candidate = root / "target/selected/cargo-allow"
            candidate.parent.mkdir(parents=True)
            candidate.write_text("#!/bin/sh\nexec " + shlex.quote(sys.executable) + " "
                                 + shlex.quote(str(child)) + ' "$@"\n', encoding="utf-8")
            candidate.chmod(0o755)
            # Metadata remains unknown in this fixture; do not probe a host Rust toolchain.
            tools = root / "fixture-tools"
            tools.mkdir()
            (tools / "rustc").write_text("#!/bin/sh\nexit 127\n", encoding="utf-8")
            (tools / "rustc").chmod(0o755)
            probe = root / "scripts/perf-budget-smoke.sh"
            probe.write_text(
                bootstrap + measurement + '\nnow_ms() {\n'
                + '  if [[ "$PROBE_CLOCK_MODE" == over && -e "$PROBE_CLOCK" ]]; then printf 60001; '
                + 'else : >"$PROBE_CLOCK"; printf 0; fi\n}\n'
                + matrix_row + '\nwrite_receipt "pass" ""\nprintf "full-json-probe-complete\\n"\n',
                encoding="utf-8",
            )
            environment = os.environ.copy()
            environment.update({
                "PATH": str(tools) + os.pathsep + environment["PATH"],
                "CARGO_ALLOW_BIN": "target/selected/cargo-allow", "PROFILE": "debug",
                "OUTPUT_DIR": str(root / "receipt"), "HARD_CEILING_MS": "60000",
                "PYTHON3": sys.executable, "CDPATH": "", "PERF_AGENT_LOOP_SUMMARY": "",
                "PROBE_INPUT": str(root / "input"), "PROBE_EXIT": str(status),
                "PROBE_DISPATCH": str(root / "dispatch.json"),
                "PROBE_CLOCK_MODE": clock, "PROBE_CLOCK": str(root / "clock"),
            })
            result = subprocess.run(["bash", str(probe)], cwd=root / "outside caller", env=environment,
                                    capture_output=True, text=True, timeout=20)
            dispatch = json.loads((root / "dispatch.json").read_text(encoding="utf-8"))
            self.assertEqual(dispatch, {
                "cwd": str(root),
                "argv": ["check", "--mode", "no-new", "--format", "json", "--receipt",
                         str(output / "full-check.receipt.json"), "--output", str(output / "full-check.json")],
            })
            metrics = (root / "receipt/.operator-latency.samples.tsv").read_text(encoding="utf-8")
            receipt = json.loads((root / "receipt/operator-latency.receipt.json").read_text(encoding="utf-8"))
            self.assertFalse(list((root / "target").glob("cargo-allow-operator-latency.*")))
            return result, metrics, receipt, inputs

    def assert_refused(self, diagnostic, *args, **kwargs):
        result, metrics, receipt, _ = self.invoke(*args, **kwargs)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn(diagnostic, result.stderr)
        self.assertNotIn("full-json-probe-complete", result.stdout)
        self.assertEqual(metrics, "")
        self.assertEqual(receipt["result"], "failed")
        self.assertEqual(receipt["samples"], [])
        self.assertEqual(receipt["failure"]["kind"], "instrument_failure")
        self.assertIn(diagnostic, receipt["failure"]["message"])

    def test_valid_command_retains_exact_bytes_and_distinct_receipt(self):
        for sizes in ({}, {"report": REPORT_LIMIT, "receipt": RECEIPT_LIMIT}):
            with self.subTest(sizes=sizes):
                result, metrics, receipt, inputs = self.invoke(sizes=sizes)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn("full-json-probe-complete", result.stdout)
                fields = metrics.strip().split("\t")
                self.assertEqual(len(fields), 11)
                self.assertEqual(fields[:3], ["warm", "full_check_json", "0"])
                self.assertEqual(fields[3], "artifacts/full-check.json")
                self.assertEqual(fields[5], "artifacts/full-check.receipt.json")
                self.assertEqual(fields[7], "passed")
                for key, digest_field, size_field in (("report", 4, 9), ("receipt", 6, 10)):
                    self.assertEqual(fields[digest_field], hashlib.sha256(inputs[key]).hexdigest())
                    self.assertEqual(int(fields[size_field]), len(inputs[key]))
                self.assertEqual(receipt["result"], "pass")
                self.assertEqual(receipt["schema_version"], 4)
                self.assertEqual(receipt["samples"][0]["cache_mode"], "not_applicable")
                self.assertEqual(receipt["sample_policy"]["warm_process_samples"], 1)

    def test_missing_truncated_duplicate_and_non_json_artifacts_refuse(self):
        values = command_artifacts()
        for index, key in enumerate(("report", "receipt", "reference", "audit")):
            with self.subTest(key=key, mutation="missing"):
                diagnostic = "did not produce" if key in ("report", "receipt") else "report/receipt validation failed"
                self.assert_refused(diagnostic, missing=(key,))
            for label, data, diagnostic in (
                ("truncated", encoded(values[index])[:-2], "report/receipt validation failed"),
                ("duplicate", encoded(values[index]).rstrip()[:-1] + b', "failed": false}', "duplicate JSON key"),
                ("non_json", encoded(values[index]).replace(b'"schema_version": 1', b'"schema_version": NaN'),
                 "non-JSON numeric constant"),
            ):
                with self.subTest(key=key, mutation=label):
                    self.assert_refused(diagnostic, raw={key: data})

    def test_semantic_mismatches_refuse_without_recording_a_passed_row(self):
        cases = (
            ("schema", lambda r, c, p: r.__setitem__("schema_id", "cargo-allow.receipt.v1"), "expected cargo-allow.report.v1"),
            ("schema_bool", lambda r, c, p: r.__setitem__("schema_version", True), "schema_version must be 1"),
            ("status", lambda r, c, p: r.__setitem__("status", "failed"), "expected a passed check result"),
            ("inventory", lambda r, c, p: r["inventory"].__setitem__("files_scanned", 3), "report/receipt inventory differs"),
            ("policy", lambda r, c, p: c.__setitem__("policy_digest", "sha256:v1:" + "c" * 64), "warm_check semantic result"),
            ("findings", lambda r, c, p: r["findings"].pop(), "arrays differ from summary lengths"),
            ("outcomes", lambda r, c, p: r["outcomes"].pop(), "arrays differ from summary lengths"),
            ("counts", lambda r, c, p: r["summary"].__setitem__("matched", 3), "summary differs from receipt counts"),
            ("count_bool", lambda r, c, p: r["summary"].__setitem__("new", False), "summary counts must be nonnegative integers"),
            ("outcome_status", lambda r, c, p: r["outcomes"][0].__setitem__("status", "new"), "outcomes differ from receipt counts"),
            ("unknown_status", lambda r, c, p: r["outcomes"][0].__setitem__("status", "unknown"), "unknown outcome status"),
            ("index", lambda r, c, p: r["outcomes"][0].__setitem__("finding_index", 2), "finding_index is outside"),
            ("index_bool", lambda r, c, p: r["outcomes"][0].__setitem__("finding_index", True), "finding_index is outside"),
        )
        for label, mutate, diagnostic in cases:
            with self.subTest(label=label):
                values = command_artifacts()
                mutate(*values[:3])
                self.assert_refused(diagnostic, values)

    def test_full_report_detail_mutations_refuse_against_unchanged_audit(self):
        cases = (
            ("empty_findings", lambda r: r.__setitem__("findings", [{} for _ in r["findings"]]),
             "missing required detail field"),
            ("duplicated_finding", lambda r: r.__setitem__("findings", [copy.deepcopy(r["findings"][0]) for _ in r["findings"]]),
             "first_audit semantic content"),
            ("erased_outcome_fields", lambda r: r.__setitem__("outcomes", [{key: item[key] for key in ("status", "finding_index")} for item in r["outcomes"]]),
             "missing required detail field"),
            ("null_links", lambda r: [item.__setitem__("finding_index", None) for item in r["outcomes"]],
             "first_audit semantic content"),
            ("reordered_findings", lambda r: r["findings"].reverse(), "first_audit semantic content"),
            ("changed_path", lambda r: r["findings"][0].__setitem__("path", "docs/other.md"), "first_audit semantic content"),
            ("changed_message", lambda r: r["outcomes"][0].__setitem__("message", "substitute"), "first_audit semantic content"),
            ("changed_allow_id", lambda r: r["outcomes"][0].__setitem__("allow_id", "substitute"), "first_audit semantic content"),
            ("changed_candidates", lambda r: r["outcomes"][0].__setitem__("candidate_ids", ["substitute"]), "first_audit semantic content"),
            ("changed_scanner", lambda r: r["rust_scanner"].__setitem__("files_scanned", 0), "first_audit semantic content"),
            ("bool_scanner_count", lambda r: r["rust_scanner"].__setitem__("files_scanned", True), "first_audit semantic content"),
            ("audit_field_on_check", lambda r: r.__setitem__("audit_remediation_roadmap", []), "first_audit semantic content"),
        )
        for label, mutate, diagnostic in cases:
            with self.subTest(label=label):
                values = command_artifacts()
                retained_controls = [encoded(value) for value in values[1:]]
                mutate(values[0])
                self.assertEqual([encoded(value) for value in values[1:]], retained_controls)
                self.assert_refused(diagnostic, values)

    def test_shared_invalid_detail_fields_refuse_before_content_parity(self):
        # Even a matching malformed audit cannot bless erased or ill-typed
        # detail. Field rules come from the existing report schema.
        cases = (
            ("missing_path", lambda r: r["findings"][0].pop("path"), "missing required detail field"),
            ("empty_path", lambda r: r["findings"][0].__setitem__("path", ""), "invalid detail field length"),
            ("line_bool", lambda r: r["findings"][1].__setitem__("line", True), "invalid detail field type"),
            ("line_zero", lambda r: r["findings"][1].__setitem__("line", 0), "invalid detail field minimum"),
            ("unknown_kind", lambda r: r["findings"][0].__setitem__("kind", "invented"), "invalid detail field value"),
            ("missing_message", lambda r: r["outcomes"][0].pop("message"), "missing required detail field"),
            ("score_float", lambda r: r["outcomes"][0].__setitem__("score", 0.0), "invalid detail field type"),
            ("candidate_type", lambda r: r["outcomes"][0].__setitem__("candidate_ids", [None]), "invalid detail field type"),
            ("candidate_empty", lambda r: r["outcomes"][0].__setitem__("candidate_ids", [""]), "invalid detail field length"),
            ("unknown_field", lambda r: r["findings"][0].__setitem__("omitted_detail", True), "unknown detail field"),
        )
        for label, mutate, diagnostic in cases:
            with self.subTest(label=label):
                values = command_artifacts()
                mutate(values[0])
                mutate(values[3])
                self.assert_refused(diagnostic, values)

    def test_audit_control_identity_and_presence_refuse(self):
        values = command_artifacts()
        values[3]["command"] = "check"
        self.assert_refused("expected a cargo-allow audit artifact", values)
        values = command_artifacts()
        values[3].pop("rust_scanner")
        self.assert_refused("missing required report field: first_audit", values)
        values = command_artifacts()
        values[3]["findings"][0]["path"] = "docs/other.md"
        self.assert_refused("first_audit semantic content", values)

    def test_payload_and_existing_command_gates_refuse(self):
        self.assert_refused("8388608B catastrophic payload ceiling", sizes={"report": REPORT_LIMIT + 1})
        self.assert_refused("16384B catastrophic payload ceiling", sizes={"receipt": RECEIPT_LIMIT + 1})
        self.assert_refused("command failed (exit 2)", status=2)
        self.assert_refused("exceeded the 60000ms catastrophic ceiling (60001ms)", clock="over")

    def test_status_count_keys_match_the_existing_report_schema(self):
        schema = json.loads(Path("docs/schemas/report.schema.json").read_text(encoding="utf-8"))
        body = script.split("validate_full_check_json() {", 1)[1].split("<<'PY'\n", 1)[1].split("\nPY", 1)[0]
        assignments = [node for node in ast.walk(ast.parse(body)) if isinstance(node, ast.Assign)
                       and any(isinstance(target, ast.Name) and target.id == "statuses" for target in node.targets)]
        self.assertEqual(len(assignments), 1)
        self.assertEqual(set(ast.literal_eval(assignments[0].value)), set(schema["$defs"]["match_status"]["enum"]))
        self.assertEqual(set(STATUSES), set(schema["$defs"]["match_status"]["enum"]))


if __name__ == "__main__":
    unittest.main(verbosity=2)
PY_FULL_CHECK_JSON

printf 'ok operator-latency harness characterization\n'
