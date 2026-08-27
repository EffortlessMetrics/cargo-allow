#!/usr/bin/env python3
"""Focused regression tests for packaged-surface identity and assets (#3968)."""

import importlib.util
import io
import json
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("surface", ROOT / "final-packaged-surface.py")
surface = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(surface)


class FinalPackagedSurfaceTests(unittest.TestCase):
    def make_crate(self, root: Path, name="demo", version="0.2.0", readme=True):
        package = root / "packages"
        package.mkdir()
        archive_path = package / f"{name}-{version}.crate"
        with tarfile.open(archive_path, "w:gz") as archive:
            files = {
                f"{name}-{version}/Cargo.toml": (
                    f"[package]\nname = '{name}'\nversion = '{version}'\n"
                ).encode(),
                f"{name}-{version}/LICENSE": b"MIT\n",
            }
            if readme:
                files[f"{name}-{version}/README.md"] = b"# demo\n"
            for path, data in files.items():
                info = tarfile.TarInfo(path)
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
        return archive_path, package

    def test_surface_binds_archive_digest_and_assets(self):
        with tempfile.TemporaryDirectory() as directory:
            archive, _ = self.make_crate(Path(directory))
            row = surface.surface(archive, "0.2.0")
            self.assertEqual(row["result"], "Complete")
            self.assertEqual(row["version"], "0.2.0")
            self.assertEqual(row["size_bytes"], archive.stat().st_size)
            self.assertTrue(row["assets"]["readme"]["sha256"])

    def test_missing_declared_asset_is_incomplete(self):
        with tempfile.TemporaryDirectory() as directory:
            archive, _ = self.make_crate(Path(directory), readme=False)
            row = surface.surface(archive, "0.2.0")
            self.assertEqual(row["result"], "Incomplete")
            self.assertFalse(row["assets"]["readme"]["present"])

    def test_package_set_order_and_identity_are_checked(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive, packages = self.make_crate(root)
            package_set = root / "package-set.json"
            package_set.write_text(
                json.dumps(
                    {
                        "candidate": {},
                        "package_set": {
                            "crates": [{"name": "demo", "version": "0.2.0"}]
                        },
                    }
                )
            )
            output = root / "surface.json"
            subprocess.run(
                [
                    "python3",
                    str(ROOT / "final-packaged-surface.py"),
                    "--package-set-receipt",
                    str(package_set),
                    "--packages-dir",
                    str(packages),
                    "--output",
                    str(output),
                    "--expected-version",
                    "0.2.0",
                ],
                check=True,
            )
            self.assertEqual(json.loads(output.read_text())["result"], "Complete")
            self.assertTrue(archive.exists())


class ArchiveIdentityTests(unittest.TestCase):
    def test_preserves_prerelease_and_build_metadata(self):
        self.assertEqual(
            surface.archive_identity(
                "allow-core-0.2.0-rc.1+build.7.crate",
                expected_name="allow-core",
            ),
            ("allow-core", "0.2.0-rc.1+build.7"),
        )

    def test_preserves_hyphens_in_package_name(self):
        self.assertEqual(
            surface.archive_identity(
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
                    surface.archive_identity(filename, expected_name="allow-core")

    def test_expected_version_is_checked_as_a_complete_suffix(self):
        self.assertEqual(
            surface.archive_identity(
                "allow-core-0.2.0-rc.1.crate",
                expected_version="0.2.0-rc.1",
            ),
            ("allow-core", "0.2.0-rc.1"),
        )
        with self.assertRaises(ValueError):
            surface.archive_identity(
                "allow-core-0.2.0-rc.1.crate",
                expected_version="rc.1",
            )


if __name__ == "__main__":
    unittest.main()
