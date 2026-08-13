#!/usr/bin/env python3
"""Validate exact-candidate package and install-journey receipts.

This validator intentionally uses only the Python standard library so the
hosted package-smoke lane does not acquire a schema-validation dependency.
It implements the JSON Schema keywords used by the two receipt contracts and
then applies the cross-receipt invariants that JSON Schema cannot express.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import tomllib
from pathlib import Path
from typing import Any


EXPECTED_NEGATIVES = {
    "decisive_install_source_checkout_denied": "CheckoutIsolated",
    "omit_candidate_from_local_registry": "PackageMissing",
    "older_internal_package_version": "InternalVersionConflict",
}
EXPECTED_JOURNEY_STEPS = {
    "audit_with_finding",
    "policy_rollback_after_prune",
}
EXPECTED_ARTIFACT_SCHEMAS = {
    "cargo-allow.doctor.v1",
    "cargo-allow.report.v1",
    "cargo-allow.list.v1",
    "cargo-allow.refresh.v1",
    "cargo-allow.prune.v1",
}


def fail(message: str) -> None:
    raise SystemExit(f"exact-candidate-receipt: {message}")


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"could not read JSON {path}: {error}")
    if not isinstance(value, dict):
        fail(f"receipt must be an object: {path}")
    return value


def schema_type(value: Any, expected: str) -> bool:
    return {
        "object": isinstance(value, dict),
        "array": isinstance(value, list),
        "string": isinstance(value, str),
        "integer": isinstance(value, int) and not isinstance(value, bool),
        "number": isinstance(value, (int, float)) and not isinstance(value, bool),
        "boolean": isinstance(value, bool),
        "null": value is None,
    }.get(expected, True)


def validate_schema(value: Any, schema: dict[str, Any], path: str = "$") -> None:
    if "const" in schema and value != schema["const"]:
        fail(f"{path}: expected const {schema['const']!r}, got {value!r}")
    if "enum" in schema and value not in schema["enum"]:
        fail(f"{path}: value {value!r} is not in enum")

    expected_type = schema.get("type")
    if expected_type is not None:
        types = expected_type if isinstance(expected_type, list) else [expected_type]
        if not any(schema_type(value, item) for item in types):
            fail(f"{path}: expected type {expected_type!r}")

    if isinstance(value, dict):
        required = schema.get("required", [])
        for key in required:
            if key not in value:
                fail(f"{path}: missing required property {key!r}")
        properties = schema.get("properties", {})
        if schema.get("additionalProperties") is False:
            unknown = sorted(set(value) - set(properties))
            if unknown:
                fail(f"{path}: unexpected properties {unknown!r}")
        for key, child_schema in properties.items():
            if key in value:
                validate_schema(value[key], child_schema, f"{path}.{key}")

    if isinstance(value, list):
        if "minItems" in schema and len(value) < schema["minItems"]:
            fail(f"{path}: expected at least {schema['minItems']} items")
        if "maxItems" in schema and len(value) > schema["maxItems"]:
            fail(f"{path}: expected at most {schema['maxItems']} items")
        item_schema = schema.get("items")
        if isinstance(item_schema, dict):
            for index, item in enumerate(value):
                validate_schema(item, item_schema, f"{path}[{index}]")

    if isinstance(value, str):
        if "minLength" in schema and len(value) < schema["minLength"]:
            fail(f"{path}: string is too short")
        pattern = schema.get("pattern")
        if pattern is not None and re.fullmatch(pattern, value) is None:
            fail(f"{path}: string does not match {pattern!r}")

    if isinstance(value, (int, float)) and not isinstance(value, bool):
        if "minimum" in schema and value < schema["minimum"]:
            fail(f"{path}: number is below minimum")


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def expected_crates(fixture_path: Path) -> list[str]:
    fixture = tomllib.loads(fixture_path.read_text(encoding="utf-8"))
    crates = fixture.get("crates")
    if not isinstance(crates, list) or not all(isinstance(item, str) for item in crates):
        fail(f"candidate fixture has no string crates list: {fixture_path}")
    if not crates:
        fail(f"candidate fixture must contain at least one crate: {fixture_path}")
    return crates


def negative_map(receipt: dict[str, Any], path: str) -> dict[str, dict[str, Any]]:
    values = receipt.get("negative_controls")
    if not isinstance(values, list):
        fail(f"{path}.negative_controls must be an array")
    result: dict[str, dict[str, Any]] = {}
    for item in values:
        if isinstance(item, dict) and isinstance(item.get("id"), str):
            result[item["id"]] = item
    return result


def validate_package_receipt(
    receipt_path: Path, schema_path: Path, fixture_path: Path
) -> dict[str, Any]:
    receipt = load_json(receipt_path)
    schema = load_json(schema_path)
    validate_schema(receipt, schema, "package_receipt")
    crates = expected_crates(fixture_path)
    if receipt.get("result") != "Passed":
        fail("package receipt is not Passed")
    if receipt.get("candidate", {}).get("crate_set_schema_id") != "cargo-allow.candidate-crate-set.v1":
        fail("package receipt has the wrong crate-set schema id")
    order = receipt.get("package_set", {}).get("order")
    if order != crates:
        fail(f"package receipt order does not match canonical fixture: {order!r}")
    rows = receipt.get("package_set", {}).get("crates")
    if not isinstance(rows, list) or len(rows) != len(crates):
        fail("package receipt does not contain all canonical crate rows")
    if receipt.get("isolation", {}).get("source_checkout_denied") is not True:
        fail("package receipt does not prove source-checkout denial")
    if receipt.get("install", {}).get("method") != "cargo_install_path_extracted_with_local_registry":
        fail("package receipt did not use the extracted local-registry install")
    negatives = negative_map(receipt, "package_receipt")
    for identifier, classification in EXPECTED_NEGATIVES.items():
        item = negatives.get(identifier)
        if item is None or item.get("passed") is not True:
            fail(f"package receipt missing passed negative {identifier}")
        if item.get("result_class") != classification:
            fail(
                f"package negative {identifier} classified as {item.get('result_class')!r}, "
                f"expected {classification!r}"
            )
    return receipt


def validate_journey_receipt(receipt_path: Path, schema_path: Path) -> dict[str, Any]:
    receipt = load_json(receipt_path)
    schema = load_json(schema_path)
    validate_schema(receipt, schema, "journey_receipt")
    if receipt.get("result") != "Passed":
        fail("journey receipt is not Passed")
    return receipt


def validate_final_receipt(
    receipt_path: Path, schema_path: Path, fixture_path: Path
) -> dict[str, Any]:
    receipt = validate_journey_receipt(receipt_path, schema_path)
    crates = expected_crates(fixture_path)
    candidate = receipt.get("candidate", {})
    provenance = receipt.get("provenance", {})
    if candidate.get("crate_count") != len(crates):
        fail("final receipt crate count is not fixture-derived")
    if provenance.get("crate_order") != crates:
        fail("final receipt crate order does not match canonical fixture")
    for key in (
        "package_set_receipt_sha256",
        "journey_receipt_sha256",
        "candidate_fixture_sha256",
    ):
        if not re.fullmatch(r"[0-9a-f]{64}", str(provenance.get(key, ""))):
            fail(f"final receipt has invalid digest at provenance.{key}")
    if provenance.get("candidate_fixture_sha256") != sha256_file(fixture_path):
        fail("final receipt fixture digest does not match the fixture on disk")
    if receipt.get("install", {}).get("source_checkout_denied") is not True:
        fail("final receipt does not prove source-checkout denial")
    if receipt.get("install", {}).get("source_hidden_journey_passed") is not True:
        fail("final receipt does not prove source-hidden journey")
    if receipt.get("install", {}).get("no_undeclared_source_reads") is not True:
        fail("final receipt does not prove no undeclared source reads")
    journey = receipt.get("journey", {})
    if not EXPECTED_JOURNEY_STEPS.issubset(set(journey.get("steps_expected", []))):
        fail("final receipt omitted the finding or rollback journey step")
    artifact_ids = set(journey.get("artifact_schema_ids", []))
    if not EXPECTED_ARTIFACT_SCHEMAS.issubset(artifact_ids):
        fail("final receipt omitted a validated journey artifact schema")
    negatives = negative_map(receipt, "final_receipt")
    required_final_negatives = {
        "source_checkout_denied_during_exact_install": "CheckoutIsolated",
        "source_checkout_read_after_install_rejected": "CheckoutIsolated",
        "missing_candidate_sibling_rejected": "PackageMissing",
        "wrong_candidate_sibling_version_rejected": "InternalVersionConflict",
    }
    for identifier, classification in required_final_negatives.items():
        item = negatives.get(identifier)
        if item is None or item.get("passed") is not True:
            fail(f"final receipt missing passed negative {identifier}")
        if item.get("result_class") != classification:
            fail(f"final negative {identifier} has the wrong result class")
    cleanup = receipt.get("cleanup", {})
    for key in (
        "temporary_consumer_removed",
        "temporary_config_removed",
        "journey_artifacts_removed",
        "durable_exact_candidate_install_preserved",
    ):
        if cleanup.get(key) is not True:
            fail(f"cleanup.{key} is not true")
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("package", "source", "journey", "final"))
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--schema", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--package-receipt", type=Path)
    parser.add_argument("--package-schema", type=Path)
    parser.add_argument("--journey-receipt", type=Path)
    parser.add_argument("--journey-schema", type=Path)
    args = parser.parse_args()

    if args.mode == "package":
        validate_package_receipt(args.receipt, args.schema, args.fixture)
        print(f"validated exact candidate package receipt: {args.receipt}")
        return 0

    if args.mode == "source":
        validate_journey_receipt(args.receipt, args.schema)
        print(f"validated source candidate journey receipt: {args.receipt}")
        return 0

    if args.mode == "final":
        validate_final_receipt(args.receipt, args.schema, args.fixture)
        print(f"validated final exact candidate receipt: {args.receipt}")
        return 0

    if args.package_receipt is None or args.package_schema is None:
        fail("journey mode requires --package-receipt and --package-schema")
    if args.journey_receipt is None or args.journey_schema is None:
        fail("journey mode requires --journey-receipt and --journey-schema")

    package = validate_package_receipt(args.package_receipt, args.package_schema, args.fixture)
    journey = validate_journey_receipt(args.journey_receipt, args.journey_schema)
    final = validate_final_receipt(args.receipt, args.schema, args.fixture)
    package_digest = sha256_file(args.package_receipt)
    journey_digest = sha256_file(args.journey_receipt)
    provenance = final["provenance"]
    if provenance.get("package_set_receipt_sha256") != package_digest:
        fail("final receipt package-set digest does not match input receipt")
    if provenance.get("journey_receipt_sha256") != journey_digest:
        fail("final receipt journey digest does not match input receipt")
    if final["candidate"]["workspace_version"] != package["candidate"]["workspace_version"]:
        fail("final/package workspace versions disagree")
    if journey["candidate"]["workspace_version"] != final["candidate"]["workspace_version"]:
        fail("final/journey workspace versions disagree")
    print(f"validated exact candidate install journey receipt: {args.receipt}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, tomllib.TOMLDecodeError) as error:
        fail(str(error))
