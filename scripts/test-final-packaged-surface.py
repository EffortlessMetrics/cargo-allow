#!/usr/bin/env python3
"""Focused regression tests for packaged-surface identity and assets (#3968)."""

import importlib.util
import io
import json
import subprocess
import sys
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

    def run_cli(
        self,
        package_set: Path,
        packages: Path,
        output: Path,
        expected_version: str | None = None,
    ) -> subprocess.CompletedProcess[str]:
        command = [
            sys.executable,
            str(ROOT / "final-packaged-surface.py"),
            "--package-set-receipt",
            str(package_set),
            "--packages-dir",
            str(packages),
            "--output",
            str(output),
        ]
        if expected_version is not None:
            command.extend(["--expected-version", expected_version])
        return subprocess.run(command, capture_output=True, text=True)

    def write_package_set(
        self,
        path: Path,
        name: str,
        version: str,
        workspace_version: str | None = None,
    ) -> None:
        candidate = (
            {"workspace_version": workspace_version}
            if workspace_version is not None
            else {}
        )
        path.write_text(
            json.dumps(
                {
                    "candidate": candidate,
                    "package_set": {"crates": [{"name": name, "version": version}]},
                }
            ),
            encoding="utf-8",
        )

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
            self.write_package_set(package_set, "demo", "0.2.0", "0.2.0")
            output = root / "surface.json"
            result = self.run_cli(package_set, packages, output, "0.2.0")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(output.read_text())["result"], "Complete")
            self.assertTrue(archive.exists())

    def test_cli_preserves_prerelease_and_build_identity_end_to_end(self):
        proof = emit_prerelease_identity_proof()
        self.assertEqual(proof["schema"], "cargo-allow.release-rehearsal-prerelease-identity-proof.v1")
        self.assertEqual(proof["result"], "Complete")
        self.assertEqual(
            proof["candidate_workspace_version"], "1.2.3-alpha.1+build.7"
        )
        self.assertEqual(proof["package_name"], "effortless-repo-protocol")
        self.assertEqual(proof["package_version"], "1.2.3-alpha.1+build.7")
        self.assertEqual(proof["manifest_version"], "1.2.3-alpha.1+build.7")
        self.assertEqual(
            proof["crate_file"],
            "effortless-repo-protocol-1.2.3-alpha.1+build.7.crate",
        )

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

    def test_surface_rejects_unexpected_archive(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive, packages = self.make_crate(root, version="0.2.0-rc.1")
            package_set = root / "package-set.json"
            self.write_package_set(package_set, "demo", "0.2.0")
            result = self.run_cli(package_set, packages, root / "surface.json")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(f"unexpected packaged crate: {archive.name}", result.stderr)

    def test_cli_rejects_final_hyphen_truncation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive, packages = self.make_crate(
                root,
                name="allow-core",
                version="0.2.0-rc.1",
            )
            truncated = packages / "allow-core-rc.1.crate"
            archive.rename(truncated)
            package_set = root / "package-set.json"
            self.write_package_set(
                package_set,
                "allow-core",
                "0.2.0-rc.1",
                "0.2.0-rc.1",
            )

            result = self.run_cli(
                package_set,
                packages,
                root / "surface.json",
                "0.2.0-rc.1",
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(
                "unexpected packaged crate: allow-core-rc.1.crate",
                result.stderr,
            )


def emit_prerelease_identity_proof() -> dict[str, object]:
    case = FinalPackagedSurfaceTests(methodName="runTest")
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        name = "effortless-repo-protocol"
        version = "1.2.3-alpha.1+build.7"
        archive, packages = case.make_crate(root, name=name, version=version)
        package_set = root / "package-set.json"
        case.write_package_set(package_set, name, version, version)
        output = root / "surface.json"
        result = case.run_cli(package_set, packages, output, version)
        if result.returncode != 0:
            raise RuntimeError(result.stderr or result.stdout)
        receipt = json.loads(output.read_text(encoding="utf-8"))
        row = receipt["package_set"]["packages"][0]
        return {
            "schema": "cargo-allow.release-rehearsal-prerelease-identity-proof.v1",
            "result": receipt["result"],
            "candidate_workspace_version": receipt["candidate"]["workspace_version"],
            "package_name": row["name"],
            "package_version": row["version"],
            "manifest_version": row["metadata"]["package"]["version"],
            "crate_file": row["crate_file"],
            "observed_archive_name": archive.name,
        }


if __name__ == "__main__":
    if sys.argv[1:] == ["--emit-corpus-proof"]:
        print(json.dumps(emit_prerelease_identity_proof(), sort_keys=True))
    else:
        unittest.main()
