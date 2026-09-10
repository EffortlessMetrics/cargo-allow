"""Behavioral controls for the manifest-only floor selection producer."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


REPO = Path(__file__).resolve().parent.parent


def select(root, roots):
    source = (REPO / "scripts/proof-direct-floors.sh").read_text(encoding="utf-8")
    marker = 'python3 - Cargo.toml "${ROOTS[@]}" > floors-selection.json <<\'PY\'\n'
    program = source.split(marker, 1)[1].split("\nPY\n", 1)[0]
    result = subprocess.run(
        [sys.executable, "-", "Cargo.toml", *roots], input=program,
        cwd=root, capture_output=True, encoding="utf-8", check=False,
    )
    if result.returncode:
        raise RuntimeError(result.stderr)
    return json.loads(result.stdout)


class FloorSelectionTests(unittest.TestCase):
    def test_digest_binds_declared_members_outside_crates_and_ignores_crlf(self):
        source = (REPO / "scripts/proof-direct-floors.sh").read_text(encoding="utf-8")
        marker = 'manifest_set_digest="$(\n  python3 - <<\'PY\'\n'
        program = source.split(marker, 1)[1].split("\nPY\n", 1)[0]
        with tempfile.TemporaryDirectory(prefix="floor-digest-") as folder:
            root = Path(folder)
            (root / "crates/a").mkdir(parents=True)
            (root / "vendor/b").mkdir(parents=True)
            manifests = {
                "Cargo.toml": "[workspace]\nmembers = ['crates/a', 'vendor/b']\n",
                "crates/a/Cargo.toml": "[package]\nname = 'a'\n",
                "vendor/b/Cargo.toml": "[package]\nname = 'b'\n",
            }

            def digest():
                return subprocess.run(
                    [sys.executable, "-"], input=program, cwd=root,
                    capture_output=True, encoding="utf-8", check=True,
                ).stdout.strip()

            for relative, body in manifests.items():
                (root / relative).write_bytes(body.encode("utf-8"))
            lf = digest()
            for relative, body in manifests.items():
                (root / relative).write_bytes(body.replace("\n", "\r\n").encode("utf-8"))
            self.assertEqual(digest(), lf)
            (root / "vendor/b/Cargo.toml").write_bytes(b"[package]\nname = 'changed'\n")
            self.assertNotEqual(digest(), lf)

    def test_live_changie_activation_certifies_yaml_floor(self):
        rows = select(REPO, ["cargo-allow"])["floors"]
        self.assertIn("yaml-rust2", {row["package"] for row in rows})

    def test_forwarding_revisits_members_and_keeps_inactive_optional_out(self):
        with tempfile.TemporaryDirectory(prefix="floor-selection-") as folder:
            root = Path(folder)
            manifests = {
                "Cargo.toml": '''[workspace]
members = ["a", "b", "c"]
[workspace.dependencies]
b = { path = "b", features = ["inherited"] }
''',
                "a/Cargo.toml": '''[package]
name = "a"
[dependencies]
b = { workspace = true, features = ["member"] }
c = { path = "../c" }
''',
                "b/Cargo.toml": '''[package]
name = "b"
[features]
inherited = ["dep:first"]
member = ["second"]
later = ["dep:third"]
default = ["dormant", "dormant?/unused"]
dormant = []
hidden = ["dep:dormant"]
[dependencies]
first = { version = "0.1", optional = true }
second = { version = "0.2", optional = true }
third = { version = "0.3", optional = true }
dormant = { version = "0.4", optional = true }
''',
                "c/Cargo.toml": '''[package]
name = "c"
[features]
default = ["b/later"]
[dependencies]
b = { path = "../b" }
''',
            }
            for relative, contents in manifests.items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(contents, encoding="utf-8")
            result = select(root, ["a"])
            self.assertEqual(result["closure"], ["a", "b", "c"])
            self.assertEqual(
                [row["package"] for row in result["floors"]],
                ["first", "second", "third"],
            )


if __name__ == "__main__":
    unittest.main()
