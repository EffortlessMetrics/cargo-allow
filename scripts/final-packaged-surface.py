#!/usr/bin/env python3
"""Reconcile packaged Cargo archives into a bounded surface receipt.

The package-set harness owns selection and isolation.  This helper owns only
the bytes that Cargo actually produced, so source manifests cannot satisfy a
package-content claim by accident.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import tarfile
import tomllib
from pathlib import Path, PurePosixPath

from exact_candidate_package_identity import crate_version_from_filename


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def safe_member(name: str) -> bool:
    path = PurePosixPath(name)
    return not path.is_absolute() and ".." not in path.parts


def asset_record(files: dict[str, bytes], prefix: str, candidates: list[str]) -> dict[str, object]:
    for candidate in candidates:
        path = prefix + candidate
        if path in files:
            return {"path": path, "sha256": sha256(files[path]), "present": True}
    return {"path": None, "sha256": None, "present": False}


def surface(
    crate: Path, expected_name: str, expected_version: str
) -> dict[str, object]:
    version = crate_version_from_filename(expected_name, crate.name)
    if version != expected_version:
        raise ValueError(f"archive identity mismatch: {crate.name}")
    stem = expected_name

    with tarfile.open(crate, "r:gz") as archive:
        members = archive.getmembers()
        names = sorted(member.name for member in members)
        unsafe = [member.name for member in members if not safe_member(member.name)]
        if unsafe:
            raise ValueError(f"unsafe archive paths: {unsafe}")
        files = {
            member.name: archive.extractfile(member).read()
            for member in members
            if member.isfile()
        }

    prefix = f"{stem}-{version}/"
    manifest_name = prefix + "Cargo.toml"
    manifest = files.get(manifest_name)
    if manifest is None:
        raise ValueError(f"missing packaged manifest: {crate.name}")

    manifest_data = tomllib.loads(manifest.decode("utf-8"))
    package = manifest_data.get("package", {})
    if package.get("name") != stem or package.get("version") != version:
        raise ValueError(f"manifest identity mismatch: {crate.name}")
    dependencies: dict[str, dict[str, object]] = {}
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        for dependency, requirement in manifest_data.get(section, {}).items():
            if isinstance(requirement, dict):
                if any(key in requirement for key in ("path", "git", "workspace")):
                    raise ValueError(
                        f"unresolved source dependency {dependency} in {crate.name}"
                    )
                requirement = requirement.get("version")
            dependencies.setdefault(section, {})[dependency] = requirement

    readme = package.get("readme", "README.md")
    if not isinstance(readme, str):
        raise ValueError(f"invalid packaged readme declaration: {crate.name}")
    assets = {"readme": asset_record(files, prefix, [readme])}
    license_file = package.get("license-file")
    license_candidates = (
        [license_file]
        if isinstance(license_file, str)
        else ["LICENSE", "LICENSE-MIT", "LICENSE-APACHE"]
    )
    assets["license"] = asset_record(files, prefix, license_candidates)

    return {
        "name": stem,
        "version": version,
        "crate_file": crate.name,
        "size_bytes": crate.stat().st_size,
        "sha256": sha256(crate.read_bytes()),
        "manifest": {"path": manifest_name, "sha256": sha256(manifest)},
        "metadata": {
            "package": {
                field: package.get(field)
                for field in (
                    "name", "version", "edition", "rust-version", "license",
                    "license-file", "repository", "homepage", "documentation",
                    "readme", "keywords", "categories",
                )
                if field in package
            },
            "features": manifest_data.get("features", {}),
            "dependencies": dependencies,
        },
        "file_list": {"sha256": sha256("\n".join(names).encode()), "paths": names},
        "assets": assets,
        "result": "Complete" if all(item["present"] for item in assets.values()) else "Incomplete",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package-set-receipt", type=Path, required=True)
    parser.add_argument("--packages-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expected-version", required=False)
    args = parser.parse_args()

    package_set = json.loads(args.package_set_receipt.read_text(encoding="utf-8"))
    expected_rows = package_set["package_set"]["crates"]
    expected = [row["name"] for row in expected_rows]
    expected_versions = {row["name"]: row["version"] for row in expected_rows}
    expected_by_file = {
        f"{row["name"]}-{row["version"]}.crate": row["name"]
        for row in expected_rows
    }
    archives = sorted(args.packages_dir.glob("*.crate"))
    by_name = {}
    for archive in archives:
        archive_name = expected_by_file.get(archive.name)
        if archive_name is None:
            raise ValueError(f"unexpected packaged crate: {archive.name}")
        row = surface(
            archive, archive_name, expected_versions[archive_name]
        )
        if row["name"] in by_name:
            raise ValueError(f"duplicate packaged crate: {row['name']}")
        by_name[row["name"]] = row
    if set(by_name) != set(expected):
        raise ValueError(f"package set mismatch: expected {expected}, got {sorted(by_name)}")
    rows = [by_name[name] for name in expected]
    if args.expected_version:
        candidate_version = package_set.get("candidate", {}).get("workspace_version")
        if candidate_version and candidate_version != args.expected_version:
            raise ValueError("candidate workspace version does not match requested version")
    result = "Complete" if all(row["result"] == "Complete" for row in rows) else "Incomplete"
    receipt = {
        "schema_id": "cargo-allow.final-packaged-surface.v1",
        "schema_version": 1,
        "result": result,
        "candidate": package_set.get("candidate", {}),
        "package_set": {"order": expected, "packages": rows},
        "claim_boundary": ["actual_crate_bytes", "normalized_file_list", "declared_readme_and_license_assets"],
        "limitations": ["does_not_prove_registry_state", "does_not_execute_relocated_docs_or_doctests"],
    }
    args.output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
