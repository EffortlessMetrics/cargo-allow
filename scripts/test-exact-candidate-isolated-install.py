#!/usr/bin/env python3
"""Offline characterization for the isolated-install resolved-graph
comparison and receipt classification (#2925). The hosted stage runs the
decisive mutations against real registries; this suite pins the pure
decision logic so regressions surface without a toolchain."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "isolated", ROOT / "scripts/exact-candidate-isolated-install.py"
)
isolated = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(isolated)

CANDIDATE_ROWS = [
    {"cargo_package_name": "cargo-allow", "cargo_package_version": "0.2.0-rc.1"},
    {"cargo_package_name": "allow-core", "cargo_package_version": "0.2.0-rc.1"},
    {"cargo_package_name": "effortless-repo-protocol", "cargo_package_version": "0.1.0"},
]


def metadata(packages):
    return {
        "packages": [
            {
                "name": name,
                "version": version,
                "manifest_path": f"/checkout/target/package/{name}-{version}/Cargo.toml",
            }
            for name, version in packages
        ],
        "workspace_members": [],
    }


class IsolatedInstallComparisonTests(unittest.TestCase):
    def test_clean_resolution_matches_every_candidate_row(self):
        packages = [(row["cargo_package_name"], row["cargo_package_version"]) for row in CANDIDATE_ROWS]
        comparison = isolated.compare_resolution(metadata(packages), CANDIDATE_ROWS)
        self.assertEqual(comparison["matched_packages"], 3)
        self.assertTrue(
            not comparison["unexpected_packages"]
            and not comparison["missing_packages"]
            and not comparison["version_mismatches"]
            and not comparison["path_sources"]
        )

    def test_compatible_but_unselected_internal_version_is_rejected(self):
        packages = [(row["cargo_package_name"], row["cargo_package_version"]) for row in CANDIDATE_ROWS]
        packages[1] = ("allow-core", "0.2.0-rc.2")
        comparison = isolated.compare_resolution(metadata(packages), CANDIDATE_ROWS)
        self.assertEqual(len(comparison["version_mismatches"]), 1)

    def test_unselected_sibling_package_is_rejected(self):
        packages = [(row["cargo_package_name"], row["cargo_package_version"]) for row in CANDIDATE_ROWS]
        packages.append(("intent-model", "0.1.0"))
        comparison = isolated.compare_resolution(metadata(packages), CANDIDATE_ROWS)
        self.assertEqual(comparison["unexpected_packages"], ["intent-model"])

    def test_wrong_shared_version_is_rejected(self):
        packages = [(row["cargo_package_name"], row["cargo_package_version"]) for row in CANDIDATE_ROWS]
        packages[2] = ("effortless-repo-protocol", "0.1.1")
        comparison = isolated.compare_resolution(metadata(packages), CANDIDATE_ROWS)
        self.assertEqual(len(comparison["version_mismatches"]), 1)

    def test_workspace_path_source_is_detected(self):
        data = metadata([(row["cargo_package_name"], row["cargo_package_version"]) for row in CANDIDATE_ROWS])
        data["packages"][0]["manifest_path"] = (
            "/checkout/crates/cargo-allow/Cargo.toml"
        )
        comparison = isolated.compare_resolution(data, CANDIDATE_ROWS)
        self.assertEqual(comparison["path_sources"], ["cargo-allow"])

    def test_missing_selected_package_is_detected(self):
        data = metadata([(row["cargo_package_name"], row["cargo_package_version"]) for row in CANDIDATE_ROWS[1:]])
        comparison = isolated.compare_resolution(data, CANDIDATE_ROWS)
        self.assertEqual(len(comparison["missing_packages"]), 1)


class IsolatedInstallClassificationTests(unittest.TestCase):
    def clean_payload(self):
        return {
            "schema_id": isolated.SCHEMA_ID,
            "schema_version": isolated.SCHEMA_VERSION,
            "source_checkout_denied": True,
            "candidate_artifact_digest": "sha256:" + "1" * 64,
            "cargo_lock_digest": "sha256:" + "2" * 64,
            "registry_index_digest": "sha256:" + "3" * 64,
            "installed_executable_digest": "sha256:" + "4" * 64,
            "external_cache_identity": "sha256:" + "5" * 64,
            "graph_comparison": {"expected_packages": 3, "matched_packages": 3},
        }

    def test_complete_classification(self):
        self.assertEqual(isolated.classify(self.clean_payload()), "Complete")

    def test_stale_identity_classification(self):
        payload = self.clean_payload()
        payload["cargo_lock_digest"] = "stale"
        self.assertEqual(isolated.classify(payload), "StaleInput")

    def test_redaction_classification(self):
        payload = self.clean_payload()
        payload["external_cache_identity"] = "/home/runner/work/private-cache"
        self.assertEqual(isolated.classify(payload), "PathLeakInReceipt")

    def test_source_fallback_classification(self):
        payload = self.clean_payload()
        payload["source_checkout_denied"] = False
        self.assertEqual(isolated.classify(payload), "SourceFallbackDetected")


class IsolatedInstallReceiptTests(unittest.TestCase):
    def produce(self, checksum):
        row = {"cargo_package_name": "cargo-allow", "cargo_package_version": "0.2.0",
               "crate_digest": "sha256:" + "a" * 64, "index_checksum": checksum}
        with tempfile.TemporaryDirectory() as directory:
            artifact = Path(directory) / "candidate.json"
            artifact.write_text("{}", encoding="utf-8")
            return isolated.receipt_payload(
                artifact, [row], {"expected_packages": 1, "matched_packages": 1},
                commit="1" * 40, tree="2" * 40, cargo_lock_digest="sha256:" + "b" * 64,
                registry_index_digest="sha256:" + "c" * 64, external_cache_identity="lock-scoped",
                source_checkout_denied=True, install_root_identity="sha256:" + "d" * 64,
                cargo_home_identity="sha256:" + "e" * 64, installed_executable_digest="sha256:" + "f" * 64,
                installed_version_output="cargo-allow 0.2.0", platform="x86_64-unknown-linux-gnu", toolchain="1.95.0")

    def test_raw_registry_checksum_becomes_a_portable_receipt_identity(self):
        receipt = self.produce("a" * 64)
        row = receipt["package_rows"][0]
        self.assertEqual(row["index_checksum"], "sha256:" + "a" * 64)
        self.assertEqual(row["index_checksum"], row["crate_digest"])

    def test_malformed_or_already_prefixed_registry_checksums_refuse(self):
        for value in ("", "a" * 63, "g" * 64, "sha256:" + "a" * 64, None):
            with self.subTest(value=value), self.assertRaises(ValueError):
                self.produce(value)


if __name__ == "__main__":
    unittest.main()
