"""Controls for the product-scoped direct-floor workspace projection."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import tomllib
import unittest


SPEC = importlib.util.spec_from_file_location(
    "floor_product_workspace", Path(__file__).with_name("floor_product_workspace.py")
)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ProductWorkspaceProjectionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="floor-product-workspace-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for name in ("product-a", "product-b", "shared", "dev-helper"):
            (self.root / "crates" / name).mkdir(parents=True)
        (self.root / "Cargo.toml").write_text(
            '[workspace]\nresolver = "3"\nmembers = [\n'
            '  "crates/product-a",\n  "crates/product-b",\n'
            '  "crates/shared",\n  "crates/dev-helper",\n]\n'
            'default-members = [\n  "crates/product-a",\n  "crates/product-b",\n]\n\n'
            '[workspace.package]\nedition = "2024"\n\n'
            '[workspace.dependencies]\n'
            'shared = { path = "crates/shared" }\n'
            'dev-helper = { path = "crates/dev-helper" }\n'
            'ambient-only = "9"\n',
            encoding="utf-8",
            newline="\n",
        )
        manifests = {
            "product-a": (
                '[package]\nname = "product-a"\nversion = "0.1.0"\n'
                'edition.workspace = true\n[dependencies]\nshared.workspace = true\n'
                '[dev-dependencies]\ndev-helper.workspace = true\n'
            ),
            "product-b": (
                '[package]\nname = "product-b"\nversion = "0.1.0"\n'
                'edition.workspace = true\n[dependencies]\nambient-only.workspace = true\n'
            ),
            "shared": '[package]\nname = "shared"\nversion = "0.1.0"\nedition.workspace = true\n',
            "dev-helper": '[package]\nname = "dev-helper"\nversion = "0.1.0"\nedition.workspace = true\n',
        }
        for name, content in manifests.items():
            (self.root / "crates" / name / "Cargo.toml").write_text(
                content, encoding="utf-8", newline="\n"
            )

    def project(self, closure):
        selection = self.root / "selection.json"
        selection.write_text(
            json.dumps({"roots": ["product-a"], "closure": closure}),
            encoding="utf-8",
        )
        identity = self.root / "identity.json"
        result = MODULE.project_workspace(self.root / "Cargo.toml", selection, identity)
        return result, json.loads(identity.read_text(encoding="utf-8"))

    def test_unrelated_member_cannot_enter_the_execution_workspace(self):
        result, retained = self.project(["shared", "product-a"])
        manifest = tomllib.loads((self.root / "Cargo.toml").read_text(encoding="utf-8"))
        self.assertEqual(
            manifest["workspace"]["members"],
            ["crates/product-a", "crates/shared", "crates/dev-helper"],
        )
        self.assertEqual(
            manifest["workspace"]["default-members"],
            ["crates/product-a", "crates/shared"],
        )
        self.assertNotIn("product-b", result["execution_packages"])
        self.assertIn("crates/product-b", result["omitted_member_paths"])
        self.assertEqual(result, retained)

    def test_selection_order_does_not_change_projected_bytes_or_identity(self):
        original = (self.root / "Cargo.toml").read_bytes()
        first, _ = self.project(["product-a", "shared"])
        first_manifest = (self.root / "Cargo.toml").read_bytes()
        (self.root / "Cargo.toml").write_bytes(original)
        second, _ = self.project(["shared", "product-a"])
        self.assertEqual(first_manifest, (self.root / "Cargo.toml").read_bytes())
        self.assertEqual(first, second)

    def test_unknown_package_and_missing_root_fail_closed(self):
        for payload, diagnostic in (
            ({"roots": ["product-a"], "closure": ["unknown"]}, "outside the workspace"),
            ({"roots": ["product-a"], "closure": ["shared"]}, "roots are absent"),
        ):
            with self.subTest(payload=payload):
                selection = self.root / "selection.json"
                selection.write_text(json.dumps(payload), encoding="utf-8")
                with self.assertRaisesRegex(ValueError, diagnostic):
                    MODULE.project_workspace(
                        self.root / "Cargo.toml", selection, self.root / "identity.json"
                    )


if __name__ == "__main__":
    unittest.main()
