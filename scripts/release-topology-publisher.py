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
    "cargo-allow": {"shared", "cargo-allow"},
    "all": {"shared", "cargo-intent", "cargo-proof", "cargo-allow"},
}


def fail(message: str) -> NoReturn:
    raise SystemExit(f"release-topology-publisher: error: {message}")


def bounded_reference(value: str, field: str) -> str:
    if not value or len(value) > 200 or any(
        character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._:/#-"
        for character in value
    ):
        fail(f"{field} must be a bounded reference token")
    return value


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
        if mode == "cargo-allow" and raw.get("candidate_inclusion") is not True:
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


def package_workspace(selected: set[str], packages: dict[str, dict[str, Any]]) -> None:
    command = ["cargo", "package", "--workspace", "--locked", "--no-verify"]
    for name in sorted(packages.keys() - selected):
        command.extend(["--exclude", name])
    run(command)


def package_crate(name: str, version: str) -> tuple[Path, str]:
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


def load_recovery_receipt(
    path: Path,
    *,
    mode: str,
    topology: dict[str, Any],
    topology_path: Path,
    authorization: str,
) -> dict[str, Any]:
    try:
        receipt = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"recovery receipt is unreadable or malformed: {error}")
    if not isinstance(receipt, dict):
        fail("recovery receipt must be a JSON object")
    if receipt.get("schema_id") != "cargo-allow.topology-publish-receipt.v1":
        fail("recovery receipt has an unexpected schema")
    if receipt.get("mode") != mode or receipt.get("publish") is not True:
        fail("recovery receipt mode or publication posture does not match the request")
    if receipt.get("authorization", authorization) != authorization:
        fail("recovery receipt authorization differs from the selected authorization")
    if receipt.get("complete") is not False:
        fail("recovery requires an incomplete incident receipt")
    if receipt.get("incident_state") not in {"partial", "release_incident"}:
        fail("recovery receipt does not preserve a publish incident")
    expected = {
        "topology_id": topology["topology_id"],
        "topology_sha256": sha256_text(topology_path),
        "cargo_lock_sha256": sha256_text(ROOT / "Cargo.lock"),
        "commit": git_identity("commit"),
        "tree": git_identity("tree"),
    }
    for field, value in expected.items():
        if receipt.get(field) != value:
            fail(f"recovery receipt {field} differs from the exact candidate")
    rows = receipt.get("rows")
    if not isinstance(rows, list) or not rows:
        fail("recovery receipt has no package-row evidence")
    return receipt


def recovery_rows(receipt: dict[str, Any]) -> dict[tuple[str, str], dict[str, Any]]:
    result: dict[tuple[str, str], dict[str, Any]] = {}
    for row in receipt["rows"]:
        if not isinstance(row, dict):
            fail("recovery receipt contains a malformed package row")
        name = row.get("name")
        version = row.get("version")
        if not isinstance(name, str) or not isinstance(version, str):
            fail("recovery receipt package rows require name and version")
        key = (name, version)
        if key in result:
            fail(f"recovery receipt contains duplicate package row {name} {version}")
        result[key] = row
    return result


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
    parser.add_argument(
        "--package-only",
        action="store_true",
        help="package the selected workspace candidate without registry checks or publication",
    )
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--topology", type=Path, default=DEFAULT_TOPOLOGY)
    parser.add_argument("--receipt", type=Path, default=DEFAULT_RECEIPT)
    parser.add_argument("--authorization", default="")
    parser.add_argument(
        "--recovery-receipt",
        type=Path,
        help="incomplete incident receipt from the exact candidate run",
    )
    args = parser.parse_args()

    if args.package_only and args.publish:
        fail("--package-only cannot be combined with --publish")

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
    authorization = bounded_reference(args.authorization, "authorization") if args.authorization else ""
    if args.publish and not authorization:
        fail("--authorization is required before publication")
    recovery_receipt = None
    prior_rows: dict[tuple[str, str], dict[str, Any]] = {}
    if args.recovery_receipt is not None:
        if not args.publish:
            fail("a recovery receipt requires --publish")
        recovery_receipt = load_recovery_receipt(
            args.recovery_receipt.resolve(),
            mode=args.mode,
            topology=topology,
            topology_path=topology_path,
            authorization=authorization,
        )
        prior_rows = recovery_rows(recovery_receipt)

    packages = cargo_packages()
    validate_rows(rows, packages)
    package_workspace({row["cargo_package_name"] for row in rows}, packages)
    if args.package_only:
        return 0
    receipt: dict[str, Any] = {
        "schema_id": "cargo-allow.topology-publish-receipt.v1",
        "schema_version": 1,
        "mode": args.mode,
        "publish": args.publish,
        "authorization": authorization,
        "topology_id": topology["topology_id"],
        "topology_sha256": sha256_text(topology_path),
        "cargo_lock_sha256": sha256_text(ROOT / "Cargo.lock"),
        "commit": git_identity("commit"),
        "tree": git_identity("tree"),
        "rows": [],
        "complete": False,
        "incident_state": "none",
        "first_irreversible_row": None,
    }
    if recovery_receipt is not None:
        receipt["recovery_receipt"] = "validated"
        receipt["incident_state"] = "partial"
    write_receipt(args.receipt, receipt)

    publish_env = os.environ.copy()
    for row in rows:
        name = row["cargo_package_name"]
        version = row["package_version"]
        crate_path, local_checksum = package_crate(name, version)
        prior = prior_rows.get((name, version))
        if prior is not None:
            if prior.get("local_checksum") != local_checksum:
                fail(f"recovery candidate bytes differ for {name} {version}")
            prior_state = prior.get("state")
            if prior_state in {"verified_existing", "published_verified"}:
                row_receipt = {
                    "logical_id": row["logical_id"],
                    "name": name,
                    "version": version,
                    "family": row["product_family"],
                    "release_order": row["release_order"],
                    "crate": str(crate_path.relative_to(ROOT)),
                    "local_checksum": local_checksum,
                    "registry_checksum": prior.get("registry_checksum"),
                    "state": "recovered_already_published_exact",
                }
                receipt["rows"].append(row_receipt)
                write_receipt(args.receipt, receipt)
                continue
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
                receipt["incident_state"] = "release_incident"
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

        try:
            run(["cargo", "publish", "--dry-run", "-p", name, "--locked"], env=publish_env)
        except SystemExit:
            receipt["incident_state"] = "release_incident"
            write_receipt(args.receipt, receipt)
            raise
        if receipt["first_irreversible_row"] is None:
            receipt["first_irreversible_row"] = row["release_order"]
        write_receipt(args.receipt, receipt)
        try:
            run(["cargo", "publish", "-p", name, "--locked"], env=publish_env)
        except SystemExit:
            receipt["incident_state"] = "partial"
            write_receipt(args.receipt, receipt)
            raise
        published_checksum = sha256_file(crate_path)
        row_receipt["state"] = "uploaded_waiting_for_registry"
        write_receipt(args.receipt, receipt)
        try:
            wait_for_checksum(name, version, published_checksum)
        except SystemExit:
            receipt["incident_state"] = "partial"
            write_receipt(args.receipt, receipt)
            raise
        row_receipt["registry_checksum"] = receipt_checksum(published_checksum)
        row_receipt["state"] = "published_verified"
        write_receipt(args.receipt, receipt)

    receipt["complete"] = True
    write_receipt(args.receipt, receipt)
    print(json.dumps(receipt, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
