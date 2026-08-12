#!/usr/bin/env python3
"""Deterministic checksum contract tests for the topology publisher."""

from __future__ import annotations

import importlib.util
from pathlib import Path
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

    print("topology publisher checksum contract: passed")


if __name__ == "__main__":
    main()
