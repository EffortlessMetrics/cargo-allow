#!/usr/bin/env python3
"""Characterization tests for exact candidate .crate filename identity."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from exact_candidate_package_identity import crate_version_from_filename


class CrateVersionFromFilenameTests(unittest.TestCase):
    def test_preserves_release_candidate_identity(self) -> None:
        self.assertEqual(
            crate_version_from_filename(
                "allow-core", "allow-core-0.2.0-rc.1.crate"
            ),
            "0.2.0-rc.1",
        )

    def test_preserves_hyphenated_package_and_build_metadata(self) -> None:
        self.assertEqual(
            crate_version_from_filename(
                "effortless-repo-protocol",
                "effortless-repo-protocol-1.2.3-alpha.1+build.7.crate",
            ),
            "1.2.3-alpha.1+build.7",
        )

    def test_rejects_malformed_or_mismatched_filenames(self) -> None:
        cases = [
            ("", "allow-core-0.2.0-rc.1.crate"),
            ("nested/allow-core", "nested/allow-core-0.2.0-rc.1.crate"),
            ("nested/allow-core", "nested/allow-core-0.2.0-rc.1.crate"),
            ("allow-core", "allow-core-0.2.0-rc.1.tar.gz"),
            ("allow-core", "allow-policy-0.2.0-rc.1.crate"),
            ("allow-core", "allow-core-.crate"),
            ("allow-core", "nested/allow-core-0.2.0-rc.1.crate"),
            ("allow-core", r"nested\allow-core-0.2.0-rc.1.crate"),
        ]
        for package_name, crate_file in cases:
            with self.subTest(package_name=package_name, crate_file=crate_file):
                with self.assertRaises(ValueError):
                    crate_version_from_filename(package_name, crate_file)

    def test_consumers_never_split_identity_on_the_final_hyphen(self) -> None:
        scripts_dir = Path(__file__).resolve().parent
        forbidden = ('rsplit("-", 1)', 'rpartition("-")')
        for filename in (
            "exact-candidate-package-set.sh",
            "final-packaged-surface.py",
        ):
            source = (scripts_dir / filename).read_text(encoding="utf-8")
            self.assertIn("crate_version_from_filename", source)
            for token in forbidden:
                with self.subTest(filename=filename, token=token):
                    self.assertNotIn(token, source)

if __name__ == "__main__":
    unittest.main()
