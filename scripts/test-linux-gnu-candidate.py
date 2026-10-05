#!/usr/bin/env python3
"""Bounded ABI and predecessor falsifiers; no compiler or candidate execution."""
import copy
import importlib.util
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("candidate", Path(__file__).with_name("linux-gnu-candidate.py"))
candidate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(candidate)

NEEDS = """Version symbols section '.gnu.version' contains 3 entries:
  000: 0 (*local*) 2 (GLIBC_2.35) 3 (GLIBC_2.9)

Version needs section '.gnu.version_r' contains 1 entry:
 Addr: 0x0000000000000818 Offset: 0x000818 Link: 7 (.dynstr)
  000000: Version: 1  File: libc.so.6  Cnt: 3
  0x0010: Name: GLIBC_2.2.5  Flags: none  Version: 2
  0x0020: Name: GLIBC_2.35  Flags: none  Version: 3
  0x0030: Name: GLIBC_2.9  Flags: none  Version: 4
"""


def elf() -> bytes:
    data = bytearray(64)
    data[:7] = b"\x7fELF\x02\x01\x01"
    struct.pack_into("<HHI", data, 16, 3, 62, 1)
    return bytes(data)


def predecessors():
    common = {"schema_version": 2, "repository_commit": "1" * 40, "repository_tree": "2" * 40}
    package = {**common, "schema_id": "cargo-allow.package-candidate.v2", "root_package_version": "0.2.0",
               "cargo_lock_digest": "sha256:" + "a" * 64,
               "rows": [{"logical_id": "cargo-allow", "cargo_package_name": "cargo-allow",
                         "cargo_package_version": "0.2.0", "crate_digest": "sha256:" + "a" * 64,
                         "crate_size_bytes": 123}]}
    install = {**common, "schema_id": "cargo-allow.isolated-install.v2",
               "candidate_artifact_digest": "sha256:" + "d" * 64, "installed_executable_digest": "sha256:" + "f" * 64,
               "installed_version_output": "cargo-allow 0.2.0", "platform": candidate.TARGET,
               "source_checkout_denied": True, "graph_comparison": {"expected_packages": 1, "matched_packages": 1},
               "cargo_lock_digest": "sha256:" + "b" * 64, "registry_index_digest": "sha256:" + "3" * 64,
               "external_cache_identity": "lock-scoped", "install_root_identity": "sha256:" + "4" * 64,
               "cargo_home_identity": "sha256:" + "5" * 64, "toolchain": "1.95.0",
               "claim_boundary": "isolated candidate only", "limitations": [],
               "package_rows": [{"package_name": "cargo-allow", "package_version": "0.2.0",
                                 "crate_digest": "sha256:" + "a" * 64,
                                 "index_checksum": "sha256:" + "a" * 64, "resolved_version": "0.2.0"}]}
    qualification = {**common, "schema_id": "cargo-allow.exact-candidate.v2",
                     "candidate_artifact_digest": "sha256:" + "d" * 64, "isolated_install_receipt_digest": "sha256:" + "e" * 64,
                     "installed_executable_digest": "sha256:" + "f" * 64, "installed_version_output": "cargo-allow 0.2.0",
                     "platform": candidate.TARGET, "journey_steps": [{"id": "finding-to-green", "exit_code": 0}],
                     "scanner_completeness": "complete", "cargo_lock_digest": "sha256:" + "c" * 64,
                     "toolchain": "1.95.0", "support_matrix_generation": "current", "diff_base_identity": "baseline",
                     "claim_boundary": "unpublished candidate only", "limitations": [], "not_included": [],
                     "artifact_schema_results": ["cargo-allow.report.v1: ok"],
                     "package_rows": [{"logical_id": "cargo-allow", "package_name": "cargo-allow",
                                       "package_version": "0.2.0", "crate_digest": "sha256:" + "a" * 64}]}
    return [package, install, qualification]


class AbiTests(unittest.TestCase):
    def test_only_needs_are_inspected_with_numeric_ordering(self):
        definitions = "Version definition section '.gnu.version_d' contains 1 entry:\n  Name: GLIBC_99.0 Flags: none\n"
        self.assertEqual(candidate.glibc_needs(definitions + NEEDS), ["2.2.5", "2.9", "2.35"])
        self.assertEqual(candidate.version("2.35.0"), candidate.version("2.35"))

    def test_absent_malformed_and_unknown_requirements_refuse(self):
        for text in ("", "No version information found in this file.", NEEDS + NEEDS,
                     NEEDS.replace("Cnt: 3", "Cnt: 4"), NEEDS.replace("1 entry:", "2 entries:"),
                     NEEDS.replace("GLIBC_2.35", "GLIBC_PRIVATE"),
                     NEEDS.replace("GLIBC_2.35", "GLIBC_ABI_DT_RELR"),
                     NEEDS.replace("GLIBC_", "GCC_")):
            with self.subTest(text=text[:60]), self.assertRaises(ValueError):
                candidate.glibc_needs(text)

    def test_real_file_inspection_accepts_boundary_and_refuses_higher_requirement(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "cargo-allow"
            binary.write_bytes(elf())
            for text, accepted in ((NEEDS, True), (NEEDS.replace("2.35", "2.36"), False)):
                result = subprocess.CompletedProcess([], 0, text, "")
                with mock.patch.object(candidate.subprocess, "run", return_value=result) as readelf:
                    if accepted:
                        inspected = candidate.inspect_abi(binary, "2.35")
                        self.assertEqual(inspected["executable_sha256"], candidate.digest_bytes(elf()))
                        self.assertEqual(inspected["required_glibc"][-1], "2.35")
                    else:
                        with self.assertRaisesRegex(ValueError, "above the GLIBC_2.35 baseline"):
                            candidate.inspect_abi(binary, "2.35")
                    self.assertEqual(readelf.call_args.args[0][0:3], ["readelf", "--wide", "--version-info"])

    def test_wrong_architecture_and_non_elf_refuse_without_execution(self):
        arm = bytearray(elf())
        struct.pack_into("<H", arm, 18, 183)
        elf32 = bytearray(elf())
        elf32[4] = 1
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "cargo-allow"
            for data in (b"#!/bin/sh\nexit 0\n", bytes(arm), bytes(elf32)):
                binary.write_bytes(data)
                with mock.patch.object(candidate.subprocess, "run") as execution:
                    with self.assertRaises(ValueError):
                        candidate.inspect_abi(binary, "2.35")
                    execution.assert_not_called()

    def test_inspection_failure_and_binary_drift_refuse(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "cargo-allow"
            binary.write_bytes(elf())
            for result in (subprocess.CompletedProcess([], 1, NEEDS, ""),
                           subprocess.CompletedProcess([], 0, NEEDS, "readelf: warning")):
                with mock.patch.object(candidate.subprocess, "run", return_value=result):
                    with self.assertRaisesRegex(ValueError, "inspection failed"):
                        candidate.inspect_abi(binary, "2.35")
            with mock.patch.object(candidate.subprocess, "run", side_effect=FileNotFoundError("readelf")):
                with self.assertRaises(OSError):
                    candidate.inspect_abi(binary, "2.35")
            def replace_during_inspection(*args, **kwargs):
                binary.write_bytes(elf() + b"replacement")
                return subprocess.CompletedProcess([], 0, NEEDS, "")
            with mock.patch.object(candidate.subprocess, "run", side_effect=replace_during_inspection):
                with self.assertRaisesRegex(ValueError, "changed during ABI inspection"):
                    candidate.inspect_abi(binary, "2.35")


class BindingTests(unittest.TestCase):
    def validate(self, payloads):
        return candidate.validate_predecessors(payloads, ["sha256:" + item * 64 for item in "de0"],
                                               "1" * 40, "2" * 40, "sha256:" + "f" * 64)

    def test_bound_predecessors_preserve_distinct_lock_digest_meanings(self):
        self.assertEqual(self.validate(predecessors()), "0.2.0")

    def test_each_stale_generation_source_or_digest_refuses(self):
        for index in range(3):
            for field in ("schema_id", "schema_version", "repository_commit", "repository_tree"):
                data = predecessors()
                data[index][field] = "wrong"
                with self.subTest(index=index, field=field), self.assertRaises(ValueError):
                    self.validate(data)
        for index, field in ((1, "candidate_artifact_digest"), (2, "candidate_artifact_digest"),
                             (2, "isolated_install_receipt_digest"), (1, "installed_executable_digest"),
                             (2, "installed_executable_digest"), (1, "installed_version_output"),
                             (2, "platform")):
            data = predecessors()
            data[index][field] = "wrong"
            with self.subTest(index=index, field=field), self.assertRaises(ValueError):
                self.validate(data)

    def test_qualification_structural_gaps_refuse(self):
        mutations = [("artifact_schema_results", ["cargo-allow.check-receipt.v2: failed"]),
                     ("package_rows", []), ("cargo_lock_digest", ""),
                     ("cargo_lock_digest", "sha256:" + "z" * 64),
                     ("toolchain", " "), ("support_matrix_generation", ""),
                     ("diff_base_identity", ""), ("claim_boundary", ""),
                     ("journey_steps", [{"id": " ", "exit_code": 0}])]
        for field, value in mutations:
            data = predecessors()
            data[2][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                self.validate(data)

    def test_qualification_package_identity_and_portable_fields_refuse(self):
        for field, value in (("logical_id", ""), ("package_name", " "), ("crate_digest", "short")):
            data = predecessors()
            data[2]["package_rows"][0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                self.validate(data)
        data = predecessors()
        data[2]["package_rows"] *= 2
        with self.assertRaises(ValueError):
            self.validate(data)
        for path in ("/home/runner/secret", "/Users/local", "C:\\work", "/runner/work/project",
                     "/cargo-allow/crates/private"):
            for field in ("claim_boundary", "artifact_schema_results", "not_included"):
                data = predecessors()
                data[2][field] = path if field == "claim_boundary" else [path + ": ok"]
                with self.subTest(field=field, path=path), self.assertRaises(ValueError):
                    self.validate(data)

    def test_malformed_candidate_bytes_and_numeric_types_refuse(self):
        for field, value in (("crate_digest", "bad"), ("crate_size_bytes", 0),
                             ("crate_size_bytes", True), ("crate_size_bytes", 123.0)):
            data = predecessors()
            data[0]["rows"][0][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                self.validate(data)
        for index in range(3):
            data = predecessors()
            data[index]["schema_version"] = 2.0
            with self.subTest(index=index), self.assertRaises(ValueError):
                self.validate(data)
        for value in (True, 1.0):
            data = predecessors()
            data[1]["graph_comparison"] = {"expected_packages": value, "matched_packages": value}
            with self.subTest(value=value), self.assertRaises(ValueError):
                self.validate(data)

    def test_existing_candidate_derivation_is_read_only_and_refuses_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            artifact = Path(directory) / "candidate.json"
            subprocess.run([sys.executable, "scripts/exact-candidate-package-candidate.py", "--mode", "derive",
                            "--output", str(artifact)], cwd=candidate.ROOT, check=True, capture_output=True)
            original = artifact.read_bytes()
            candidate.validate_candidate_derivation(artifact)
            self.assertEqual(artifact.read_bytes(), original)
            for field, value in (("rows", []), ("topology_id", ""), ("cargo_lock_digest", "short")):
                data = json.loads(original)
                data[field] = value
                changed = json.dumps(data).encode()
                artifact.write_bytes(changed)
                with self.subTest(field=field), self.assertRaises(subprocess.CalledProcessError):
                    candidate.validate_candidate_derivation(artifact)
                self.assertEqual(artifact.read_bytes(), changed)
    def test_non_complete_predecessors_refuse(self):
        mutations = [(1, "source_checkout_denied", False),
                     (1, "graph_comparison", {"expected_packages": 0, "matched_packages": 0}),
                     (1, "graph_comparison", {"expected_packages": 13, "matched_packages": 12}),
                     (1, "graph_comparison", {"expected_packages": 13, "matched_packages": 13, "path_sources": ["allow-core"]}),
                     (2, "journey_steps", []), (2, "journey_steps", [{"exit_code": 1}]),
                     (2, "journey_steps", [{"exit_code": False}]), (2, "scanner_completeness", "partial")]
        for index, field, value in mutations:
            data = predecessors()
            data[index][field] = copy.deepcopy(value)
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                self.validate(data)

    def test_install_structural_gaps_refuse(self):
        for field, value in (("package_rows", []), ("toolchain", ""), ("cargo_lock_digest", "short"),
                             ("registry_index_digest", ""), ("external_cache_identity", "/home/local"),
                             ("install_root_identity", "path"), ("cargo_home_identity", ""),
                             ("claim_boundary", " ")):
            data = predecessors()
            data[1][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                self.validate(data)
        for field, value in (("package_name", ""), ("package_version", ""), ("crate_digest", "bad"),
                             ("index_checksum", "bad"), ("resolved_version", " ")):
            data = predecessors()
            data[1]["package_rows"][0][field] = value
            with self.subTest(row_field=field), self.assertRaises(ValueError):
                self.validate(data)
        data = predecessors()
        data[1]["package_rows"] *= 2
        with self.assertRaises(ValueError):
            self.validate(data)

    def test_cross_receipt_package_rows_and_counts_must_match(self):
        for index, rows_key, field in ((0, "rows", "crate_digest"), (1, "package_rows", "package_version"),
                                      (1, "package_rows", "index_checksum"), (2, "package_rows", "logical_id")):
            data = predecessors()
            data[index][rows_key][0][field] = "sha256:" + "9" * 64
            with self.subTest(index=index, field=field), self.assertRaises(ValueError):
                self.validate(data)
        data = predecessors()
        data[1]["graph_comparison"] = {"expected_packages": 2, "matched_packages": 2}
        with self.assertRaises(ValueError):
            self.validate(data)


if __name__ == "__main__":
    unittest.main()
