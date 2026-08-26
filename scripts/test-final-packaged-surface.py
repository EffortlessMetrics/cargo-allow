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
                f"{name}-{version}/Cargo.toml": b"[package]\nname = 'demo'\nversion = '0.2.0'\n",
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


if __name__ == "__main__":
    unittest.main()
