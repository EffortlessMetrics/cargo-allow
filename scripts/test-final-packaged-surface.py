#!/usr/bin/env python3
"""Focused regression tests for Cargo archive identity parsing (#3968)."""

import importlib.util
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("final-packaged-surface.py")
SPEC = importlib.util.spec_from_file_location("final_packaged_surface", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class ArchiveIdentityTests(unittest.TestCase):
    def test_preserves_prerelease_and_build_metadata(self):
        self.assertEqual(
            MODULE.archive_identity(
                "allow-core-0.2.0-rc.1+build.7.crate",
                expected_name="allow-core",
            ),
            ("allow-core", "0.2.0-rc.1+build.7"),
        )

    def test_preserves_hyphens_in_package_name(self):
        self.assertEqual(
            MODULE.archive_identity(
                "allow-policy-legacy-0.2.0-rc.1.crate",
                expected_name="allow-policy-legacy",
            ),
            ("allow-policy-legacy", "0.2.0-rc.1"),
        )

    def test_rejects_missing_suffix_or_prefix(self):
        for filename in (
            "allow-core-0.2.0-rc.1.tar.gz",
            "other-crate-0.2.0-rc.1.crate",
            "allow-core-.crate",
        ):
            with self.subTest(filename=filename):
                with self.assertRaises(ValueError):
                    MODULE.archive_identity(filename, expected_name="allow-core")

    def test_expected_version_is_checked_as_a_complete_suffix(self):
        self.assertEqual(
            MODULE.archive_identity(
                "allow-core-0.2.0-rc.1.crate",
                expected_version="0.2.0-rc.1",
            ),
            ("allow-core", "0.2.0-rc.1"),
        )
        with self.assertRaises(ValueError):
            MODULE.archive_identity(
                "allow-core-0.2.0-rc.1.crate",
                expected_version="rc.1",
            )


if __name__ == "__main__":
    unittest.main()
