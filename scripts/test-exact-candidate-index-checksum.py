#!/usr/bin/env python3
"""Compiler-free, in-memory characterization of the consumed checksum control."""

import copy
import importlib.util
import json
import os
from pathlib import Path
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "checksum", Path(__file__).with_name("exact-candidate-index-checksum.py")
)
checksum = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checksum)


class ConsumedChecksumTests(unittest.TestCase):
    def setUp(self):
        self.archive = b"exact candidate archive bytes"
        self.hash = checksum.digest(self.archive).removeprefix("sha256:")
        self.rows = [{"cargo_package_name": name, "cargo_package_version": version,
                      "crate_digest": "sha256:" + self.hash, "index_checksum": self.hash}
                     for name, version in (("cargo-allow", "0.2.0"), ("allow-core", "0.2.0"))]
        self.root = "path+file:///candidate/cargo-allow#0.2.0"
        self.dependency = "registry+https://github.com/rust-lang/crates.io-index#allow-core@0.2.0"
        self.metadata = {
            "packages": [
                {"id": self.root, "name": "cargo-allow", "version": "0.2.0", "source": None},
                {"id": self.dependency, "name": "allow-core", "version": "0.2.0",
                 "source": "registry+https://github.com/rust-lang/crates.io-index"}],
            "resolve": {"root": self.root, "nodes": [
                {"id": self.root, "dependencies": [self.dependency]},
                {"id": self.dependency, "dependencies": []}]}}
        # Deliberately put the unused root first and an unselected version before the chosen row.
        self.files = {"index/0-root": self.line("cargo-allow", "0.2.0"),
                      "index/al/lo/allow-core": self.line("allow-core", "0.1.0") + self.line("allow-core", "0.2.0")}

    def line(self, name, version):
        return (json.dumps({"name": name, "vers": version, "cksum": self.hash,
                            "deps": [], "features": {}}) + "\n").encode()

    def selected(self):
        return checksum.select_dependency(self.metadata, self.rows)

    def test_unused_root_and_other_version_cannot_receive_mutation(self):
        original = copy.deepcopy(self.files)
        path, mutated, record = checksum.mutate_index(self.files, self.selected(), self.archive)
        self.assertEqual(path, "index/al/lo/allow-core")
        self.assertEqual(self.files, original)
        before, after = self.files[path].splitlines(), mutated.splitlines()
        self.assertEqual(before[0], after[0])
        self.assertNotEqual(before[1], after[1])
        old, new = json.loads(before[1]), json.loads(after[1])
        self.assertEqual(new.pop("cksum"), record["mutated_checksum"])
        self.assertEqual(old.pop("cksum"), record["original_checksum"])
        self.assertEqual(old, new)
        self.assertNotEqual(record["original_checksum"], record["mutated_checksum"])
        self.assertRegex(record["mutated_checksum"], r"^[0-9a-f]{64}$")

    def test_selection_is_independent_of_candidate_and_metadata_order(self):
        expected = self.selected()
        self.rows.reverse()
        self.metadata["packages"].reverse()
        self.metadata["resolve"]["nodes"].reverse()
        self.assertEqual(self.selected(), expected)

    def test_path_unreachable_or_missing_dependency_refuses(self):
        for change in ("path", "unreachable", "missing"):
            with self.subTest(change=change):
                data = copy.deepcopy(self.metadata)
                if change == "path":
                    data["packages"][1]["source"] = None
                elif change == "unreachable":
                    data["resolve"]["nodes"][0]["dependencies"] = []
                else:
                    data["packages"].pop()
                with self.assertRaises(ValueError):
                    checksum.select_dependency(data, self.rows)

    def test_duplicate_candidate_metadata_and_resolution_nodes_refuse(self):
        with self.assertRaises(ValueError):
            checksum.select_dependency(self.metadata, self.rows + [self.rows[1]])
        for key in ("packages", "nodes"):
            data = copy.deepcopy(self.metadata)
            sequence = data[key] if key == "packages" else data["resolve"][key]
            sequence.append(copy.deepcopy(sequence[-1]))
            with self.subTest(key=key), self.assertRaises(ValueError):
                checksum.select_dependency(data, self.rows)

    def test_missing_or_duplicate_exact_index_row_refuses(self):
        for files in ({"index/root": self.files["index/0-root"]},
                      {**self.files, "index/duplicate": self.line("allow-core", "0.2.0")},
                      {"index/repeated": self.line("allow-core", "0.2.0") * 2}):
            with self.subTest(files=list(files)), self.assertRaises(ValueError):
                checksum.mutate_index(files, self.selected(), self.archive)

    def test_archive_candidate_and_index_drift_refuse(self):
        with self.assertRaises(ValueError):
            checksum.mutate_index(self.files, self.selected(), self.archive + b"x")
        for key, value in (("crate_digest", "sha256:" + "0" * 64),
                           ("index_checksum", "0" * 64), ("index_checksum", "sha256:" + self.hash)):
            selected = {**self.selected(), key: value}
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                checksum.mutate_index(self.files, selected, self.archive)
        files = {"index/row": self.line("allow-core", "0.2.0").replace(self.hash.encode(), b"0" * 64)}
        with self.assertRaises(ValueError):
            checksum.mutate_index(files, self.selected(), self.archive)

    def test_checksum_refusal_requires_exact_selected_identity_and_failure(self):
        message = "error: checksum for `allow-core v0.2.0` changed between lock files\n"
        self.assertTrue(checksum.checksum_refused(101, message, self.selected()))
        self.assertTrue(checksum.checksum_refused(101, "error: failed to verify the checksum of `allow-core v0.2.0`\n", self.selected()))
        self.assertTrue(checksum.checksum_refused(101, "Caused by:\n  failed to verify the checksum of `allow-core v0.2.0`\n", self.selected()))
        self.assertTrue(checksum.checksum_refused(101, "\tchecksum for `allow-core v0.2.0` changed between lock files\n", self.selected()))
        for status, stderr in ((0, message), (True, message), (1, "missing file /tmp/index-checksum-control"),
                               (101, message.replace("allow-core", "allow-core-other")),
                               (101, message.replace("allow-core", "other-allow-core")),
                               (101, message.replace("allow-core", "other_allow-core")),
                               (101, message.replace("0.2.0", "0.2.0-rc.1")),
                               (101, "Unpacking allow-core v0.2.0 from /tmp/checksum"),
                               (101, "error: unrelated failure")):
            with self.subTest(status=status, stderr=stderr):
                self.assertFalse(checksum.checksum_refused(status, stderr, self.selected()))

    def test_recorded_metadata_command_overrides_forced_parent_color(self):
        with patch.dict(os.environ, {"CARGO_TERM_COLOR": "always"}):
            argv = checksum.metadata_command(Path("candidate/Cargo.toml"), Path("registry"))
        self.assertEqual(argv[:4], ["cargo", "--color", "never", "metadata"])
        self.assertIn("--offline", argv)
        self.assertIn("--locked", argv)
        self.assertEqual(argv[argv.index("--manifest-path") + 1], str(Path("candidate/Cargo.toml")))
        self.assertEqual(argv[-1], 'source.candidate-local-registry.local-registry="registry"')


if __name__ == "__main__":
    unittest.main()
