#!/usr/bin/env python3
"""Offline characterization for the #2267 release publication gate.

Pins the aggregate/verify decision logic of
scripts/release-source-candidate-smoke-gate.py so every refusal row of the
issue's aggregate law (missing, skipped, cancelled, timed-out, stale,
partial package graph, failed journey, unclaimed receipt, unexpected row)
reproduces without a toolchain or a hosted runner. Hosted release runs
remain the authoritative integration proof; this suite only proves the
checker refuses every non-clean input form and accepts one exact green
composition."""

from __future__ import annotations

import contextlib
import hashlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "smoke_gate", ROOT / "scripts/release-source-candidate-smoke-gate.py"
)
smoke_gate = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(smoke_gate)

FIXTURE = ROOT / "docs/dogfood/fixtures/release/candidate-crate-set.toml"
CRATES = json.loads(json.dumps([]))  # replaced in setUpClass via tomllib


def read_fixture_crates() -> list[str]:
    import tomllib

    document = tomllib.loads(FIXTURE.read_text(encoding="utf-8"))
    return list(document["crates"])


COMMIT = "a" * 40
TREE = "b" * 40
VERSION = "0.2.0"
REPOSITORY = "EffortlessMetrics/cargo-allow"
REF = "refs/tags/v0.2.0"
TAG = "v0.2.0"
RUN_ID = "12345"
RUN_ATTEMPT = "1"
PLATFORMS = {row["platform"]: row for row in smoke_gate.REQUIRED_ROWS}


def expected_args(**overrides):
    fields = {
        "expected_repository": REPOSITORY,
        "expected_ref": REF,
        "expected_commit": COMMIT,
        "expected_tree": TREE,
        "expected_version": VERSION,
        "expected_tag": TAG,
        "expected_run_id": RUN_ID,
        "expected_run_attempt": RUN_ATTEMPT,
        "rows_dir": "",
        "fixture": str(FIXTURE),
        "needs_result": "success",
        "output": "",
        "aggregate": "",
        "mode": "aggregate",
    }
    fields.update(overrides)
    return fields


def harness_receipt(platform: str) -> dict:
    declared = PLATFORMS[platform]
    return {
        "schema_version": 1,
        "schema_id": smoke_gate.HARNESS_SCHEMA_ID,
        "tool": "cargo-allow",
        "result": smoke_gate.HARNESS_RESULT_PASSED,
        "claim_boundary": ["installed_binary_first_hour_journey"],
        "candidate": {
            "workspace_version": VERSION,
            "git_head": COMMIT,
            "package_set_provenance": "workspace_path_install_after_optional_package_gate",
            "install_method": "cargo_install_path",
        },
        "environment": {
            "os": platform,
            "arch": "x86_64",
            "rustc_version": "rustc 1.95.0",
            "cargo_version": "cargo 1.95.0",
            "network_posture": "not_required_for_core_journey",
        },
        "installed_binary": {
            "binary_sha256": "c" * 64,
            "version_output": f"cargo-allow {VERSION}",
            "path_redacted": True,
        },
        "journey": {
            "fixture_generation": "first_hour_brownfield_v1",
            "steps_expected": ["version", "doctor_no_policy", "final_check_no_new"],
            "steps_executed": [
                {"id": step, "exit_code": 0, "artifact_schema_id": "none"}
                for step in ["version", "doctor_no_policy", "final_check_no_new"]
            ],
        },
        "negative_controls": [{"id": "ordinary_scan_offline", "passed": True}],
        "limitations": ["does_not_prove_crates_io_install"],
        "selected_matrix": declared,
    }


def row_receipt(platform: str, crates: list[str], harness_path: Path) -> dict:
    declared = PLATFORMS[platform]
    return {
        "repository": REPOSITORY,
        "ref": REF,
        "candidate_commit": COMMIT,
        "candidate_tree": TREE,
        "candidate_version": VERSION,
        "tag": TAG,
        "platform": platform,
        "target": declared["target"],
        "toolchain": declared["toolchain"],
        "required": True,
        "result": smoke_gate.HARNESS_RESULT_PASSED,
        "packaged_crates": crates,
        "harness_receipt_sha256": "sha256:"
        + hashlib.sha256(harness_path.read_bytes()).hexdigest(),
        "workflow_run_id": RUN_ID,
        "workflow_run_attempt": RUN_ATTEMPT,
    }


def write_row(rows_dir: Path, platform: str, crates: list[str]) -> Path:
    row_dir = rows_dir / platform
    row_dir.mkdir(parents=True)
    harness_path = row_dir / "source-candidate-smoke.receipt.json"
    harness_path.write_bytes(
        (json.dumps(harness_receipt(platform), indent=2) + "\n").encode("utf-8")
    )
    (row_dir / f"release-source-candidate-smoke-{platform}.receipt.json").write_bytes(
        (json.dumps(row_receipt(platform, crates, harness_path), indent=2) + "\n").encode("utf-8")
    )
    return row_dir


class AggregateLawTests(unittest.TestCase):
    def green_rows(self, root: Path) -> Path:
        rows_dir = root / "rows"
        rows_dir.mkdir()
        crates = read_fixture_crates()
        for platform in PLATFORMS:
            write_row(rows_dir, platform, crates)
        return rows_dir

    def run_gate(self, args, mode: str = "aggregate"):
        out = args.get("output") or "receipt.json"
        argv = [
            mode,
            "--expected-repository", args["expected_repository"],
            "--expected-ref", args["expected_ref"],
            "--expected-commit", args["expected_commit"],
            "--expected-tree", args["expected_tree"],
            "--expected-version", args["expected_version"],
            "--expected-tag", args["expected_tag"],
            "--expected-run-id", args["expected_run_id"],
            "--expected-run-attempt", args["expected_run_attempt"],
            "--output", out,
        ]
        if mode == "aggregate":
            argv += ["--rows-dir", args["rows_dir"], "--fixture", args["fixture"], "--needs-result", args["needs_result"]]
        else:
            argv += ["--aggregate", args["aggregate"]]
        return smoke_gate.main(argv)

    def refuse_message(self, callable_and_args) -> str:
        with self.assertRaises(SystemExit) as captured, contextlib.redirect_stderr(io.StringIO()) as stderr:
            callable_and_args()
        message = getattr(captured.exception, "message", "") or stderr.getvalue()
        return message

    def test_green_aggregate_passes_and_binds_every_identity_field(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows_dir = self.green_rows(root)
            output = str(root / "aggregate.receipt.json")
            self.assertEqual(
                self.run_gate(expected_args(rows_dir=str(rows_dir), output=output)), 0
            )
            aggregate = json.loads(Path(output).read_text(encoding="utf-8"))
            self.assertEqual(aggregate["schema_id"], smoke_gate.SCHEMA_ID)
            self.assertEqual(aggregate["result"], "Passed")
            self.assertEqual(aggregate["candidate_commit"], COMMIT)
            self.assertEqual(aggregate["candidate_tree"], TREE)
            self.assertEqual(aggregate["candidate_version"], VERSION)
            self.assertEqual(aggregate["workflow_run_id"], int(RUN_ID))
            self.assertEqual(
                sorted(row["platform"] for row in aggregate["required_rows"]),
                sorted(PLATFORMS),
            )
            self.assertEqual(aggregate["matrix_authority"], "#3737")
            classifications = sorted(
                (row["platform"], row["classification"])
                for row in aggregate["not_included_rows"]
            )
            self.assertIn(("macos", "NotIncluded"), classifications)
            self.assertIn(("rust-1.85", "NotIncluded"), classifications)

    def test_green_aggregate_verifies_inside_publish(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows_dir = self.green_rows(root)
            aggregate_path = str(root / "aggregate.receipt.json")
            self.run_gate(expected_args(rows_dir=str(rows_dir), output=aggregate_path))
            verification = str(root / "verify.receipt.json")
            self.assertEqual(
                self.run_gate(
                    expected_args(aggregate=aggregate_path, output=verification), mode="verify"
                ),
                0,
            )
            receipt = json.loads(Path(verification).read_text(encoding="utf-8"))
            self.assertEqual(receipt["result"], "Passed")
            self.assertEqual(sorted(receipt["verified_platforms"]), sorted(PLATFORMS))

    def test_every_non_success_needs_result_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows_dir = self.green_rows(root)
            for needs_result in ("failure", "cancelled", "timed_out", "skipped", ""):
                with self.subTest(needs_result=needs_result):
                    message = self.refuse_message(
                        lambda: self.run_gate(
                            expected_args(rows_dir=str(rows_dir), needs_result=needs_result)
                        )
                    )
                    self.assertIn("smoke matrix did not succeed", message)

    def test_missing_rows_directory_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            message = self.refuse_message(
                lambda: self.run_gate(expected_args(rows_dir=str(Path(directory) / "absent")))
            )
            self.assertIn("no platform row was claimed", message)

    def test_missing_required_row_receipt_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows_dir = self.green_rows(root)
            (rows_dir / "windows" / "release-source-candidate-smoke-windows.receipt.json").unlink()
            message = self.refuse_message(
                lambda: self.run_gate(expected_args(rows_dir=str(rows_dir)))
            )
            self.assertIn("required platform row windows is missing its receipt", message)

    def test_unclaimed_row_result_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows_dir = self.green_rows(root)
            path = rows_dir / "linux" / "release-source-candidate-smoke-linux.receipt.json"
            row = json.loads(path.read_text(encoding="utf-8"))
            row["result"] = "Skipped"
            path.write_text(json.dumps(row), encoding="utf-8")
            message = self.refuse_message(
                lambda: self.run_gate(expected_args(rows_dir=str(rows_dir)))
            )
            self.assertIn("row result is 'Skipped'", message)

    def test_stale_head_and_wrong_version_refuse(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows_dir = self.green_rows(root)
            harness = rows_dir / "linux" / "source-candidate-smoke.receipt.json"
            document = json.loads(harness.read_text(encoding="utf-8"))
            document["candidate"]["git_head"] = "f" * 40
            harness.write_text(json.dumps(document), encoding="utf-8")
            message = self.refuse_message(
                lambda: self.run_gate(expected_args(rows_dir=str(rows_dir)))
            )
            self.assertIn("stale", message)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows_dir = self.green_rows(root)
            path = rows_dir / "windows" / "release-source-candidate-smoke-windows.receipt.json"
            row = json.loads(path.read_text(encoding="utf-8"))
            row["candidate_version"] = "0.1.11"
            path.write_text(json.dumps(row), encoding="utf-8")
            message = self.refuse_message(
                lambda: self.run_gate(expected_args(rows_dir=str(rows_dir)))
            )
            self.assertIn("binds a different candidate_version", message)

    def test_partial_journey_and_partial_package_graph_refuse(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows_dir = self.green_rows(root)
            harness = rows_dir / "linux" / "source-candidate-smoke.receipt.json"
            document = json.loads(harness.read_text(encoding="utf-8"))
            document["journey"]["steps_executed"] = document["journey"]["steps_executed"][:-1]
            harness.write_text(json.dumps(document), encoding="utf-8")
            message = self.refuse_message(
                lambda: self.run_gate(expected_args(rows_dir=str(rows_dir)))
            )
            self.assertIn("journey is partial", message)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows_dir = self.green_rows(root)
            path = rows_dir / "linux" / "release-source-candidate-smoke-linux.receipt.json"
            row = json.loads(path.read_text(encoding="utf-8"))
            row["packaged_crates"] = row["packaged_crates"][:-1]
            path.write_text(json.dumps(row), encoding="utf-8")
            message = self.refuse_message(
                lambda: self.run_gate(expected_args(rows_dir=str(rows_dir)))
            )
            self.assertIn("packaged crate graph differs", message)

    def test_missing_negative_controls_and_forged_harness_digest_refuse(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows_dir = self.green_rows(root)
            harness = rows_dir / "windows" / "source-candidate-smoke.receipt.json"
            document = json.loads(harness.read_text(encoding="utf-8"))
            document["negative_controls"] = []
            harness.write_text(json.dumps(document), encoding="utf-8")
            message = self.refuse_message(
                lambda: self.run_gate(expected_args(rows_dir=str(rows_dir)))
            )
            self.assertIn("ran no negative controls", message)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows_dir = self.green_rows(root)
            path = rows_dir / "linux" / "release-source-candidate-smoke-linux.receipt.json"
            row = json.loads(path.read_text(encoding="utf-8"))
            row["harness_receipt_sha256"] = "sha256:" + "0" * 64
            path.write_text(json.dumps(row), encoding="utf-8")
            message = self.refuse_message(
                lambda: self.run_gate(expected_args(rows_dir=str(rows_dir)))
            )
            self.assertIn("does not match the retained harness receipt bytes", message)

    def test_toolchain_suffix_is_not_a_prefix_match(self):
        for field, value in (
            ("rustc_version", "rustc 1.95.0-nightly"),
            ("cargo_version", "cargo 1.95.0-nightly"),
        ):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                rows_dir = self.green_rows(root)
                harness = rows_dir / "linux" / "source-candidate-smoke.receipt.json"
                document = json.loads(harness.read_text(encoding="utf-8"))
                document["environment"][field] = value
                harness.write_text(json.dumps(document), encoding="utf-8")
                message = self.refuse_message(
                    lambda: self.run_gate(expected_args(rows_dir=str(rows_dir)))
                )
                self.assertIn("does not prove", message)

    def test_toolchain_hash_suffix_still_passes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows_dir = self.green_rows(root)
            harness = rows_dir / "linux" / "source-candidate-smoke.receipt.json"
            document = json.loads(harness.read_text(encoding="utf-8"))
            document["environment"]["rustc_version"] = "rustc 1.95.0 (abcdef 2025-01-01)"
            document["environment"]["cargo_version"] = "cargo 1.95.0 (abcdef 2025-01-01)"
            harness.write_text(json.dumps(document), encoding="utf-8")
            row_path = rows_dir / "linux" / "release-source-candidate-smoke-linux.receipt.json"
            row = json.loads(row_path.read_text(encoding="utf-8"))
            row["harness_receipt_sha256"] = (
                "sha256:" + hashlib.sha256(harness.read_bytes()).hexdigest()
            )
            row_path.write_text(json.dumps(row), encoding="utf-8")
            self.assertEqual(
                self.run_gate(expected_args(rows_dir=str(rows_dir))), 0
            )

    def test_unexpected_platform_row_cannot_widen_the_matrix(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows_dir = self.green_rows(root)
            crates = read_fixture_crates()
            extra = rows_dir / "macos"
            extra.mkdir()
            (extra / "release-source-candidate-smoke-macos.receipt.json").write_text("{}", encoding="utf-8")
            message = self.refuse_message(
                lambda: self.run_gate(expected_args(rows_dir=str(rows_dir)))
            )
            self.assertIn("may not widen the claimed platform set", message)

    def test_verify_refuses_absent_foreign_or_non_passed_aggregates(self):
        with tempfile.TemporaryDirectory() as directory:
            message = self.refuse_message(
                lambda: self.run_gate(
                    expected_args(aggregate=str(Path(directory) / "absent.json")),
                    mode="verify",
                )
            )
            self.assertIn("aggregate is absent", message)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            aggregate_path = root / "foreign.json"
            aggregate_path.write_text(
                json.dumps({"schema_id": smoke_gate.SCHEMA_ID, "result": "Passed"}),
                encoding="utf-8",
            )
            message = self.refuse_message(
                lambda: self.run_gate(
                    expected_args(aggregate=str(aggregate_path)), mode="verify"
                )
            )
            self.assertIn("binds a different", message)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows_dir = self.green_rows(root)
            aggregate_path = str(root / "aggregate.receipt.json")
            self.run_gate(expected_args(rows_dir=str(rows_dir), output=aggregate_path))
            document = json.loads(Path(aggregate_path).read_text(encoding="utf-8"))
            document["required_rows"][0]["result"] = "Failed"
            Path(aggregate_path).write_text(json.dumps(document), encoding="utf-8")
            message = self.refuse_message(
                lambda: self.run_gate(
                    expected_args(aggregate=aggregate_path), mode="verify"
                )
            )
            self.assertIn("publication refuses", message)


if __name__ == "__main__":
    unittest.main()
