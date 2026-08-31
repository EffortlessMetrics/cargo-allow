#!/usr/bin/env python3
"""Derive and verify the topology-selected cargo-allow package candidate.

#2924: selection derives every and only candidate row from the strict V2
package topology (`candidate_inclusion` over the cargo-allow-0.2 and
shared-0.1 version lines), binds the exact repository commit/tree and
Cargo.lock identity, and verifies packaged `.crate` bytes against the typed
expectations. This producer never builds a registry, installs anything,
touches the network, creates tags, or publishes.

Modes:
  derive          render the typed candidate payload from the live tree
  verify-packaged additionally consume packaged `.crate` files, verify each
                  packaged manifest/assets by exact name-version identity,
                  and record digests into the payload before rendering
  check           re-derive and fail if an existing artifact drifted
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - CI runs 3.11+
    import tomli as tomllib  # type: ignore[no-redef]

TOPOLOGY_SCHEMA = "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001"
IDENTITIES_MANIFEST_ID = "CARGO-ALLOW-ARCH-V2-0001"
SELECTED_VERSION_LINES = {"cargo-allow-0.2", "shared-0.1"}
ROOT_LOGICAL_ID = "cargo-allow"
CANDIDATE_PRODUCT_ID = "cargo-allow-0.2"
TARGET_CLASS = "cargo-allow-0.2"
FEATURE_SET_ID = "default"
SCHEMA_ID = "cargo-allow.package-candidate.v2"
SCHEMA_VERSION = 2
CLAIM_BOUNDARY = (
    "Exact topology-selected mixed-version package identity for cargo-allow. "
    "Proves the selected source rows package independently; does not prove "
    "installation, runtime behavior, or publication."
)
LIMITATIONS = [
    "no registry is built and nothing is installed",
    "no publication, tag, upload, or live-control change occurs",
    "resolved installed-graph verification belongs to the next child",
]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(65536), b""):
            digest.update(block)
    return f"sha256:{digest.hexdigest()}"


def sha256_file_lf_normalized(path: Path) -> str:
    """Hash text artifacts over LF-normalized bytes.

    Working-tree checkouts on Windows materialize CRLF for LF-blob files, so
    a raw-byte digest would differ per platform and make the committed
    baseline unreproducible. Normalizing to LF pins the digest to the
    checked-in content regardless of checkout EOLs.
    """
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(65536), b""):
            digest.update(block.replace(b"\r\n", b"\n"))
    return f"sha256:{digest.hexdigest()}"


def git_identity(root: Path) -> tuple[str, str]:
    commit = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=root, check=True, capture_output=True, text=True
    ).stdout.strip()
    tree = subprocess.run(
        ["git", "rev-parse", "HEAD^{tree}"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    return commit, tree


def parse_toml(path: Path) -> dict[str, Any]:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def manifest_table(path: Path, root: Path) -> dict[str, Any]:
    data = parse_toml(path)
    package = data.get("package")
    if not isinstance(package, dict):
        raise ValueError(f"{path} has no [package] table")
    if isinstance(package.get("version"), dict) and package["version"].get("workspace") is True:
        workspace = parse_toml(root / "Cargo.toml")
        inherited = (workspace.get("workspace") or {}).get("package") or {}
        data["package"] = {**package, "version": inherited.get("version")}
    return data


def manifest_package_version(package: dict[str, Any], path: Path) -> str:
    version = package.get("version")
    if not isinstance(version, str) or not version.strip():
        raise ValueError(f"{path} has no resolvable package version")
    return version


def packaged_path_dependency(manifest_path: Path) -> str | None:
    """Return the first dependency table entry carrying a `path` key.

    Only dependency tables are inspected: cargo's own normalized `[lib]`
    path points at in-crate sources and is not a dependency leak.
    """
    data = parse_toml(manifest_path)

    def scan(table: dict[str, Any], section: str) -> str | None:
        for name, spec in (table.get(section) or {}).items():
            if isinstance(spec, dict) and "path" in spec:
                return name
        return None

    for key, value in data.items():
        if key in ("dependencies", "dev-dependencies", "build-dependencies"):
            if isinstance(value, dict):
                found = scan(data, key)
                if found is not None:
                    return found
        elif key.startswith("target.") and isinstance(value, dict):
            for section in ("dependencies", "dev-dependencies", "build-dependencies"):
                if isinstance(value.get(section), dict):
                    found = scan(value, section)
                    if found is not None:
                        return found
    return None


def selected_rows(topology: dict[str, Any]) -> list[dict[str, Any]]:
    rows = [
        row
        for row in topology.get("package", [])
        if row.get("candidate_inclusion") is True
        and row.get("version_line") in SELECTED_VERSION_LINES
    ]
    if not rows:
        raise ValueError("topology selected no candidate rows")
    return rows


def dependency_rows(
    manifest: dict[str, Any],
    internal_names: set[str],
    topology_by_name: dict[str, dict[str, Any]] | None = None,
) -> list[dict[str, str]]:
    def declared_version(name: str, spec_version: str) -> str:
        if name in internal_names and topology_by_name and name in topology_by_name:
            exact = topology_by_name[name].get("package_version")
            if isinstance(exact, str) and exact.strip():
                return exact
        return spec_version

    rows: list[dict[str, str]] = []
    for section in ("dependencies", "build-dependencies"):
        for name, spec in (manifest.get(section) or {}).items():
            if isinstance(spec, dict):
                version = spec.get("version", "")
                workspace = spec.get("workspace")
                if workspace is True:
                    # Workspace-inherited dependency specs are resolved for
                    # the packaged manifest by cargo itself; the candidate
                    # carries the declared workspace dependency name only.
                    version = "workspace"
            else:
                version = spec
            kind = "internal" if name in internal_names else "external"
            if kind == "external" and topology_by_name:
                excluded = topology_by_name.get(name)
                if (
                    isinstance(excluded, dict)
                    and excluded.get("candidate_inclusion") is False
                    and str(excluded.get("publication_state", "")).startswith(
                        "Unpublished"
                    )
                    and excluded.get("publish") is True
                ):
                    raise ValueError(
                        f"dependency {name} is an unpublished topology package "
                        "that is absent from the candidate"
                    )
            rows.append(
                {
                    "package_name": name,
                    "package_version": declared_version(name, str(version).lstrip("=")),
                    "dependency_kind": kind,
                }
            )
    rows.sort(key=lambda row: (row["package_name"], row["package_version"]))
    return rows


def derive_payload(
    root: Path, topology_path: Path, identities_path: Path
) -> dict[str, Any]:
    topology = parse_toml(topology_path)
    if topology.get("topology_id") != TOPOLOGY_SCHEMA:
        raise ValueError("unexpected package topology identity")
    identities = parse_toml(identities_path)
    if identities.get("manifest_id") != IDENTITIES_MANIFEST_ID:
        raise ValueError("unexpected crate identity architecture manifest")
    identity_by_id = {
        row["logical_id"]: row
        for row in identities.get("crate_identity", [])
        if isinstance(row, dict) and row.get("logical_id")
    }
    rows_topology = selected_rows(topology)
    internal_names = {row["cargo_package_name"] for row in rows_topology}
    topology_by_name = {
        row["cargo_package_name"]: row
        for row in topology.get("package", [])
        if isinstance(row, dict) and row.get("cargo_package_name")
    }

    rendered_rows = []
    for row in rows_topology:
        identity = identity_by_id.get(row["logical_id"])
        if not isinstance(identity, dict):
            raise ValueError(
                f"topology row {row['logical_id']} has no crate-identity row"
            )
        manifest_path = root / identity["workspace_path"] / "Cargo.toml"
        manifest = manifest_table(manifest_path, root)
        package = manifest.get("package", {})
        name = package.get("name")
        version = manifest_package_version(package, manifest_path)
        if name != row["cargo_package_name"]:
            raise ValueError(
                f"manifest name {name!r} disagrees with topology {row['cargo_package_name']!r}"
            )
        expected_version = row["package_version"]
        if version != expected_version:
            raise ValueError(
                f"manifest version {version!r} disagrees with topology "
                f"{expected_version!r} for {name}"
            )
        lib = manifest.get("lib") or {}
        library_name = lib.get("name") or str(name).replace("-", "_")
        assets = [
            asset
            for asset in (row.get("asset_roots") or [])
            if isinstance(asset, str) and asset.strip()
        ]
        rendered_rows.append(
            {
                "logical_id": row["logical_id"],
                "cargo_package_name": name,
                "cargo_package_version": version,
                "rust_library_name": library_name,
                "workspace_source_path": identity["workspace_path"],
                "product_family": row["version_line"],
                "publication_state": row["publication_state"],
                "publish": bool(row.get("publish", False)),
                "support_tier": row["support_tier"],
                "release_order": row["release_order"],
                "selected_features": list(row.get("features") or []),
                "expected_manifest_identity": f"{name}:{version}",
                "expected_dependency_rows": dependency_rows(
                    manifest, internal_names, topology_by_name
                ),
                "required_assets": assets,
            }
        )
    rendered_rows.sort(key=lambda row: row["release_order"])

    root_row = next(
        (row for row in rendered_rows if row["logical_id"] == ROOT_LOGICAL_ID), None
    )
    if root_row is None:
        raise ValueError("candidate is missing its root package row")

    # Topology asset_roots are workspace directories the package owns for
    # release-asset qualification; they must exist in the source tree. They
    # are not asserted inside the .crate, which cargo builds from the crate
    # directory alone.
    for row in rendered_rows:
        for asset in row["required_assets"]:
            if not (root / asset).exists():
                raise ValueError(
                    f"required asset root {asset} is missing from the workspace "
                    f"for {row['cargo_package_name']}"
                )

    known_exclusions = sorted(
        f"{row['logical_id']}: {row['version_line']} is not selected for this candidate"
        for row in topology.get("package", [])
        if row.get("logical_id") not in {r["logical_id"] for r in rendered_rows}
    )

    return {
        "schema_id": SCHEMA_ID,
        "schema_version": SCHEMA_VERSION,
        "topology_id": topology["topology_id"],
        "topology_digest": sha256_file_lf_normalized(topology_path),
        "repository_commit": git_identity(root)[0],
        "repository_tree": git_identity(root)[1],
        "cargo_lock_digest": sha256_file_lf_normalized(root / "Cargo.lock"),
        "candidate_product_id": CANDIDATE_PRODUCT_ID,
        "root_logical_id": ROOT_LOGICAL_ID,
        "root_package_name": root_row["cargo_package_name"],
        "root_package_version": root_row["cargo_package_version"],
        "target_class": TARGET_CLASS,
        "feature_set_id": FEATURE_SET_ID,
        "rows": rendered_rows,
        "known_exclusions": known_exclusions,
        "limitations": list(LIMITATIONS),
        "claim_boundary": CLAIM_BOUNDARY,
    }


def verify_packaged(payload: dict[str, Any], package_dir: Path) -> None:
    """Verify packaged `.crate` bytes against the typed rows, in place."""
    for row in payload["rows"]:
        expected_file = package_dir / (
            f"{row['cargo_package_name']}-{row['cargo_package_version']}.crate"
        )
        if not expected_file.is_file():
            raise ValueError(
                f"packaged crate missing for {row['cargo_package_name']}: "
                f"expected {expected_file.name} (exact name-version identity)"
            )
        row["crate_digest"] = sha256_file(expected_file)
        row["crate_size_bytes"] = expected_file.stat().st_size
        with tempfile.TemporaryDirectory() as directory:
            inspection = Path(directory)
            with tarfile.open(expected_file, "r:gz") as archive:
                archive.extractall(inspection, filter="data")
            unpacked = inspection / f"{row['cargo_package_name']}-{row['cargo_package_version']}"
            if not unpacked.is_dir():
                raise ValueError(
                    f"crate {expected_file.name} did not unpack to its exact name-version root"
                )
            leak = packaged_path_dependency(unpacked / "Cargo.toml")
            if leak is not None:
                raise ValueError(
                    f"packaged manifest for {row['cargo_package_name']} leaks a "
                    f"path dependency on {leak!r}"
                )
            packaged_manifest = parse_toml(unpacked / "Cargo.toml")
            package_table = packaged_manifest.get("package") or {}
            readme = package_table.get("readme")
            if isinstance(readme, str) and readme.strip():
                if not (unpacked / readme).is_file():
                    raise ValueError(
                        f"packaged crate {row['cargo_package_name']} is missing "
                        f"its declared readme {readme!r}"
                    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--mode",
        choices=("derive", "verify-packaged", "check", "baseline"),
        default="derive",
    )
    parser.add_argument(
        "--baseline",
        type=Path,
        default=Path("docs/dogfood/receipts/package-candidate-v2.example.json"),
    )
    parser.add_argument("--workspace-root", type=Path, default=Path.cwd())
    parser.add_argument(
        "--topology",
        type=Path,
        default=Path("policy/product-package-topology-v2.toml"),
    )
    parser.add_argument(
        "--identities",
        type=Path,
        default=Path("policy/product-crates-v2.toml"),
    )
    parser.add_argument("--output", type=Path, default=Path("target/cargo-allow/package-candidate-v2.json"))
    parser.add_argument("--package-dir", type=Path, default=Path("target/package"))
    args = parser.parse_args()

    root = args.workspace_root.resolve()
    topology_path = (
        args.topology if args.topology.is_absolute() else root / args.topology
    )
    identities_path = (
        args.identities if args.identities.is_absolute() else root / args.identities
    )
    payload = derive_payload(root, topology_path, identities_path)

    if args.mode == "baseline":
        if not args.baseline.is_file():
            print(f"baseline missing at {args.baseline}", file=sys.stderr)
            return 1
        baseline = json.loads(
            (args.baseline if args.baseline.is_absolute() else root / args.baseline)
            .read_text(encoding="utf-8")
        )
        volatile = {
            "repository_commit",
            "repository_tree",
            "crate_digest",
            "crate_size_bytes",
        }

        def strip_volatile(node: Any) -> Any:
            if isinstance(node, dict):
                return {
                    key: strip_volatile(value)
                    for key, value in node.items()
                    if key not in volatile
                }
            if isinstance(node, list):
                return [strip_volatile(item) for item in node]
            return node

        if strip_volatile(payload) != strip_volatile(baseline):
            print(
                "candidate derivation drifted from the committed baseline",
                file=sys.stderr,
            )
            return 1
        print("candidate derivation matches the committed baseline")
        return 0
    if args.mode in ("verify-packaged", "check"):
        if args.output.is_file():
            existing = json.loads(args.output.read_text(encoding="utf-8"))
            if args.mode == "check":
                fresh = dict(payload)
                if existing.get("rows"):
                    for row, prior in zip(fresh["rows"], existing["rows"]):
                        if prior.get("crate_digest"):
                            row["crate_digest"] = prior.get("crate_digest")
                            row["crate_size_bytes"] = prior.get("crate_size_bytes")
                if existing != fresh:
                    print("candidate artifact drifted from the current derivation", file=sys.stderr)
                    return 1
                print("candidate artifact matches the current derivation")
                return 0
        if args.mode == "check":
            print(f"candidate artifact missing at {args.output}", file=sys.stderr)
            return 1
        verify_packaged(payload, args.package_dir if args.package_dir.is_absolute() else root / args.package_dir)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(f"candidate rows: {len(payload['rows'])} -> {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
