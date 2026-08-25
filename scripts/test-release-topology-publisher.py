#!/usr/bin/env python3
"""Deterministic checksum contract tests for the topology publisher."""

from __future__ import annotations

from contextlib import redirect_stdout
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
from typing import Any, Callable


ROOT = Path(__file__).resolve().parent.parent
PUBLISHER_PATH = ROOT / "scripts/release-topology-publisher.py"
SPEC = importlib.util.spec_from_file_location("release_topology_publisher", PUBLISHER_PATH)
if SPEC is None or SPEC.loader is None:
    raise SystemExit("could not load release topology publisher")
PUBLISHER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PUBLISHER)

DIGEST = "a" * 64
CANONICAL = f"sha256:{DIGEST}"


def expect_failure(action: Callable[[], Any]) -> None:
    try:
        action()
    except SystemExit:
        return
    raise AssertionError("expected checksum validation failure")


def shared_fixture_rows() -> list[dict[str, Any]]:
    return [
        {
            "logical_id": f"shared-{index}",
            "cargo_package_name": f"effortless-fixture-{index}",
            "package_version": "0.1.0",
            "product_family": "shared",
            "release_order": index,
        }
        for index in (80, 85, 90)
    ]


def exercise_shared_registry_preflight() -> None:
    original = PUBLISHER.registry_checksum
    original_api = PUBLISHER.crate_api
    original_package = PUBLISHER.package_crate
    original_write = PUBLISHER.write_receipt
    try:
        PUBLISHER.package_crate = lambda name, version: (
            ROOT / f"target/package/{name}-{version}.crate",
            DIGEST,
        )
        PUBLISHER.write_receipt = lambda _path, _receipt: None
        receipt = {"incident_state": "none"}
        calls: list[str] = []
        PUBLISHER.registry_checksum = lambda name, _version: calls.append(name) or DIGEST
        PUBLISHER.shared_registry_preflight(
            shared_fixture_rows(), publish=True, receipt=receipt, receipt_path=ROOT / "unused"
        )
        assert calls == [row["cargo_package_name"] for row in shared_fixture_rows()]
        assert receipt["shared_registry_preflight_complete"] is True
        assert all(item["state"] == "already_published_exact" for item in receipt["shared_registry_preflight"])

        # Expected checksum matching
        rows_with_expected = [
            dict(r, expected_registry_checksum=CANONICAL) for r in shared_fixture_rows()
        ]
        receipt = {"incident_state": "none"}
        PUBLISHER.shared_registry_preflight(
            rows_with_expected, publish=True, receipt=receipt, receipt_path=ROOT / "unused"
        )
        assert all(item["state"] == "already_published_exact" for item in receipt["shared_registry_preflight"])

        # Expected checksum conflict fails closed
        rows_with_conflict = [
            dict(r, expected_registry_checksum="sha256:" + ("b" * 64)) for r in shared_fixture_rows()
        ]
        expect_failure(
            lambda: PUBLISHER.shared_registry_preflight(
                rows_with_conflict, publish=True, receipt={"incident_state": "none"}, receipt_path=ROOT / "unused"
            )
        )

        PUBLISHER.registry_checksum = lambda _name, _version: None
        expect_failure(
            lambda: PUBLISHER.shared_registry_preflight(
                shared_fixture_rows(), publish=True, receipt={"incident_state": "none"}, receipt_path=ROOT / "unused"
            )
        )

        PUBLISHER.registry_checksum = original
        PUBLISHER.crate_api = lambda _name, _version: []
        expect_failure(lambda: PUBLISHER.registry_checksum("fixture", "0.1.0"))
        PUBLISHER.crate_api = lambda _name, _version: {"version": {}}
        expect_failure(lambda: PUBLISHER.registry_checksum("fixture", "0.1.0"))
        PUBLISHER.crate_api = lambda _name, _version: {
            "version": {"num": "0.1.1", "checksum": DIGEST}
        }
        expect_failure(lambda: PUBLISHER.registry_checksum("fixture", "0.1.0"))
    finally:
        PUBLISHER.registry_checksum = original
        PUBLISHER.crate_api = original_api
        PUBLISHER.package_crate = original_package
        PUBLISHER.write_receipt = original_write


def exercise_preflight_schema_contract() -> None:
    schema = json.loads(
        (ROOT / "docs/schemas/topology-publish-receipt.schema.json").read_text(
            encoding="utf-8"
        )
    )
    items = schema["properties"]["shared_registry_preflight"]["items"]
    branches = items["oneOf"]
    assert any(
        branch["properties"]["state"].get("const") == "missing"
        and branch["properties"]["registry_checksum"].get("type") == "null"
        for branch in branches
    )
    assert any(
        branch["properties"]["state"].get("enum")
        == ["already_published_exact", "checksum_conflict"]
        and branch["properties"]["registry_checksum"].get("type") == "string"
        for branch in branches
    )


def exercise_shared_topology_contract() -> None:
    """Keep the shared rehearsal bound to the four V2 package identities.

    The cargo-allow release candidate intentionally overlaps only three shared
    packages.  The standalone shared rehearsal must remain a distinct,
    complete four-package candidate, including the source-index package that
    is not part of the cargo-allow install closure.
    """
    topology, rows = PUBLISHER.load_rows(PUBLISHER.DEFAULT_TOPOLOGY, "shared")
    assert topology["topology_id"]
    assert [row["cargo_package_name"] for row in rows] == [
        "effortless-repo-protocol",
        "effortless-repo-snapshot",
        "effortless-repo-edit",
        "effortless-rust-source-index",
    ]
    assert [row["release_order"] for row in rows] == [80, 85, 90, 230]
    assert all(row["product_family"] == "shared" for row in rows)
    assert all(row["package_version"] == "0.1.0" for row in rows)

    cargo_allow_rows = PUBLISHER.load_rows(PUBLISHER.DEFAULT_TOPOLOGY, "cargo-allow")[1]
    assert "effortless-rust-source-index" not in {
        row["cargo_package_name"] for row in cargo_allow_rows
    }


def row(state: str, *, local: str, registry: str | None) -> dict[str, Any]:
    return PUBLISHER.receipt_row(
        {
            "logical_id": "fixture-package",
            "cargo_package_name": "fixture-package",
            "package_version": "9.9.9",
            "product_family": "cargo-allow",
            "release_order": 1,
        },
        crate_path=ROOT / "target/package/fixture-package-9.9.9.crate",
        local_checksum=local,
        registry_checksum=registry,
        state=state,
    )


def assert_root_schema_accepts(schema_path: Path, artifact_path: Path) -> dict[str, Any]:
    schema = json.loads(schema_path.read_text(encoding="utf-8"))
    artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
    required = set(schema.get("required", []))
    missing = required - artifact.keys()
    assert not missing, f"artifact misses required schema properties: {sorted(missing)}"
    properties = schema.get("properties", {})
    if schema.get("additionalProperties") is False:
        unexpected = artifact.keys() - properties.keys()
        assert not unexpected, f"artifact has undeclared schema properties: {sorted(unexpected)}"
    for name, rules in properties.items():
        if name not in artifact:
            continue
        if "const" in rules:
            assert artifact[name] == rules["const"], f"{name} violates schema const"
        if "enum" in rules:
            assert artifact[name] in rules["enum"], f"{name} violates schema enum"
    return artifact


def exercise_main_receipt_shapes() -> None:
    original = {
        name: getattr(PUBLISHER, name)
        for name in (
            "cargo_packages",
            "git_identity",
            "load_rows",
            "package_crate",
            "package_workspace",
            "registry_checksum",
            "sha256_text",
            "validate_rows",
            "shared_registry_preflight",
        )
    }
    original_argv = sys.argv
    def fixture_rows(mode: str) -> list[dict[str, Any]]:
        if mode == "shared":
            return [
                {
                    "logical_id": f"shared-{index}",
                    "cargo_package_name": f"effortless-fixture-{index}",
                    "package_version": "0.1.0",
                    "product_family": "shared",
                    "release_order": index,
                }
                for index in range(1, 5)
            ]
        return [
            {
                "logical_id": "fixture-package",
                "cargo_package_name": "fixture-package",
                "package_version": "9.9.9",
                "product_family": "cargo-allow",
                "release_order": 1,
            }
        ]

    try:
        PUBLISHER.cargo_packages = lambda: {
            "fixture-package": {},
            **{f"effortless-fixture-{index}": {} for index in range(1, 5)},
        }
        PUBLISHER.git_identity = lambda _kind: DIGEST
        PUBLISHER.load_rows = lambda _path, mode: (
            {"topology_id": "fixture-topology"},
            fixture_rows(mode),
        )
        PUBLISHER.package_crate = lambda name, version: (
            ROOT / f"target/package/{name}-{version}.crate",
            DIGEST,
        )
        PUBLISHER.package_workspace = lambda _selected, _packages: None
        PUBLISHER.registry_checksum = lambda _name, _version: None
        PUBLISHER.sha256_text = lambda _path: DIGEST
        PUBLISHER.validate_rows = lambda _rows, _packages: None
        PUBLISHER.shared_registry_preflight = lambda *_args, **_kwargs: None

        with tempfile.TemporaryDirectory() as directory:
            receipt = Path(directory) / "topology.json"
            sys.argv = [
                str(PUBLISHER_PATH),
                "--mode",
                "cargo-allow",
                "--receipt",
                str(receipt),
            ]
            with redirect_stdout(io.StringIO()):
                assert PUBLISHER.main() == 0
            topology = assert_root_schema_accepts(
                ROOT / "docs/schemas/topology-publish-receipt.schema.json", receipt
            )
            assert "package_only" not in topology

            sys.argv = [
                str(PUBLISHER_PATH),
                "--mode",
                "shared",
                "--package-only",
                "--receipt",
                str(receipt),
            ]
            with redirect_stdout(io.StringIO()):
                assert PUBLISHER.main() == 0
            shared = assert_root_schema_accepts(
                ROOT / "docs/schemas/shared-package-candidate.v1.schema.json", receipt
            )
            assert shared["package_only"] is True
    finally:
        sys.argv = original_argv
        for name, value in original.items():
            setattr(PUBLISHER, name, value)


def exercise_cargo_allow_checksum_equality() -> None:
    original = {
        name: getattr(PUBLISHER, name)
        for name in (
            "cargo_packages",
            "git_identity",
            "load_rows",
            "package_crate",
            "package_workspace",
            "registry_checksum",
            "sha256_text",
            "validate_rows",
            "shared_registry_preflight",
            "run",
            "wait_for_checksum",
        )
    }
    original_argv = sys.argv
    try:
        PUBLISHER.cargo_packages = lambda: {"cargo-allow": {}}
        PUBLISHER.git_identity = lambda _kind: DIGEST
        PUBLISHER.load_rows = lambda _path, mode: (
            {"topology_id": "fixture-topology"},
            [
                {
                    "logical_id": "cargo-allow",
                    "cargo_package_name": "cargo-allow",
                    "package_version": "0.2.0-rc.1",
                    "product_family": "cargo-allow",
                    "release_order": 100,
                }
            ],
        )
        PUBLISHER.package_crate = lambda name, version: (
            ROOT / f"target/package/{name}-{version}.crate",
            DIGEST,
        )
        PUBLISHER.package_workspace = lambda _selected, _packages: None
        PUBLISHER.sha256_text = lambda _path: DIGEST
        PUBLISHER.validate_rows = lambda _rows, _packages: None
        PUBLISHER.shared_registry_preflight = lambda *_args, **_kwargs: None
        PUBLISHER.run = lambda *args, **kwargs: ""

        # Existing row with matching checksum -> verified_existing
        PUBLISHER.registry_checksum = lambda _name, _version: DIGEST
        with tempfile.TemporaryDirectory() as directory:
            receipt = Path(directory) / "topology.json"
            sys.argv = [
                str(PUBLISHER_PATH),
                "--mode",
                "cargo-allow",
                "--receipt",
                str(receipt),
            ]
            with redirect_stdout(io.StringIO()):
                assert PUBLISHER.main() == 0
            data = json.loads(receipt.read_text(encoding="utf-8"))
            assert data["rows"][0]["state"] == "verified_existing"

        # Existing row with conflicting checksum -> fails closed
        PUBLISHER.registry_checksum = lambda _name, _version: "b" * 64
        with tempfile.TemporaryDirectory() as directory:
            receipt = Path(directory) / "topology.json"
            sys.argv = [
                str(PUBLISHER_PATH),
                "--mode",
                "cargo-allow",
                "--receipt",
                str(receipt),
            ]
            expect_failure(lambda: PUBLISHER.main())

        # Newly published row with conflicting post-upload checksum -> fails closed
        PUBLISHER.registry_checksum = lambda _name, _version: None
        PUBLISHER.wait_for_checksum = lambda _name, _version: "b" * 64
        with tempfile.TemporaryDirectory() as directory:
            receipt = Path(directory) / "topology.json"
            sys.argv = [
                str(PUBLISHER_PATH),
                "--mode",
                "cargo-allow",
                "--publish",
                "--authorization",
                "issue:3760",
                "--receipt",
                str(receipt),
            ]
            expect_failure(lambda: PUBLISHER.main())
    finally:
        sys.argv = original_argv
        for name, value in original.items():
            setattr(PUBLISHER, name, value)


def main() -> None:
    assert PUBLISHER.receipt_checksum(DIGEST, field="fresh local checksum") == CANONICAL
    assert PUBLISHER.receipt_checksum(CANONICAL, field="published registry checksum") == CANONICAL
    assert PUBLISHER.receipt_checksum(None, field="missing registry checksum") is None

    fresh = row("missing", local=DIGEST, registry=None)
    published = row("published_verified", local=DIGEST, registry=DIGEST)
    recovered = row("recovered_already_published_exact", local=CANONICAL, registry=CANONICAL)
    for receipt_row in (fresh, published, recovered):
        assert receipt_row["local_checksum"] == CANONICAL
        if receipt_row["registry_checksum"] is not None:
            assert receipt_row["registry_checksum"] == CANONICAL

    accepted = PUBLISHER.recovery_rows({"rows": [recovered]})
    assert accepted[("fixture-package", "9.9.9")]["local_checksum"] == CANONICAL
    prior = dict(recovered)
    prior["state"] = "published_verified"
    assert PUBLISHER.recovery_row_is_exact(prior, CANONICAL)

    for registry_checksum in (None, "sha256:" + ("b" * 64)):
        incomplete = dict(prior)
        incomplete["registry_checksum"] = registry_checksum
        assert not PUBLISHER.recovery_row_is_exact(incomplete, CANONICAL)

    for malformed in (
        DIGEST,
        "sha256:" + ("A" * 64),
        "sha256:" + ("a" * 63),
        "sha256:sha256:" + DIGEST,
    ):
        invalid = dict(recovered)
        invalid["local_checksum"] = malformed
        expect_failure(lambda invalid=invalid: PUBLISHER.recovery_rows({"rows": [invalid]}))

    exercise_main_receipt_shapes()
    exercise_shared_registry_preflight()
    exercise_preflight_schema_contract()
    exercise_shared_topology_contract()
    exercise_cargo_allow_checksum_equality()
    source = PUBLISHER_PATH.read_text(encoding="utf-8")
    main_start = source.index("def main()")
    assert source.index("shared_registry_preflight(", main_start) < source.index(
        'run(["cargo", "publish"', main_start
    )

    print("topology publisher checksum contract: passed")


if __name__ == "__main__":
    main()
