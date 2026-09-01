#!/usr/bin/env python3
"""Fail-closed fixture tests for the final package-docs receipt (#3773)."""

from __future__ import annotations

import hashlib
import io
import json
import importlib.util
from pathlib import Path
import sys
import tarfile
import tempfile
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
TOOL_PATH = ROOT / "scripts/final-package-docs.py"
SPEC = importlib.util.spec_from_file_location("final_package_docs", TOOL_PATH)
if SPEC is None or SPEC.loader is None:
    raise SystemExit("could not load final-package-docs producer")
TOOL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TOOL)

CARGO_ROWS = [
    ("allow-core", 10, []),
    ("allow-policy", 20, ["allow-core"]),
    ("allow-policy-legacy", 30, ["allow-policy"]),
    ("allow-inventory", 40, []),
    ("allow-files", 50, []),
    ("allow-rust", 60, []),
    ("allow-match", 70, []),
    ("allow-report", 80, ["allow-core"]),
    ("allow-diff", 90, ["allow-core"]),
    ("cargo-allow", 100, ["allow-core", "allow-report"]),
]
SHARED_ROWS = [
    ("effortless-repo-protocol", 110, "sha256:" + "1" * 64),
    ("effortless-repo-edit", 120, "sha256:" + "2" * 64),
    ("effortless-repo-snapshot", 130, "sha256:" + "3" * 64),
]
FINAL_IDENTITY = {
    "version": "0.2.0",
    "tag": "v0.2.0",
    "channel": "stable",
    "github_prerelease": False,
}


def fixture_basis(version: str = "0.2.0", tag: str = "v0.2.0",
                  channel: str = "stable", prerelease: bool = False) -> dict[str, Any]:
    return {
        "schema": "cargo-allow.final-package-docs-basis.v1",
        "commit": "a" * 40,
        "tree": "b" * 40,
        "cargo_lock_sha256": "sha256:" + "c" * 64,
        "topology_sha256": TOOL.sha256_file(TOOL.TOPLOGY),
        "release_identity": {
            "version": version,
            "tag": tag,
            "channel": channel,
            "github_prerelease": prerelease,
        },
    }


def normalized_manifest(name: str, version: str) -> str:
    internal = CARGO_ROW_DEPS.get(name, [])
    lines = [
        "[package]",
        f'name = "{name}"',
        f'version = "{version}"',
        'edition = "2024"',
        'rust-version = "1.95"',
        'license = "MIT OR Apache-2.0"',
        'repository = "https://github.com/EffortlessMetrics/cargo-allow"',
        "",
        "[dependencies]",
    ]
    for dep in internal:
        lines.append(f'{dep} = {{ version = "={version}" }}')
    for shared, _, _ in SHARED_ROWS:
        if name == "cargo-allow":
            lines.append(f'{shared} = {{ version = "0.1.0" }}')
    lines.append("")
    return "\n".join(lines)


CARGO_ROW_DEPS = {name: deps for name, _, deps in CARGO_ROWS}


def write_fixture_crate(directory: Path, name: str, version: str,
                        *, drop_readme: bool = False) -> Path:
    root = f"{name}-{version}/"
    crate = directory / f"{name}-{version}.crate"
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
        def add(member: str, payload: bytes) -> None:
            info = tarfile.TarInfo(member)
            info.size = len(payload)
            archive.addfile(info, io.BytesIO(payload))

        add(root + "Cargo.toml", normalized_manifest(name, version).encode("utf-8"))
        if not drop_readme:
            add(root + "README.md", f"# {name}\n".encode("utf-8"))
        source = "src/lib.rs"
        payload = f"//! {name} docs.\n".encode("utf-8")
        if name == "allow-policy-legacy":
            payload = b"//! Compatibility and migration posture.\n"
        if name == "cargo-allow":
            source = "src/main.rs"
            payload = b"//! cargo-allow CLI product.\nfn main() {}\n"
        add(root + source, payload)
    crate.write_bytes(buffer.getvalue())
    return crate


def stub_packaging(rows: list[dict[str, Any]], directory: Path,
                   *, drop_readme_for: str | None = None) -> None:
    crates_by_name = {row["cargo_package_name"]: row for row in rows
                      if row["product_family"] == "cargo-allow"}
    all_names = {row["cargo_package_name"] for row in rows}
    packages_dir = ROOT / "target/package"
    packages_dir.mkdir(parents=True, exist_ok=True)

    def cargo_packages() -> dict[str, dict[str, Any]]:
        return {name: {"version": "0.2.0", "publish": True} for name in all_names}

    def validate_rows(_rows: list[dict[str, Any]],
                      _packages: dict[str, dict[str, Any]]) -> None:
        return None

    def package_workspace(selected: set[str],
                          _packages: dict[str, dict[str, Any]]) -> None:
        for name in selected:
            row = crates_by_name[name]
            drop = drop_readme_for is not None and name == drop_readme_for
            crate = write_fixture_crate(packages_dir, name, row["package_version"],
                                        drop_readme=drop)
            assert crate.is_file()

    TOOL.PUBLISHER.cargo_packages = cargo_packages
    TOOL.PUBLISHER.validate_rows = validate_rows
    TOOL.PUBLISHER.package_workspace = package_workspace
    TOOL.PUBLISHER.load_rows = lambda _path, mode: (
        {"topology_id": "fixture"},
        [dict(row) for row in rows],
    )


class Harness:
    def __init__(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.work = Path(self.directory.name)
        self.originals = {
            name: getattr(TOOL.PUBLISHER, name)
            for name in ("package_workspace", "load_rows")
        }

    def cleanup(self) -> None:
        for name, value in self.originals.items():
            setattr(TOOL.PUBLISHER, name, value)
        self.directory.cleanup()

    def run(self, basis: dict[str, Any], rows: list[dict[str, Any]],
            *, drop_readme_for: str | None = None) -> tuple[int, dict[str, Any] | None]:
        basis_path = self.work / "basis.json"
        receipt_path = self.work / "receipt.json"
        basis_path.write_text(json.dumps(basis), encoding="utf-8")
        if receipt_path.exists():
            receipt_path.unlink()
        stub_packaging(rows, self.work, drop_readme_for=drop_readme_for)
        code = 0
        error: str | None = None
        try:
            TOOL.build_receipt(basis_path, receipt_path, skip_package=False)
        except SystemExit as exited:
            code = exited.code if isinstance(exited.code, int) else 1
            error = str(exited)
        receipt = None
        if receipt_path.is_file():
            receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        return code, receipt if code == 0 else None, error


def expect_failure(harness: Harness, basis: dict[str, Any],
                   rows: list[dict[str, Any]], needle: str,
                   *, drop_readme_for: str | None = None) -> None:
    code, _receipt, error = harness.run(basis, rows, drop_readme_for=drop_readme_for)
    assert code != 0, f"expected failure containing {needle!r}, got success"
    assert needle in (error or ""), f"failure {error!r} missing {needle!r}"


def full_rows(version: str = "0.2.0") -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for name, order, _deps in CARGO_ROWS:
        rows.append({
            "logical_id": name,
            "cargo_package_name": name,
            "package_version": version,
            "product_family": "cargo-allow",
            "release_order": order,
        })
    for name, order, checksum in SHARED_ROWS:
        rows.append({
            "logical_id": name,
            "cargo_package_name": name,
            "package_version": "0.1.0",
            "product_family": "shared",
            "release_order": order,
            "expected_registry_checksum": checksum,
        })
    return rows


def main() -> None:
    harness = Harness()
    try:
        rows = full_rows()
        code, receipt, error = harness.run(fixture_basis(), rows)
        assert code == 0 and receipt is not None, error or "happy path failed"
        assert len(receipt["rows"]) == 10
        assert all(row["result"] == "Complete" for row in receipt["rows"])
        assert all(row["docs_posture"] in
                   {"product_cli", "library", "compatibility_migration"}
                   for row in receipt["rows"])
        assert receipt["rc_line_inputs_excluded"] is True
        assert [row["version"] for row in receipt["rows"]] == ["0.2.0"] * 10
        assert [row["name"] for row in receipt["shared_prerequisites"]] == [
            name for name, _, _ in SHARED_ROWS
        ]
        print("ok final receipt proves the ten-package candidate")

        expect_failure(harness, fixture_basis(version="0.2.0-rc.1", tag="v0.2.0-rc.1",
                                              channel="release_candidate", prerelease=True),
                       full_rows(), "is not a stable x.y.z identity")
        expect_failure(harness, fixture_basis(version="0.2.0", tag="v0.2.0",
                                              channel="stable", prerelease=True),
                       full_rows(), "rc-line or prerelease basis")
        print("ok rc-line basis rejected as final authority")

        stale_rows = full_rows()
        stale_rows[0]["package_version"] = "0.2.0-rc.1"
        expect_failure(harness, fixture_basis(), stale_rows, "not the selected final identity")
        print("ok stale candidate row rejected against the final basis")

        moved_rows = full_rows()
        for row in moved_rows:
            if row["cargo_package_name"] == "effortless-repo-edit":
                row["package_version"] = "0.2.0"
        expect_failure(harness, fixture_basis(), moved_rows, "moved off the independently published 0.1.0 line")
        print("ok shared prerequisite moved onto the final line is rejected")

        stale_basis = fixture_basis()
        stale_basis["topology_sha256"] = "sha256:" + "9" * 64
        expect_failure(harness, stale_basis, full_rows(), "does not match the basis")
        print("ok stale topology basis rejected")

        malformed = fixture_basis()
        malformed["cargo_lock_sha256"] = "sha256:short"
        expect_failure(harness, malformed, full_rows(), "canonical sha256 digest")
        print("ok malformed basis digest rejected")

        missing_readme_rows = full_rows()
        code, receipt, error = harness.run(
            fixture_basis(), missing_readme_rows, drop_readme_for="allow-core"
        )
        assert code != 0, "missing README must fail"
        print("ok missing packaged README rejected")
    finally:
        harness.cleanup()
    print("final-package-docs fixture contract: passed")


if __name__ == "__main__":
    main()
