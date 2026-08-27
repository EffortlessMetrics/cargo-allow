#!/usr/bin/env python3
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
import tarfile


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
                    f"[package]\nname = '{name}'\nversion = '{version}'\n".encode()
                ),
                f"{name}-{version}/LICENSE": b"MIT\n",
            }
            if readme:
                files[f"{name}-{version}/README.md"] = b"# demo\n"
            for path, data in files.items():
                info = tarfile.TarInfo(path)
                info.size = len(data)
                import io
                archive.addfile(info, io.BytesIO(data))
        return archive_path, package

    def test_surface_binds_archive_digest_and_assets(self):
        with tempfile.TemporaryDirectory() as directory:
            archive, _ = self.make_crate(Path(directory))
            row = surface.surface(archive, "demo", "0.2.0")
            self.assertEqual(row["result"], "Complete")
            self.assertEqual(row["version"], "0.2.0")
            self.assertEqual(row["size_bytes"], archive.stat().st_size)
            self.assertTrue(row["assets"]["readme"]["sha256"])

    def test_missing_declared_asset_is_incomplete(self):
        with tempfile.TemporaryDirectory() as directory:
            archive, _ = self.make_crate(Path(directory), readme=False)
            row = surface.surface(archive, "demo", "0.2.0")
            self.assertEqual(row["result"], "Incomplete")
            self.assertFalse(row["assets"]["readme"]["present"])

    def test_package_set_order_and_identity_are_checked(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive, packages = self.make_crate(root)
            package_set = root / "package-set.json"
            package_set.write_text(json.dumps({"candidate": {}, "package_set": {"crates": [{"name": "demo", "version": "0.2.0"}]}}))
            output = root / "surface.json"
            import subprocess
            subprocess.run([
                "python3", str(ROOT / "final-packaged-surface.py"),
                "--package-set-receipt", str(package_set),
                "--packages-dir", str(packages),
                "--output", str(output),
                "--expected-version", "0.2.0",
            ], check=True)
            self.assertEqual(json.loads(output.read_text())["result"], "Complete")
            self.assertTrue(archive.exists())

    def test_surface_preserves_prerelease_and_hyphenated_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            archive, _ = self.make_crate(
                Path(directory),
                name="effortless-repo-protocol",
                version="1.2.3-alpha.1+build.7",
            )
            row = surface.surface(
                archive, "effortless-repo-protocol", "1.2.3-alpha.1+build.7"
            )
            self.assertEqual(row["name"], "effortless-repo-protocol")
            self.assertEqual(row["version"], "1.2.3-alpha.1+build.7")

    def test_surface_rejects_archive_that_is_not_expected_by_package_set(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive, packages = self.make_crate(
                root, name="demo", version="0.2.0-rc.1"
            )
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
            import subprocess

            result = subprocess.run(
                [
                    "python3",
                    str(ROOT / "final-packaged-surface.py"),
                    "--package-set-receipt",
                    str(package_set),
                    "--packages-dir",
                    str(packages),
                    "--output",
                    str(output),
                ],
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(f"unexpected packaged crate: {archive.name}", result.stderr)


if __name__ == "__main__":
    unittest.main()
