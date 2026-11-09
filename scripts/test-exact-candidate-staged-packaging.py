#!/usr/bin/env python3
"""Contract characterization for the staged outside-workspace packaging producer (#2886).

Builds a self-contained fixture workspace whose two candidate rows carry
deliberately different versions (0.1.0 leaf, 0.2.0 top), stages it with the
real producer, packages it offline against a fixture-only local registry
with an isolated CARGO_HOME (no ambient cache, no network), and asserts the
mixed-version candidate laws end to end — including the verification build,
which is the offline ceiling short of a registry upload (`cargo publish
--dry-run` is blocked by cargo itself whenever crates.io is source-replaced
by a local registry). Negative controls fail closed on digest mismatch,
missing packages, membership drift, packaged-manifest drift, registry
checksum drift, and lockfile membership drift. No upload occurs and no
crates.io lookup is reachable from this suite.
"""

from __future__ import annotations

import importlib.util
import io
import json
import os
import subprocess
import tarfile
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PRODUCER_PATH = ROOT / "scripts/exact-candidate-staged-packaging.py"
SPEC = importlib.util.spec_from_file_location(
    "exact_candidate_staged_packaging", PRODUCER_PATH
)
if SPEC is None or SPEC.loader is None:
    raise SystemExit("could not load exact-candidate-staged-packaging.py")
producer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(producer)

FIXTURE_TOPOLOGY = """\
schema_version = "2.0"
authority_generation = 2
topology_id = "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001"

[[package]]
logical_id = "fixture-leaf"
cargo_package_name = "fixture-leaf"
  version_line = "shared-0.1"
product_family = "shared"
package_version = "0.1.0"
publication_state = "UnpublishedInternal"
publish = true
candidate_inclusion = true
release_order = 10
support_tier = "internal-stabilizing"

[[package]]
logical_id = "cargo-allow"
cargo_package_name = "fixture-top"
  version_line = "cargo-allow-0.2"
product_family = "cargo-allow"
package_version = "0.2.0"
publication_state = "UnpublishedInternal"
publish = true
candidate_inclusion = true
release_order = 20
support_tier = "supported"
"""

FIXTURE_IDENTITIES = """\
manifest_id = "CARGO-ALLOW-ARCH-V2-0001"

[[crate_identity]]
logical_id = "fixture-leaf"
workspace_path = "crates/fixture-leaf"

[[crate_identity]]
logical_id = "cargo-allow"
workspace_path = "crates/fixture-top"
"""

FIXTURE_WORKSPACE_MANIFEST = """\
[workspace]
resolver = "3"
members = [
  "crates/fixture-leaf",
  "crates/fixture-top",
  "crates/fixture-devonly",
]

[workspace.lints.rust]
warnings = "deny"

[workspace.package]
edition = "2021"
license = "MIT OR Apache-2.0"
version = "0.2.0"
rust-version = "1.85"
repository = "https://fixture.invalid/cargo-allow"
readme = "README.md"
keywords = ["fixture"]
categories = ["development-tools"]

[workspace.dependencies]
fixture-leaf = { path = "crates/fixture-leaf", version = "0.1.0" }
"""

FIXTURE_LEAF_MANIFEST = """\
[package]
name = "fixture-leaf"
description = "Fixture leaf at a deliberately different candidate version."
version = "0.1.0"
edition.workspace = true
license.workspace = true
readme = "README.md"

[lints]
workspace = true
"""

FIXTURE_TOP_MANIFEST = """\
[package]
name = "fixture-top"
description = "Fixture top at the workspace candidate version."
version.workspace = true
edition.workspace = true
license.workspace = true
readme = "README.md"

[dependencies]
fixture-leaf.workspace = true

[dev-dependencies]
fixture-devonly = { path = "../fixture-devonly" }

[lints]
workspace = true
"""

FIXTURE_DEVONLY_MANIFEST = """\
[package]
name = "fixture-devonly"
description = "Unselected dev-only fixture source."
version = "9.9.9-unpublished"
edition.workspace = true
license.workspace = true

[dependencies]
fixture-leaf.workspace = true

[lints]
workspace = true
"""

FIXTURE_LIB = "pub fn fixture() -> u32 { 1 }\n"


def build_fixture(root: Path) -> None:
    (root / "crates/fixture-leaf/src").mkdir(parents=True)
    (root / "crates/fixture-top/src").mkdir(parents=True)
    (root / "crates/fixture-devonly/src").mkdir(parents=True)
    (root / "Cargo.toml").write_text(FIXTURE_WORKSPACE_MANIFEST, encoding="utf-8")
    (root / "crates/fixture-leaf/Cargo.toml").write_text(
        FIXTURE_LEAF_MANIFEST, encoding="utf-8"
    )
    (root / "crates/fixture-top/Cargo.toml").write_text(
        FIXTURE_TOP_MANIFEST, encoding="utf-8"
    )
    (root / "crates/fixture-devonly/Cargo.toml").write_text(
        FIXTURE_DEVONLY_MANIFEST, encoding="utf-8"
    )
    for name in ("fixture-leaf", "fixture-top"):
        (root / f"crates/{name}/README.md").write_text(
            f"{name} fixture readme\n", encoding="utf-8"
        )
    for name in ("fixture-leaf", "fixture-top", "fixture-devonly"):
        (root / f"crates/{name}/src/lib.rs").write_text(FIXTURE_LIB, encoding="utf-8")
    (root / "policy").mkdir()
    (root / "policy/fixture-topology.toml").write_text(
        FIXTURE_TOPOLOGY, encoding="utf-8"
    )
    (root / "policy/fixture-identities.toml").write_text(
        FIXTURE_IDENTITIES, encoding="utf-8"
    )
    # Minimal lockfile: the fixture closure has no external registry packages,
    # so the staged registry warm set is empty by construction.
    (root / "Cargo.lock").write_text(
        'version = 4\n\n[[package]]\nname = "fixture-leaf"\nversion = "0.1.0"\n',
        encoding="utf-8",
    )
    environment = {
        **os.environ,
        "GIT_AUTHOR_NAME": "fixture",
        "GIT_AUTHOR_EMAIL": "fixture@invalid",
        "GIT_COMMITTER_NAME": "fixture",
        "GIT_COMMITTER_EMAIL": "fixture@invalid",
    }
    for argv in (["init"], ["add", "."], ["commit", "-m", "fixture closure"]):
        subprocess.run(
            ["git", *argv], cwd=root, check=True, capture_output=True, env=environment
        )


def packaged_manifest(crate_path: Path, name: str, version: str) -> dict:
    with tarfile.open(crate_path, "r:gz") as archive:
        extracted = archive.extractfile(f"{name}-{version}/Cargo.toml")
        if extracted is None:
            raise AssertionError(f"no manifest in {crate_path.name}")
        return tomllib.loads(extracted.read().decode("utf-8"))


class StagedPackagingFixtureTests(unittest.TestCase):
    def setUp(self) -> None:
        self._temporary = tempfile.TemporaryDirectory(prefix="staged-packaging-")
        base = Path(self._temporary.name)
        self.fixture_root = base / "fixture"
        self.output_root = base / "out"
        self.cargo_home = base / "cargo-home"
        self.cargo_home.mkdir()
        build_fixture(self.fixture_root)
        self._original_cargo_home = os.environ.get("CARGO_HOME")
        os.environ["CARGO_HOME"] = str(self.cargo_home)

    def tearDown(self) -> None:
        if self._original_cargo_home is None:
            os.environ.pop("CARGO_HOME", None)
        else:
            os.environ["CARGO_HOME"] = self._original_cargo_home
        self._temporary.cleanup()

    def payload(self) -> dict:
        return producer.load_payload(
            self.fixture_root,
            self.fixture_root / "policy/fixture-topology.toml",
            self.fixture_root / "policy/fixture-identities.toml",
        )

    def run_all_modes(self) -> dict:
        payload = self.payload()
        staged = producer.stage_closure(self.fixture_root, payload, self.output_root)
        packaged = producer.package_rows(
            self.fixture_root, payload, self.output_root, verify_build=True
        )
        return {"staged": staged, "packaged": packaged, "payload": payload}

    def verify_with(self, rows_state: list[dict]) -> None:
        """Run verify_staged with explicit receipt-authority rows."""
        payload = self.payload()
        for row, prior in zip(payload["rows"], rows_state):
            row["crate_digest"] = prior["crate_digest"]
            row["crate_size_bytes"] = prior["crate_size_bytes"]
        producer.verify_staged(self.fixture_root, payload, self.output_root)

    def test_toml_emitter_round_trips_fixture_and_real_manifests(self):
        samples = [
            self.fixture_root / "Cargo.toml",
            ROOT / "crates/cargo-allow/Cargo.toml",
            ROOT / "crates/allow-rust/Cargo.toml",
        ]
        for manifest in samples:
            document = tomllib.loads(manifest.read_text(encoding="utf-8"))
            reparsed = tomllib.loads(producer.toml_dumps(document))
            self.assertEqual(reparsed, document, manifest.name)

    def test_mixed_version_fixture_packages_offline_and_verifies(self):
        result = self.run_all_modes()
        rows = result["packaged"]["rows"]
        self.assertEqual(len(rows), 2)
        self.assertEqual(rows[0]["name"], "fixture-leaf")
        self.assertEqual(rows[0]["version"], "0.1.0")
        self.assertEqual(rows[1]["name"], "fixture-top")
        self.assertEqual(rows[1]["version"], "0.2.0")
        for row in rows:
            self.assertEqual(row["state"], "packaged_locked_reverified")
            self.assertTrue(str(row["crate_digest"]).startswith("sha256:"))
        # The mixed-version law: two deliberately different versions are both
        # selected and both packaged, never one workspace version.
        self.assertNotEqual(rows[0]["version"], rows[1]["version"])

        top = packaged_manifest(Path(rows[1]["crate"]), "fixture-top", "0.2.0")
        # The packaged cross-dependency requirement carries the leaf row's
        # exact topology version, not the workspace version line.
        self.assertEqual(top["dependencies"]["fixture-leaf"], {"version": "0.1.0"})
        # Path-only dev dependencies are stripped from the packaged manifest
        # exactly as on the ambient publish path.
        self.assertNotIn("fixture-devonly", top.get("dev-dependencies", {}))
        self.assertEqual(top["package"]["version"], "0.2.0")

        staged_top = producer.parse_toml(
            self.output_root / "workspace/crates/fixture-top/Cargo.toml"
        )
        self.assertEqual(
            staged_top["dependencies"]["fixture-leaf"], {"version": "0.1.0"}
        )
        self.assertEqual(
            staged_top["dev-dependencies"]["fixture-devonly"],
            {"path": "../../dev-only/fixture-devonly"},
        )
        devonly = producer.parse_toml(
            self.output_root / "workspace/dev-only/fixture-devonly/Cargo.toml"
        )
        self.assertEqual(
            devonly["dependencies"]["fixture-leaf"],
            {"path": "../../crates/fixture-leaf"},
        )
        # The verification build ran (the offline ceiling): the packaged top
        # crate compiled against the registry-sourced leaf inside the staged
        # target dir.
        verify_marker = (
            self.output_root / "cargo-target/package/fixture-top-0.2.0/Cargo.toml"
        )
        self.assertTrue(verify_marker.is_file())
        self.assertTrue(result["packaged"]["rows"])

        # The staged bytes are vcs-info-free by law.
        with tarfile.open(Path(rows[1]["crate"]), "r:gz") as archive:
            names = archive.getnames()
        self.assertNotIn("fixture-top-0.2.0/.cargo_vcs_info.json", names)

    def test_verify_fails_closed_on_digest_mismatch(self):
        result = self.run_all_modes()
        leaf_crate = self.output_root / "cargo-target/package/fixture-leaf-0.1.0.crate"
        data = bytearray(leaf_crate.read_bytes())
        data[-1] ^= 0xFF
        leaf_crate.write_bytes(bytes(data))
        # The receipt still carries the original digests: the packaged bytes
        # no longer match the recorded authority.
        with self.assertRaises(SystemExit) as caught:
            self.verify_with(result["packaged"]["rows"])
        self.assertIn("digest mismatch", str(caught.exception))

    def test_verify_fails_closed_on_missing_package(self):
        self.run_all_modes()
        leaf_crate = self.output_root / "cargo-target/package/fixture-leaf-0.1.0.crate"
        leaf_crate.unlink()
        with self.assertRaises(SystemExit) as caught:
            self.verify_with([])
        self.assertIn("packaged crate set differs", str(caught.exception))

    def test_verify_fails_closed_on_membership_drift(self):
        result = self.run_all_modes()
        staged_root_manifest = self.output_root / "workspace/Cargo.toml"
        document = producer.parse_toml(staged_root_manifest)
        document["workspace"]["members"].append("dev-only/fixture-devonly")
        staged_root_manifest.write_text(
            producer.toml_dumps(document), encoding="utf-8", newline="\n"
        )
        with self.assertRaises(SystemExit) as caught:
            self.verify_with(result["packaged"]["rows"])
        self.assertIn("membership differs", str(caught.exception))

    def test_verify_fails_closed_on_packaged_manifest_drift(self):
        result = self.run_all_modes()
        top_crate = self.output_root / "cargo-target/package/fixture-top-0.2.0.crate"
        rewritten = top_crate.with_suffix(".tampered.crate")
        with tarfile.open(top_crate, "r:gz") as source, tarfile.open(
            rewritten, "w:gz"
        ) as target:
            for member in source.getmembers():
                fileobj = source.extractfile(member) if member.isfile() else None
                if member.name == "fixture-top-0.2.0/Cargo.toml" and fileobj:
                    manifest = (
                        fileobj.read()
                        .decode("utf-8")
                        .replace('version = "0.1.0"', 'version = "0.9.9"')
                    )
                    info = tarfile.TarInfo(member.name)
                    info.size = len(manifest.encode("utf-8"))
                    target.addfile(info, io.BytesIO(manifest.encode("utf-8")))
                else:
                    target.addfile(member, fileobj)
        top_crate.unlink()
        rewritten.rename(top_crate)
        # Rebind the receipt authority to the tampered bytes so the digest
        # law passes and the manifest-dependency law itself is exercised.
        rows_state = [dict(row) for row in result["packaged"]["rows"]]
        for row in rows_state:
            crate_path = Path(row["crate"])
            row["crate_digest"] = producer.sha256_file(crate_path)
            row["crate_size_bytes"] = crate_path.stat().st_size
        with self.assertRaises(SystemExit) as caught:
            self.verify_with(rows_state)
        self.assertIn(
            "but the topology selects fixture-leaf '0.1.0'", str(caught.exception)
        )

    def test_verify_fails_closed_on_registry_checksum_drift(self):
        result = self.run_all_modes()
        index = (
            self.output_root
            / "staged-registry"
            / "index"
            / producer.index_rel_path("fixture-leaf")
        )
        if not index.is_file():
            self.fail("fixture registry index entry missing")
        entry = json.loads(index.read_text(encoding="utf-8").strip())
        entry["cksum"] = "0" * 64
        index.write_text(
            json.dumps(entry, separators=(",", ":")) + "\n", encoding="utf-8"
        )
        with self.assertRaises(SystemExit) as caught:
            self.verify_with(result["packaged"]["rows"])
        self.assertIn("registry checksum disagrees", str(caught.exception))

    def test_verify_fails_closed_on_lockfile_membership_drift(self):
        result = self.run_all_modes()
        lock = self.output_root / "workspace/Cargo.lock"
        text = lock.read_text(encoding="utf-8")
        lock.write_text(
            text.replace('name = "fixture-top"', 'name = "fixture-removed"'),
            encoding="utf-8",
        )
        with self.assertRaises(SystemExit) as caught:
            self.verify_with(result["packaged"]["rows"])
        self.assertIn("staged lockfile lacks selected row", str(caught.exception))

    def test_cargo_invocation_boundary_refuses_foreign_commands(self):
        with self.assertRaises(SystemExit):
            producer.run_cargo(
                ["publish", "--dry-run", "-p", "fixture-leaf"], cwd=self.fixture_root
            )
        with self.assertRaises(SystemExit):
            producer.run_cargo(["package", "-p", "fixture-leaf"], cwd=self.fixture_root)

    def test_staging_is_idempotent_and_atomic(self):
        payload = self.payload()
        first = producer.stage_closure(self.fixture_root, payload, self.output_root)
        second = producer.stage_closure(self.fixture_root, payload, self.output_root)
        self.assertEqual(first["staged_members"], second["staged_members"])
        self.assertEqual(
            first["staged_manifest_digests"], second["staged_manifest_digests"]
        )
        self.assertFalse((self.output_root / ".staging").exists())


if __name__ == "__main__":
    unittest.main()
