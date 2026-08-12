#!/usr/bin/env python3
"""Fail-closed crates.io publisher derived from the V2 package topology.

The script never accepts a package name or version from the workflow. It reads
policy/product-package-topology-v2.toml, validates selected rows against Cargo
metadata, packages each exact crate, compares crates.io's checksum when an exact
version already exists, and publishes only missing rows in dependency order.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - supported by Python 3.11+
    try:
        import tomli as tomllib
    except ModuleNotFoundError as error:
        raise SystemExit(
            "release-topology-publisher requires Python 3.11+ or the tomli package"
        ) from error
from typing import Any, NoReturn
from urllib.error import HTTPError, URLError
from urllib.parse import quote
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_TOPOLOGY = ROOT / "policy/product-package-topology-v2.toml"
DEFAULT_RECEIPT = ROOT / "target/cargo-allow/topology-publish.receipt.json"
FAMILY_MODES = {
    "namespace": {"shared", "cargo-intent", "cargo-proof"},
    "cargo-allow": {"cargo-allow"},
    "all": {"shared", "cargo-intent", "cargo-proof", "cargo-allow"},
}


def fail(message: str) -> NoReturn:
    raise SystemExit(f"release-topology-publisher: error: {message}")


def run(command: list[str], *, env: dict[str, str] | None = None) -> str:
    result = subprocess.run(
        command,
        cwd=ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    if result.returncode != 0:
        sys.stderr.write(result.stdout)
        fail(f"command failed ({result.returncode}): {' '.join(command)}")
    return result.stdout


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sha256_text(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git_identity(kind: str) -> str:
    return run(["git", "rev-parse", f"HEAD^{{{kind}}}"]).strip()


def load_rows(topology_path: Path, mode: str) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    with topology_path.open("rb") as handle:
        topology = tomllib.load(handle)
    if topology.get("schema_version") != "2.0" or topology.get("authority_generation") != 2:
        fail("topology is not the selected V2 authority")
    families = FAMILY_MODES[mode]
    rows: list[dict[str, Any]] = []
    for raw in topology.get("package", []):
        if raw.get("product_family") not in families or raw.get("publish") is not True:
            continue
        required = {
            "logical_id",
            "cargo_package_name",
            "product_family",
            "package_version",
            "publication_state",
            "release_order",
        }
        missing = sorted(required - raw.keys())
        if missing:
            fail(f"topology row is missing fields {missing}: {raw}")
        row = dict(raw)
        row["release_order"] = int(row["release_order"])
        rows.append(row)
    rows.sort(key=lambda row: (row["release_order"], row["cargo_package_name"]))
    names = [row["cargo_package_name"] for row in rows]
    orders = [row["release_order"] for row in rows]
    if not rows:
        fail(f"topology selected no rows for mode {mode}")
    if len(names) != len(set(names)):
        fail(f"topology selected duplicate package names for mode {mode}")
    if len(orders) != len(set(orders)):
        fail(f"topology selected duplicate release_order values for mode {mode}")
    return topology, rows


def cargo_packages() -> dict[str, dict[str, Any]]:
    metadata = json.loads(run(["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"]))
    return {package["name"]: package for package in metadata["packages"]}


def validate_rows(rows: list[dict[str, Any]], packages: dict[str, dict[str, Any]]) -> None:
    selected = {row["cargo_package_name"]: row for row in rows}
    order = {name: row["release_order"] for name, row in selected.items()}
    for name, row in selected.items():
        package = packages.get(name)
        if package is None:
            fail(f"topology package {name} is absent from Cargo metadata")
        if package["version"] != row["package_version"]:
            fail(
                f"{name} topology version {row['package_version']} differs from Cargo metadata {package['version']}"
            )
        publish = package.get("publish")
        if isinstance(publish, list) and "crates-io" not in publish:
            fail(f"{name} is selected for publication but its manifest does not allow crates.io")
        for dependency in package.get("dependencies", []):
            if dependency.get("kind") == "dev":
                continue
            dependency_name = dependency["name"]
            dependency_package = packages.get(dependency_name)
            if (
                dependency_package is not None
                and dependency_package.get("source") is None
                and dependency_name not in selected
            ):
                fail(
                    f"publication selection for {name} is not closed: "
                    f"workspace dependency {dependency_name} is not selected"
                )
            if dependency_name not in selected:
                continue
            if order[dependency_name] >= order[name]:
                fail(
                    f"release order is invalid: {name} ({order[name]}) depends on "
                    f"{dependency_name} ({order[dependency_name]})"
                )


def crate_api(name: str, version: str) -> dict[str, Any] | None:
    url = f"https://crates.io/api/v1/crates/{quote(name, safe='')}/{quote(version, safe='')}"
    request = Request(url, headers={"User-Agent": "cargo-allow-release-controller/0.2"})
    for attempt in range(1, 4):
        try:
            with urlopen(request, timeout=30) as response:
                return json.loads(response.read().decode("utf-8"))
        except HTTPError as error:
            if error.code == 404:
                return None
            transient = error.code == 429 or 500 <= error.code <= 599
            if not transient or attempt == 3:
                fail(f"crates.io returned HTTP {error.code} for {name} {version}")
            print(
                f"transient crates.io HTTP {error.code} for {name} {version}; "
                f"retrying ({attempt}/3)",
                file=sys.stderr,
            )
        except URLError as error:
            if attempt == 3:
                fail(f"crates.io lookup failed for {name} {version}: {error}")
            print(
                f"transient crates.io lookup failure for {name} {version}; "
                f"retrying ({attempt}/3): {error}",
                file=sys.stderr,
            )
        time.sleep(attempt * 2)
    fail(f"crates.io lookup exhausted retries for {name} {version}")


def registry_checksum(name: str, version: str) -> str | None:
    payload = crate_api(name, version)
    if payload is None:
        return None
    version_payload = payload.get("version") or {}
    checksum = version_payload.get("checksum")
    if not isinstance(checksum, str) or len(checksum) != 64:
        fail(f"crates.io returned no bounded checksum for {name} {version}")
    return checksum


def receipt_checksum(value: str | None) -> str | None:
    return None if value is None else f"sha256:{value}"


def package_crate(name: str, version: str) -> tuple[Path, str]:
    run(["cargo", "package", "-p", name, "--locked", "--no-verify"])
    crate_path = ROOT / "target/package" / f"{name}-{version}.crate"
    if not crate_path.is_file():
        fail(f"cargo package did not create {crate_path.relative_to(ROOT)}")
    return crate_path, sha256_file(crate_path)


def write_receipt(path: Path, receipt: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile("w", encoding="utf-8", dir=path.parent, delete=False) as handle:
        json.dump(receipt, handle, indent=2, sort_keys=True)
        handle.write("\n")
        temporary = Path(handle.name)
    temporary.replace(path)


def wait_for_checksum(name: str, version: str, expected: str) -> None:
    for attempt in range(1, 31):
        observed = registry_checksum(name, version)
        if observed is None:
            print(f"waiting for {name} {version} registry visibility ({attempt}/30)")
            time.sleep(10)
            continue
        if observed != expected:
            fail(
                f"registry checksum conflict for {name} {version}: expected {expected}, observed {observed}"
            )
        return
    fail(f"timed out waiting for {name} {version} in crates.io")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=sorted(FAMILY_MODES), required=True)
    parser.add_argument("--publish", action="store_true")
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--topology", type=Path, default=DEFAULT_TOPOLOGY)
    parser.add_argument("--receipt", type=Path, default=DEFAULT_RECEIPT)
    args = parser.parse_args()

    topology_path = args.topology.resolve()
    topology, rows = load_rows(topology_path, args.mode)
    if args.list:
        for row in rows:
            print(
                f"{row['release_order']}\t{row['product_family']}\t"
                f"{row['cargo_package_name']}\t{row['package_version']}"
            )
        return 0

    token = os.environ.get("CARGO_REGISTRY_TOKEN", "")
    if args.publish and not token:
        fail("CARGO_REGISTRY_TOKEN is required before the first upload")

    packages = cargo_packages()
    validate_rows(rows, packages)
    receipt: dict[str, Any] = {
        "schema_id": "cargo-allow.topology-publish-receipt.v1",
        "schema_version": 1,
        "mode": args.mode,
        "publish": args.publish,
        "topology_id": topology["topology_id"],
        "topology_sha256": sha256_text(topology_path),
        "cargo_lock_sha256": sha256_text(ROOT / "Cargo.lock"),
        "commit": git_identity("commit"),
        "tree": git_identity("tree"),
        "rows": [],
        "complete": False,
    }
    write_receipt(args.receipt, receipt)

    publish_env = os.environ.copy()
    for row in rows:
        name = row["cargo_package_name"]
        version = row["package_version"]
        crate_path, local_checksum = package_crate(name, version)
        observed = registry_checksum(name, version)
        row_receipt: dict[str, Any] = {
            "logical_id": row["logical_id"],
            "name": name,
            "version": version,
            "family": row["product_family"],
            "release_order": row["release_order"],
            "crate": str(crate_path.relative_to(ROOT)),
            "local_checksum": local_checksum,
            "registry_checksum": receipt_checksum(observed),
            "state": "missing" if observed is None else "already_visible",
        }
        receipt["rows"].append(row_receipt)
        write_receipt(args.receipt, receipt)

        if observed is not None:
            if observed != local_checksum:
                row_receipt["state"] = "checksum_conflict"
                write_receipt(args.receipt, receipt)
                fail(
                    f"registry checksum conflict for {name} {version}: "
                    f"local {local_checksum}, registry {observed}"
                )
            row_receipt["state"] = "verified_existing"
            write_receipt(args.receipt, receipt)
            continue

        if not args.publish:
            continue

        run(["cargo", "publish", "--dry-run", "-p", name, "--locked"], env=publish_env)
        run(["cargo", "publish", "-p", name, "--locked"], env=publish_env)
        published_checksum = sha256_file(crate_path)
        row_receipt["state"] = "uploaded_waiting_for_registry"
        write_receipt(args.receipt, receipt)
        wait_for_checksum(name, version, published_checksum)
        row_receipt["registry_checksum"] = receipt_checksum(published_checksum)
        row_receipt["state"] = "published_verified"
        write_receipt(args.receipt, receipt)

    receipt["complete"] = True
    write_receipt(args.receipt, receipt)
    print(json.dumps(receipt, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
