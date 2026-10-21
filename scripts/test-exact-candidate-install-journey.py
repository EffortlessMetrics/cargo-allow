#!/usr/bin/env python3
"""Offline false-success and cleanup-order controls; never invokes Cargo."""

from __future__ import annotations

import argparse
import copy
import importlib.util
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("exact_journey", ROOT / "scripts/exact-candidate-install-journey.py")
WRAPPER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(WRAPPER)
VALIDATOR = WRAPPER.VALIDATOR
HEAD = "a" * 40


class ReceiptTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="exact-journey-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        schemas = ROOT / "docs/dogfood/fixtures/release"
        self.args = argparse.Namespace(
            receipt=self.root / "final.json", schema=schemas / "exact-candidate-install-journey.v1.schema.json",
            fixture=schemas / "candidate-crate-set.toml", topology=ROOT / "policy/product-package-topology-v2.toml",
            package_receipt=self.root / "package.json", package_schema=schemas / "exact-candidate-package-set.v1.schema.json",
            journey_receipt=self.root / "source.json", journey_schema=schemas / "source-candidate-smoke-receipt.v1.schema.json",
            binary=self.root / "cargo-allow", current_head=HEAD,
        )
        self.args.binary.write_bytes(b"fixture installed binary\n")
        examples = ROOT / "docs/dogfood/receipts"
        self.package = VALIDATOR.load_json(examples / "exact-candidate-package-set-pass.example.json")
        self.source = VALIDATOR.load_json(examples / "source-candidate-smoke-pass.example.json")
        self.package["candidate"]["git_head"] = HEAD
        self.source["candidate"].update(git_head=HEAD, workspace_version="0.2.0", install_method="prebuilt_override")
        self.source["installed_binary"]["version_output"] = "cargo-allow 0.2.0"
        self.write_inputs()

    def write_inputs(self):
        self.args.package_receipt.write_text(json.dumps(self.package), encoding="utf-8")
        self.args.journey_receipt.write_text(json.dumps(self.source), encoding="utf-8")

    def package_valid(self):
        return VALIDATOR.validate_package_receipt(self.args.package_receipt, self.args.package_schema,
                                                  self.args.fixture, self.args.topology)

    def source_valid(self):
        return VALIDATOR.validate_journey_receipt(self.args.journey_receipt, self.args.journey_schema)

    def test_valid_complete_binding(self):
        VALIDATOR.publish_receipt(self.package_valid(), self.source_valid(), self.args)
        final = VALIDATOR.validate_final_receipt(self.args.receipt, self.args.schema, self.args.fixture)
        VALIDATOR.validate_binding(self.package, self.source, final, self.args)
        self.assertNotIn("no_undeclared_source_reads", final["install"])

    def test_package_mutations(self):
        mutations = {
            "duplicate-row": lambda p: p["package_set"]["crates"].__setitem__(0, copy.deepcopy(p["package_set"]["crates"][1])),
            "wrong-version": lambda p: p["package_set"]["crates"][0].__setitem__("version", "9.9.9"),
            "wrong-filename": lambda p: p["package_set"]["crates"][0].__setitem__("crate_file", "other.crate"),
            "path-dependency": lambda p: p["package_set"]["crates"][0].__setitem__("no_path_deps", False),
            "failed-install": lambda p: p["install"].__setitem__("exit_code", 99),
            "wrong-version-output": lambda p: p["install"].__setitem__("version_output", "cargo-allow 9.9.9"),
            "missing-digest": lambda p: p["install"].pop("binary_sha256"),
            "null-head": lambda p: p["candidate"].__setitem__("git_head", None),
            "duplicate-negative": lambda p: p["negative_controls"].append(copy.deepcopy(p["negative_controls"][0])),
            "workspace-present": lambda p: p["isolation"].__setitem__("workspace_paths_absent", False),
        }
        original = copy.deepcopy(self.package)
        for label, mutate in mutations.items():
            with self.subTest(label=label):
                self.package = copy.deepcopy(original)
                mutate(self.package)
                self.write_inputs()
                with self.assertRaises(SystemExit):
                    self.package_valid()

    def test_source_mutations(self):
        mutations = {
            "no-negatives": lambda s: s.__setitem__("negative_controls", []),
            "duplicate-negative": lambda s: s["negative_controls"].append(copy.deepcopy(s["negative_controls"][0])),
            "wrong-negative-class": lambda s: s["negative_controls"][0].__setitem__("result_class", "Passed"),
            "omitted-step": lambda s: s["journey"]["steps_executed"].pop(),
            "omitted-declared-step": lambda s: s["journey"]["steps_expected"].pop(),
            "duplicate-step": lambda s: s["journey"]["steps_executed"].__setitem__(0, copy.deepcopy(s["journey"]["steps_executed"][1])),
            "duplicate-expected": lambda s: s["journey"]["steps_expected"].append("version"),
            "failed-step": lambda s: s["journey"]["steps_executed"][0].__setitem__("exit_code", 99),
            "boolean-exit": lambda s: s["journey"]["steps_executed"][0].__setitem__("exit_code", False),
            "wrong-artifact": lambda s: s["journey"]["steps_executed"][1].__setitem__("artifact_schema_id", "cargo-allow.report.v1"),
        }
        original = copy.deepcopy(self.source)
        for label, mutate in mutations.items():
            with self.subTest(label=label):
                self.source = copy.deepcopy(original)
                mutate(self.source)
                self.write_inputs()
                with self.assertRaises(SystemExit):
                    self.source_valid()

    def test_cross_binding_mutations_never_publish(self):
        mutations = {
            "package-head": lambda: self.package["candidate"].__setitem__("git_head", "b" * 40),
            "source-head": lambda: self.source["candidate"].__setitem__("git_head", "b" * 40),
            "binary-bytes": lambda: self.args.binary.write_bytes(b"changed"),
            "source-binary": lambda: self.source["installed_binary"].__setitem__("binary_sha256", "f" * 64),
            "wrong-method": lambda: self.source["candidate"].__setitem__("install_method", "cargo_install_path"),
            "source-version": lambda: self.source["installed_binary"].__setitem__("version_output", "cargo-allow 9.9.9"),
        }
        original_package, original_source = copy.deepcopy(self.package), copy.deepcopy(self.source)
        for label, mutate in mutations.items():
            with self.subTest(label=label):
                self.package, self.source = copy.deepcopy(original_package), copy.deepcopy(original_source)
                self.args.binary.write_bytes(b"fixture installed binary\n")
                mutate()
                self.write_inputs()
                with self.assertRaises(SystemExit):
                    VALIDATOR.publish_receipt(self.package_valid(), self.source_valid(), self.args)
                self.assertFalse(self.args.receipt.exists())

    def test_final_binding_detects_projection_and_input_changes(self):
        final = VALIDATOR.assemble_receipt(self.package, self.source, self.args)
        for key in ("package_set_receipt_sha256", "journey_receipt_sha256", "candidate_topology_sha256", "installed_binary_sha256"):
            with self.subTest(key=key):
                forged = copy.deepcopy(final)
                forged["provenance"][key] = "f" * 64
                with self.assertRaises(SystemExit):
                    VALIDATOR.validate_binding(self.package, self.source, forged, self.args)
        forged = copy.deepcopy(final)
        forged["journey"]["steps_executed"].reverse()
        with self.assertRaises(SystemExit):
            VALIDATOR.validate_binding(self.package, self.source, forged, self.args)

    def test_input_change_after_validation_cannot_be_digest_laundered(self):
        self.package_valid()
        self.source_valid()
        changed = copy.deepcopy(self.package)
        changed["candidate"]["git_head"] = "b" * 40
        self.args.package_receipt.write_text(json.dumps(changed), encoding="utf-8")
        with self.assertRaisesRegex(SystemExit, "input receipt changed"):
            VALIDATOR.publish_receipt(self.package, self.source, self.args)
        self.assertFalse(self.args.receipt.exists())

    def test_final_duplicates_and_boolean_success_are_rejected(self):
        final = VALIDATOR.assemble_receipt(self.package, self.source, self.args)
        mutations = (
            lambda f: f["negative_controls"].append(copy.deepcopy(f["negative_controls"][0])),
            lambda f: f["journey"]["steps_executed"].__setitem__(0, copy.deepcopy(f["journey"]["steps_executed"][1])),
            lambda f: f["journey"]["steps_executed"][0].__setitem__("exit_code", False),
        )
        for mutate in mutations:
            forged = copy.deepcopy(final)
            mutate(forged)
            self.args.receipt.write_text(json.dumps(forged), encoding="utf-8")
            with self.assertRaises(SystemExit):
                VALIDATOR.validate_final_receipt(self.args.receipt, self.args.schema, self.args.fixture)

    def test_schema_failure_never_publishes_or_leaves_pending_output(self):
        self.args.schema = self.root / "reject-schema.json"
        self.args.schema.write_text('{"const":"reject"}', encoding="utf-8")
        with self.assertRaises(SystemExit):
            VALIDATOR.publish_receipt(self.package, self.source, self.args)
        self.assertFalse(self.args.receipt.exists())
        self.assertEqual(list(self.root.glob(".pending-receipt-*")), [])

    def wrapper_fixture(self):
        repo = self.root / "repo"
        repo.mkdir()
        for relative in ("docs/dogfood/fixtures/release", "policy"):
            (repo / relative).mkdir(parents=True, exist_ok=True)
        for source in (self.args.schema, self.args.fixture, self.args.package_schema, self.args.journey_schema):
            shutil.copyfile(source, repo / "docs/dogfood/fixtures/release" / source.name)
        shutil.copyfile(self.args.topology, repo / "policy" / self.args.topology.name)
        package_dir = repo / "target/exact-candidate-package-set"
        (package_dir / "install/bin").mkdir(parents=True)
        shutil.copyfile(self.args.binary, package_dir / "install/bin/cargo-allow")
        shutil.copyfile(self.args.package_receipt, package_dir / "exact-candidate-package-set.receipt.json")
        work = repo / "target/exact-candidate-install-journey"
        return repo, work

    def test_wrapper_cleanup_order_and_refusals(self):
        for mode in ("success", "child-failure", "leftover-child", "cleanup-refusal"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory(dir=self.root) as case:
                previous = self.root
                self.root = Path(case)
                repo, work = self.wrapper_fixture()
                self.root = previous
                final = work / "exact-candidate-install-journey.receipt.json"
                calls = []

                def child(*args, **kwargs):
                    parent = Path(kwargs["env"]["WORK_DIR"])
                    (parent / "source-candidate-smoke.receipt.json").write_text(json.dumps(self.source), encoding="utf-8")
                    if mode in ("child-failure", "leftover-child"):
                        (parent / "retained-child").mkdir()
                    if mode == "child-failure":
                        raise subprocess.CalledProcessError(73, "source journey")

                def remove(root, directory, purpose, token):
                    calls.append(directory)
                    self.assertFalse(final.exists())
                    self.assertTrue((work / "source-candidate-smoke.receipt.json").is_file())
                    self.assertEqual(WRAPPER.LIFECYCLE.load_marker(directory)["token"], token)
                    if mode == "cleanup-refusal":
                        raise SystemExit("injected cleanup refusal")
                    # A nonrecursive fake for the dependency boundary; real
                    # lifecycle deletion has its own platform/symlink controls.
                    for leaf in directory.iterdir():
                        leaf.unlink()
                    directory.rmdir()

                with mock.patch.dict(os.environ, {}, clear=True), \
                     mock.patch.object(WRAPPER.LIFECYCLE, "run_git", return_value=SimpleNamespace(stdout=HEAD)), \
                     mock.patch.object(WRAPPER.subprocess, "run", side_effect=child), \
                     mock.patch.object(WRAPPER.LIFECYCLE, "remove", side_effect=remove):
                    if mode == "success":
                        self.assertEqual(WRAPPER.run(repo), final)
                        self.assertTrue(final.is_file())
                        self.assertEqual(len(calls), 1)
                    else:
                        with self.assertRaises((SystemExit, subprocess.CalledProcessError)):
                            WRAPPER.run(repo)
                        self.assertFalse(final.exists())
                        self.assertEqual(len(calls), 1 if mode == "cleanup-refusal" else 0)
                        self.assertEqual(len(list(work.glob("exact-candidate-source-journey.*"))), 1)

    def test_wrapper_preserves_preexisting_output_and_consumer(self):
        repo, work = self.wrapper_fixture()
        consumer = self.root / "consumer"
        consumer.mkdir()
        sentinel = consumer / "caller-file"
        sentinel.write_bytes(b"preserve")
        with mock.patch.dict(os.environ, {"CONSUMER_DIR": str(consumer)}, clear=True):
            with self.assertRaises(SystemExit):
                WRAPPER.run(repo)
        self.assertEqual(sentinel.read_bytes(), b"preserve")
        self.assertFalse(work.exists())
        work.mkdir()
        (work / "caller-file").write_bytes(b"preserve")
        with mock.patch.dict(os.environ, {}, clear=True), self.assertRaises(SystemExit):
            WRAPPER.run(repo)
        self.assertEqual((work / "caller-file").read_bytes(), b"preserve")

    def test_wrapper_refuses_symlinked_output(self):
        repo, work = self.wrapper_fixture()
        outside = self.root / "outside"
        outside.mkdir()
        sentinel = outside / "sentinel"
        sentinel.write_bytes(b"preserve")
        try:
            work.symlink_to(outside, target_is_directory=True)
        except OSError as error:
            self.skipTest(f"host cannot create symlink fixture: {error}")
        with mock.patch.dict(os.environ, {}, clear=True), self.assertRaises(SystemExit):
            WRAPPER.run(repo)
        self.assertEqual(sentinel.read_bytes(), b"preserve")

    def test_actual_source_cleanup_propagates_refusal_without_fallback(self):
        bash = shutil.which("bash")
        if os.name == "nt":
            git_bash = Path(os.environ.get("ProgramFiles", "C:/Program Files")) / "Git/bin/bash.exe"
            if git_bash.is_file():
                bash = str(git_bash)
        if not bash:
            self.skipTest("bash unavailable")
        script = (ROOT / "scripts/source-candidate-smoke.sh").read_text(encoding="utf-8")
        cleanup = "cleanup() {" + script.split("\ncleanup() {", 1)[1].split("\ntrap cleanup EXIT", 1)[0]
        for failure in (0, 73):
            log = self.root / f"cleanup-{failure}.log"
            program = (
                "set -euo pipefail\n"
                f"log='{log.as_posix()}'\n"
                "lifecycle=unused consumer_parent=unused consumer_dir=unused consumer_token=unused\n"
                "source_parent=unused work_dir=unused work_token=unused\n"
                "restore_source_tree() { return 0; }\n"
                f"python3() {{ printf '%s\\n' \"$*\" >> \"$log\"; return {failure}; }}\n"
                + cleanup + "\ntrap cleanup EXIT\nexit 0\n"
            )
            result = subprocess.run([bash, "-c", program], capture_output=True, text=True)
            self.assertEqual(result.returncode, failure, result.stderr)
            attempts = log.read_text().splitlines()
            self.assertEqual(len(attempts), 1 if failure else 2)
            self.assertIn("source-candidate-consumer", attempts[0])


if __name__ == "__main__":
    unittest.main()
