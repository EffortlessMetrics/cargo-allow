#!/usr/bin/env python3
"""Tests and negative controls for release-rehearsal harness."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
REHEARSAL_PATH = ROOT / "scripts/release-rehearsal.py"
SPEC = importlib.util.spec_from_file_location("release_rehearsal", REHEARSAL_PATH)
if SPEC is None or SPEC.loader is None:
    raise SystemExit("could not load release rehearsal harness")
REHEARSAL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REHEARSAL)

class TestReleaseRehearsal(unittest.TestCase):
    def test_harness_produces_valid_receipt(self):
        receipt = REHEARSAL.build_rehearsal_receipt("0123456789abcdef0123456789abcdef01234567")
        self.assertEqual(receipt["schema_version"], "1.0")
        self.assertTrue(receipt["subject_lockfile_digest"].startswith("sha256:"))
        self.assertTrue(receipt["subject_topology_digest"].startswith("sha256:"))
        self.assertEqual(receipt["aggregate_status"], "Complete")
        
        # Verify zero-mutation proof guarantees
        proof = receipt["zero_mutation_proof"]
        self.assertTrue(proof["tag_mutation_prevented"])
        self.assertTrue(proof["token_read_prevented"])
        self.assertTrue(proof["cargo_publish_prevented"])
        self.assertTrue(proof["registry_mutation_prevented"])
        self.assertTrue(proof["github_release_mutation_prevented"])
        self.assertTrue(proof["live_setting_mutation_prevented"])
        self.assertTrue(proof["external_repository_mutation_prevented"])

        # Verify all 8 phases are present and Complete
        phases = receipt["phases"]
        for required_phase in [
            "release_identity",
            "candidate_package_set",
            "shared_prerequisites",
            "publisher_state_machine",
            "docs_and_support_identity",
            "manifest_and_assets",
            "authorization_boundary",
            "workflow_graph_permissions",
        ]:
            self.assertIn(required_phase, phases)
            self.assertEqual(phases[required_phase], "Complete")

    def test_negative_control_token_leak_fails(self):
        import os
        old_val = os.environ.get("CARGO_REGISTRY_TOKEN")
        try:
            os.environ["CARGO_REGISTRY_TOKEN"] = "leak_test"
            receipt = {}
            status = REHEARSAL.run_phase_authorization_boundary(receipt)
            self.assertEqual(status, "InstrumentFailure")
        finally:
            if old_val is None:
                os.environ.pop("CARGO_REGISTRY_TOKEN", None)
            else:
                os.environ["CARGO_REGISTRY_TOKEN"] = old_val

if __name__ == "__main__":
    unittest.main()
