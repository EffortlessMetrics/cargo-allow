#!/usr/bin/env python3
"""Checked final package-documentation receipt for the ten-package candidate.

Consumes one exact prospective final basis (commit/tree, lock digest,
topology digest, stable ReleaseIdentityV1) and produces
``cargo-allow.final-package-docs.v1``: per-package normalized-manifest and
packaged-file audits, docs-posture checks, and the rc-line exclusion that
keeps 0.2.0-rc.1 evidence from becoming final authority (#3773).

The final basis must be the stable 0.2.0 identity; an rc-line basis is
rejected as a required negative control. Packaging runs offline through the
publisher's ``cargo package --locked --no-verify`` seam and never uploads.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys
import tarfile
import tempfile
from typing import Any

import importlib.util

ROOT = Path(__file__).resolve().parent.parent
BASIS_SCHEMA = "cargo-allow.final-package-docs-basis.v1"
RECEIPT_SCHEMA = "cargo-allow.final-package-docs.v1"
TOPLOGY = ROOT / "policy/product-package-topology-v2.toml"


def _load_publisher():
    spec = importlib.util.spec_from_file_location(
        "release_topology_publisher", ROOT / "scripts/release-topology-publisher.py"
    )
    if spec is None or spec.loader is None:
        raise SystemExit("final-package-docs: could not load the topology publisher")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# Publisher functions (packaging, row selection, checksums) are reused from
# the one upload-capable authority; tests stub attributes on this module.
PUBLISHER = _load_publisher()


def fail(message: str) -> None:
    raise SystemExit(f"final-package-docs: error: {message}")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1 << 20), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def sha256_text(text: str) -> str:
    return "sha256:" + hashlib.sha256(text.encode("utf-8")).hexdigest()


def is_hex_identity(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) in (40, 64)
        and all(char in "0123456789abcdef" for char in value)
    )


def load_basis(path: Path) -> dict[str, Any]:
    try:
        basis = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"basis unreadable: {error}")
    if not isinstance(basis, dict) or basis.get("schema") != BASIS_SCHEMA:
        fail(f"basis schema must be {BASIS_SCHEMA}")
    for field in ("commit", "tree"):
        if not is_hex_identity(basis.get(field)):
            fail(f"basis {field} is not a canonical hex identity")
    for field in ("cargo_lock_sha256", "topology_sha256"):
        value = basis.get(field)
        if (
            not isinstance(value, str)
            or not value.startswith("sha256:")
            or len(value) != 71
            or any(char not in "0123456789abcdef" for char in value[7:])
        ):
            fail(f"basis {field} is not a canonical sha256 digest")
    identity = basis.get("release_identity")
    if not isinstance(identity, dict):
        fail("basis release_identity is missing")
    version = identity.get("version")
    tag = identity.get("tag")
    channel = identity.get("channel")
    prerelease = identity.get("github_prerelease")
    if not isinstance(version, str) or version.count(".") != 2 or not version.replace(".", "").isdigit():
        fail(f"basis version {version!r} is not a stable x.y.z identity")
    if tag != f"v{version}":
        fail(f"basis tag {tag!r} does not match the canonical tag v{version}")
    if channel != "stable" or prerelease is not False:
        fail(
            "rc-line or prerelease basis cannot be the final package-docs "
            "authority: 0.2.0-rc.1 evidence stays incident-lineage history "
            "under #3759 and the final basis must be stable 0.2.0"
        )
    return basis


def load_selected_rows(publisher: Any) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    _topology, rows = publisher.load_rows(TOPLOGY, "cargo-allow")
    cargo_allow = [row for row in rows if row["product_family"] == "cargo-allow"]
    shared = [row for row in rows if row["product_family"] == "shared"]
    if len(cargo_allow) != 10:
        fail(f"expected ten cargo-allow candidate rows, found {len(cargo_allow)}")
    if len(shared) != 3:
        fail(f"expected three selected shared prerequisite rows, found {len(shared)}")
    for row in shared:
        if row["package_version"] != "0.1.0":
            fail(
                f"shared prerequisite {row['cargo_package_name']} moved off the "
                f"independently published 0.1.0 line: {row['package_version']}"
            )
        expected = row.get("expected_registry_checksum")
        if not isinstance(expected, str) or not expected.startswith("sha256:"):
            fail(f"shared prerequisite {row['cargo_package_name']} lacks retained expected checksum")
    return cargo_allow, shared


def bind_basis_to_rows(basis: dict[str, Any], cargo_allow: list[dict[str, Any]]) -> None:
    version = basis["release_identity"]["version"]
    for row in cargo_allow:
        if row["package_version"] != version:
            fail(
                f"candidate row {row['cargo_package_name']} is {row['package_version']}, "
                f"not the selected final identity {version}"
            )


def bind_topology_digest(basis: dict[str, Any]) -> None:
    actual = sha256_file(TOPLOGY)
    if actual != basis["topology_sha256"]:
        fail(f"topology digest {actual} does not match the basis {basis['topology_sha256']}")


def parse_normalized_manifest(text: str) -> dict[str, Any]:
    """Parse the normalized packaged manifest without regex support."""
    package: dict[str, str] = {}
    dependencies: list[tuple[str, str]] = []
    section = ""
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if stripped.startswith("[") and stripped.endswith("]"):
            section = stripped[1:-1]
            continue
        if section == "package" and "=" in stripped:
            key, _, value = stripped.partition("=")
            package[key.strip()] = value.strip().strip('"')
        elif section in ("dependencies", "dev-dependencies", "build-dependencies") and "=" in stripped:
            name = stripped.split("=", 1)[0].strip()
            if "version =" in stripped:
                requirement = stripped.split('version = "', 1)[1].split('"', 1)[0]
            else:
                requirement = stripped.split("=", 1)[1].strip().strip('"')
            dependencies.append((name, requirement))
            for leak_key in ("path =", "git ="):
                if leak_key in stripped:
                    fail(
                        f"normalized manifest kept a checkout-relative source on "
                        f"{name}: {leak_key.strip()}"
                    )
    return {"package": package, "dependencies": dependencies}


def audit_dependencies(name: str, version: str, dependencies: list[tuple[str, str]]) -> None:
    for dep_name, requirement in dependencies:
        lowered = dep_name.lower()
        if lowered.startswith("allow-"):
            if requirement != f"={version}":
                fail(
                    f"{name}: internal requirement {dep_name} {requirement} is not "
                    f"the exact ={version} final line"
                )
        elif lowered.startswith(("effortless-", "intent-", "proof-")):
            if requirement != "0.1.0":
                fail(
                    f"{name}: shared/experimental prerequisite {dep_name} moved off "
                    f"the independently published 0.1.0 line: {requirement}"
                )


def docs_posture(name: str, lib_rs: str, main_rs: str | None) -> str:
    haystack = (main_rs if name == "cargo-allow" else lib_rs) or ""
    documented = any(
        line.strip().startswith("//!") for line in haystack.splitlines()
    )
    if not documented:
        fail(f"{name}: crate docs carry no library-level documentation header")
    lowered = haystack.lower()
    if name == "cargo-allow":
        return "product_cli"
    if name == "allow-policy-legacy":
        if "compatib" not in lowered and "migration" not in lowered:
            fail(
                "allow-policy-legacy docs must keep the compatibility/migration "
                "posture visible"
            )
        return "compatibility_migration"
    return "library"


def audit_packaged_crate(
    publisher: Any, row: dict[str, Any], crate_path: Path
) -> dict[str, Any]:
    """Audit one .crate: normalized manifest law, docs posture, digests."""
    crate_bytes = crate_path.read_bytes()
    with tarfile.open(fileobj=io_bytes_io(crate_bytes)) as archive:
        members = archive.getmembers()
        file_names = sorted(member.name for member in members)
        normalized = None
        readme_present = False
        lib_rs = None
        main_rs = None
        root_prefix = f"{row['cargo_package_name']}-{row['package_version']}/"
        for member in members:
            if member.name == root_prefix + "Cargo.toml":
                normalized = archive.extractfile(member).read().decode("utf-8")
            if member.name == root_prefix + "README.md":
                readme_present = True
            if member.name == root_prefix + "src/lib.rs":
                lib_rs = archive.extractfile(member).read().decode("utf-8")
            if member.name == root_prefix + "src/main.rs":
                main_rs = archive.extractfile(member).read().decode("utf-8")
    if normalized is None:
        fail(f"{row['cargo_package_name']}: packaged crate has no normalized Cargo.toml")
    manifest = parse_normalized_manifest(normalized)
    package = manifest["package"]
    if package.get("name") != row["cargo_package_name"]:
        fail(
            f"normalized manifest name {package.get('name')!r} disagrees with "
            f"{row['cargo_package_name']}"
        )
    if package.get("version") != row["package_version"]:
        fail(
            f"normalized manifest version {package.get('version')!r} disagrees with "
            f"{row['package_version']!r}"
        )
    for field in ("edition", "rust-version", "license", "repository"):
        if not package.get(field):
            fail(f"{row['cargo_package_name']}: normalized manifest lacks {field}")
    audit_dependencies(row["cargo_package_name"], row["package_version"], manifest["dependencies"])
    posture = docs_posture(row["cargo_package_name"], lib_rs, main_rs)
    file_list_digest = sha256_text("\n".join(file_names) + "\n")
    if not readme_present:
        fail(f"{row['cargo_package_name']}: packaged crate lacks README.md")
    return {
        "name": row["cargo_package_name"],
        "version": row["package_version"],
        "release_order": row["release_order"],
        "normalized_manifest_sha256": sha256_text(normalized),
        "file_list_digest": file_list_digest,
        "crate_sha256": sha256_file(crate_path),
        "size_bytes": crate_path.stat().st_size,
        "readme_present": readme_present,
        "docs_posture": posture,
        "result": "Complete",
    }


def io_bytes_io(payload: bytes):
    import io

    return io.BytesIO(payload)


def audit_channel_law() -> dict[str, Any]:
    getting_started = (ROOT / "docs/getting-started.md").read_text(encoding="utf-8")
    support_matrix = (ROOT / "docs/support-matrix.toml").read_text(encoding="utf-8")
    for identity in ("0.1.11", "0.2.0-rc.1", "0.2.0"):
        if identity not in getting_started:
            fail(f"getting-started lost the {identity} channel identity")
    if "cargo-allow" not in support_matrix or "supported" not in support_matrix:
        fail("support matrix lost the cargo-allow support posture")
    return {
        "getting_started": "docs/getting-started.md",
        "support_matrix": "docs/support-matrix.toml",
        "channel_identities": ["0.1.11", "0.2.0-rc.1", "0.2.0"],
    }


def build_receipt(basis_path: Path, receipt_path: Path, skip_package: bool) -> dict[str, Any]:
    publisher = PUBLISHER
    basis = load_basis(basis_path)
    bind_topology_digest(basis)
    cargo_allow, shared = load_selected_rows(publisher)
    bind_basis_to_rows(basis, cargo_allow)

    packages_dir = ROOT / "target/package"
    if not skip_package:
        packages = publisher.cargo_packages()
        # The publication-closure law runs over the full 13-row candidate
        # closure: the allow family reaches the three shared prerequisites,
        # so validating only the ten product rows would misreport closure.
        publisher.validate_rows(cargo_allow + shared, packages)
        publisher.package_workspace(
            {row["cargo_package_name"] for row in cargo_allow}, packages
        )

    rows: list[dict[str, Any]] = []
    for row in cargo_allow:
        crate_path = packages_dir / f"{row['cargo_package_name']}-{row['package_version']}.crate"
        if not crate_path.is_file():
            fail(f"packaged crate missing: {crate_path.name}")
        rows.append(audit_packaged_crate(publisher, row, crate_path))

    channel = audit_channel_law()
    receipt = {
        "schema": RECEIPT_SCHEMA,
        "basis": {
            "commit": basis["commit"],
            "tree": basis["tree"],
            "cargo_lock_sha256": basis["cargo_lock_sha256"],
            "topology_sha256": basis["topology_sha256"],
            "release_identity": basis["release_identity"],
        },
        "rows": sorted(rows, key=lambda row: row["release_order"]),
        "shared_prerequisites": [
            {
                "name": row["cargo_package_name"],
                "version": row["package_version"],
                "expected_registry_checksum": row["expected_registry_checksum"],
            }
            for row in shared
        ],
        "rc_line_inputs_excluded": True,
        "channel_law": channel,
        "limitations": [
            "normalized-manifest audit runs through offline packaging",
            "relocated docs.rs execution is deferred to the installed-experience lanes",
            "install-channel wording remains governed by the getting-started contract tests",
        ],
        "claim_boundary": (
            "Docs/package audit for the exact final candidate basis; binds the "
            "ten cargo-allow rows and the three shared prerequisites while "
            "excluding rc.1 inputs as final authority. Publication itself "
            "stays gated by #3760/#2502."
        ),
    }
    receipt_path.parent.mkdir(parents=True, exist_ok=True)
    receipt_path.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return receipt


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser(
        description="Checked final package-documentation receipt (#3773)"
    )
    parser.add_argument("--basis", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument(
        "--skip-package",
        action="store_true",
        help="reuse target/package artifacts instead of repackaging",
    )
    args = parser.parse_args()
    receipt = build_receipt(args.basis, args.receipt, args.skip_package)
    print(f"final-package-docs: receipt rows {len(receipt['rows'])}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
