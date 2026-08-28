#!/usr/bin/env python3
"""Negative controls for the package-candidate producer (#2924)."""

from __future__ import annotations

import importlib.util
import io
import json
import tarfile
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "candidate", ROOT / "scripts/exact-candidate-package-candidate.py"
)
candidate = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(candidate)


class CandidateProducerTests(unittest.TestCase):
    def real_topology(self) -> Path:
        return ROOT / "policy/product-package-topology-v2.toml"

    def real_identities(self) -> Path:
        return ROOT / "policy/product-crates-v2.toml"

    def test_derivation_is_deterministic(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            first = candidate.derive_payload(
                ROOT, self.real_topology(), self.real_identities()
            )
            (root / "Cargo.lock").write_bytes((ROOT / "Cargo.lock").read_bytes())
            second = candidate.derive_payload(
                ROOT, self.real_topology(), self.real_identities()
            )
            self.assertEqual(json.dumps(first, sort_keys=True), json.dumps(second, sort_keys=True))

    def test_derivation_selects_exactly_thirteen_mixed_version_rows(self):
        payload = candidate.derive_payload(ROOT, self.real_topology(), self.real_identities())
        self.assertEqual(len(payload["rows"]), 13)
        families = {row["product_family"] for row in payload["rows"]}
        self.assertEqual(families, {"cargo-allow-0.2", "shared-0.1"})
        orders = [row["release_order"] for row in payload["rows"]]
        self.assertEqual(orders, sorted(orders))
        self.assertEqual(payload["root_package_name"], "cargo-allow")

    def test_topology_version_disagreement_fails_derivation(self):
        with tempfile.TemporaryDirectory() as directory:
            topology_path = Path(directory) / "topology.toml"
            text = self.real_topology().read_text(encoding="utf-8").replace(
                'package_version = "0.2.0-rc.1"', 'package_version = "9.9.9"', 1
            )
            topology_path.write_text(text, encoding="utf-8")
            with self.assertRaises(ValueError):
                candidate.derive_payload(ROOT, topology_path, self.real_identities())

    def test_unpublished_internal_dependency_absent_from_candidate_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            topology_path = Path(directory) / "topology.toml"
            text = self.real_topology().read_text(encoding="utf-8")
            # Exclude effortless-repo-protocol from the candidate; the allow
            # family still depends on it and the topology marks it
            # unpublished, so derivation must fail closed.
            block_start = text.index('logical_id = "repo-protocol"')
            if block_start < 0:
                self.fail("repo-protocol row missing from topology")
            segment = text[block_start : block_start + 600]
            self.assertIn("candidate_inclusion = true", segment)
            patched = (
                text[:block_start]
                + segment.replace("candidate_inclusion = true", "candidate_inclusion = false", 1)
                + text[block_start + 600 :]
            )
            topology_path.write_text(patched, encoding="utf-8")
            with self.assertRaises(ValueError):
                candidate.derive_payload(ROOT, topology_path, self.real_identities())

    def test_verify_packaged_requires_exact_filename_and_clean_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            package_dir = Path(directory)
            crate_root = Path(directory) / "allow-core-0.2.0-rc.1"
            crate_root.mkdir()
            (crate_root / "Cargo.toml").write_text(
                '[package]\nname = "allow-core"\nversion = "0.2.0-rc.1"\n'
                '[dependencies]\nallow-policy = { version = "0.2.0-rc.1" }\n',
                encoding="utf-8",
            )
            crate_path = package_dir / "allow-core-0.2.0-rc.1.crate"
            with tarfile.open(crate_path, "w:gz") as archive:
                archive.add(
                    crate_root,
                    arcname="allow-core-0.2.0-rc.1",
                    filter=lambda info: info,
                )

            payload = candidate.derive_payload(
                ROOT, self.real_topology(), self.real_identities()
            )
            row = next(
                row
                for row in payload["rows"]
                if row["cargo_package_name"] == "allow-core"
            )
            row["expected_dependency_rows"] = [
                {
                    "package_name": "allow-policy",
                    "package_version": "0.2.0-rc.1",
                    "dependency_kind": "internal",
                }
            ]
            # Only the allow-core row is verified in isolation: narrow the
            # payload so verify-packaged does not demand the other crates.
            payload["rows"] = [row]

            candidate.verify_packaged(payload, package_dir)
            self.assertTrue(str(row["crate_digest"]).startswith("sha256:"))
            self.assertGreater(row["crate_size_bytes"], 0)

            # A wrong-version filename must fail the exact identity law.
            wrong = package_dir / "allow-core-0.1.11.crate"
            crate_path.rename(wrong)
            with self.assertRaises(ValueError):
                candidate.verify_packaged(payload, package_dir)
            wrong.rename(crate_path)

            # A packaged path dependency must fail the packaging law.
            leak_root = Path(directory) / "leaky"
            leak_root.mkdir()
            (leak_root / "Cargo.toml").write_text(
                '[package]\nname = "allow-core"\nversion = "0.2.0-rc.1"\n'
                '[dependencies]\nallow-policy = { path = "../allow-policy" }\n',
                encoding="utf-8",
            )
            leaky = package_dir / "allow-core-0.2.0-rc.1.crate"
            leaky.unlink()
            with tarfile.open(leaky, "w:gz") as archive:
                archive.add(leak_root, arcname="allow-core-0.2.0-rc.1")
            with self.assertRaises(ValueError):
                candidate.verify_packaged(payload, package_dir)


if __name__ == "__main__":
    unittest.main()
