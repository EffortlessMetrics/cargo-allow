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
import os
import re
import sys
import tempfile
import tomllib
from pathlib import Path
from typing import Any


EXPECTED_NEGATIVES = {
    "decisive_install_source_checkout_denied": "CheckoutIsolated",
    "omit_candidate_from_local_registry": "PackageMissing",
    "older_internal_package_version": "InternalVersionConflict",
}
EXPECTED_JOURNEY_STEPS = {
    "version": None,
    "doctor_no_policy": "cargo-allow.doctor.v1",
    "audit_with_finding": "cargo-allow.report.v1",
    "bootstrap_propose_write": None,
    "check_no_new_pass": "cargo-allow.report.v1",
    "list_explain_worklist": "cargo-allow.list.v1",
    "refresh_location_drift_preview_write": "cargo-allow.refresh.v1",
    "diff_against_exact_base": "cargo-allow.report.v1",
    "prune_stale_preview_write": "cargo-allow.prune.v1",
    "final_check_no_new": "cargo-allow.report.v1",
    "policy_rollback_after_prune": "cargo-allow.list.v1",
    "second_finding_why_plan_add": "cargo-allow.add.v1",
    "targeted_recheck_after_add": "cargo-allow.report.v1",
    "list_wide_projection": "cargo-allow.list.v1",
    "clean_repo_init_audit_check": "cargo-allow.report.v1",
}
EXPECTED_SOURCE_NEGATIVES = {
    "omitted_journey_step_cannot_claim_passed": "OmittedStep",
    "prune_preview_apply_subject_agree": "PreviewApplyDisagree",
    "refresh_preview_apply_subject_agree": "PreviewApplyDisagree",
    "malformed_smoke_receipt_cannot_claim_passed": "MalformedArtifact",
    "post_install_source_hidden_ordinary_scan": "CheckoutIsolated",
    "missing_asset_not_satisfied_by_source_checkout": "MissingAsset",
    "wrong_installed_binary_version": "StaleCandidate",
    "ordinary_scan_does_not_require_network": "NetworkIsolated",
    "unexpected_network_requirement_during_ordinary_scan": "NetworkRequired",
    "failed_policy_rollback_after_prune": "RecoveryFailed",
    "optional_profile_without_packaged_assets": "NotProven",
}
EXPECTED_ARTIFACT_SCHEMAS = {
    schema_id for schema_id in EXPECTED_JOURNEY_STEPS.values() if schema_id is not None
}


def fail(message: str) -> None:
    raise SystemExit(f"exact-candidate-receipt: {message}")


def load_json(path: Path) -> dict[str, Any]:
    def unique_object(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                fail(f"duplicate JSON property {key!r}: {path}")
            result[key] = value
        return result

    try:
        value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object,
                           parse_constant=lambda value: fail(f"invalid JSON constant {value}: {path}"))
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
    if "const" in schema and (
        value != schema["const"]
        or isinstance(value, bool) != isinstance(schema["const"], bool)
    ):
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
    if not crates or len(set(crates)) != len(crates):
        fail(f"candidate fixture must contain at least one crate: {fixture_path}")
    return crates


def expected_candidates(fixture_path: Path, topology_path: Path) -> list[tuple[str, str]]:
    names = expected_crates(fixture_path)
    topology = tomllib.loads(topology_path.read_text(encoding="utf-8"))
    versions = {}
    for row in topology.get("package", []):
        name = row.get("cargo_package_name")
        if name in names:
            if name in versions or not isinstance(row.get("package_version"), str):
                fail(f"ambiguous or missing topology version for {name}")
            versions[name] = row["package_version"]
    if set(versions) != set(names):
        fail("topology does not identify every canonical candidate")
    return [(name, versions[name]) for name in names]


def negative_map(receipt: dict[str, Any], path: str) -> dict[str, dict[str, Any]]:
    values = receipt.get("negative_controls")
    if not isinstance(values, list):
        fail(f"{path}.negative_controls must be an array")
    result: dict[str, dict[str, Any]] = {}
    for item in values:
        if not isinstance(item, dict) or not isinstance(item.get("id"), str):
            fail(f"{path}.negative_controls has an invalid row")
        if item["id"] in result:
            fail(f"{path}.negative_controls has duplicate id {item['id']}")
        result[item["id"]] = item
    return result


def validate_steps(journey: dict[str, Any]) -> None:
    expected = journey.get("steps_expected", [])
    canonical = list(EXPECTED_JOURNEY_STEPS)
    if expected != canonical:
        fail("journey expected steps are missing, extra, or out of order")
    executed = {}
    for step in journey.get("steps_executed", []):
        identifier = step.get("id")
        if identifier in executed or type(step.get("exit_code")) is not int or step["exit_code"] != 0:
            fail("journey executed steps are duplicate or failed")
        executed[identifier] = step
    if [identifier for identifier in executed if identifier in EXPECTED_JOURNEY_STEPS] != canonical:
        fail("journey required executed steps are missing or out of order")
    for identifier, schema_id in EXPECTED_JOURNEY_STEPS.items():
        if executed[identifier].get("artifact_schema_id") != schema_id:
            fail(f"journey step {identifier} has the wrong artifact schema")


def validate_head(receipt: dict[str, Any]) -> None:
    if not re.fullmatch(r"[0-9a-f]{40}", str(receipt.get("candidate", {}).get("git_head", ""))):
        fail("candidate git head must be an exact commit")


def validate_package_receipt(
    receipt_path: Path, schema_path: Path, fixture_path: Path, topology_path: Path
) -> dict[str, Any]:
    receipt = load_json(receipt_path)
    schema = load_json(schema_path)
    validate_schema(receipt, schema, "package_receipt")
    validate_head(receipt)
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
    identities = expected_candidates(fixture_path, topology_path)
    if [(row["name"], row["version"]) for row in rows] != identities:
        fail("package receipt identities do not match the selected topology")
    for row in rows:
        if row["crate_file"] != f"{row['name']}-{row['version']}.crate" or row["no_path_deps"] is not True:
            fail("package row filename or path-dependency posture disagrees with identity")
    if receipt["install"]["exit_code"] != 0:
        fail("package install did not succeed")
    if receipt["install"]["version_output"] != f"cargo-allow {dict(identities)['cargo-allow']}":
        fail("package install version does not match the selected topology")
    for key, expected in {"fresh_cargo_home": True, "install_from_extracted_package": True,
                          "internal_deps_patched": False, "workspace_paths_absent": True}.items():
        if receipt["isolation"].get(key) is not expected:
            fail(f"package isolation.{key} disagrees with exact install")
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
    validate_head(receipt)
    validate_steps(receipt["journey"])
    negatives = negative_map(receipt, "source_receipt")
    for identifier, classification in EXPECTED_SOURCE_NEGATIVES.items():
        row = negatives.get(identifier, {})
        if row.get("passed") is not True or row.get("result_class") != classification:
            fail(f"source receipt missing successful negative {identifier}: {classification}")
    return receipt


def validate_final_receipt(
    receipt_path: Path, schema_path: Path, fixture_path: Path
) -> dict[str, Any]:
    receipt = load_json(receipt_path)
    validate_schema(receipt, load_json(schema_path), "final_receipt")
    if receipt.get("result") != "Passed":
        fail("final receipt is not Passed")
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
        "installed_binary_sha256",
        "candidate_topology_sha256",
    ):
        if not re.fullmatch(r"[0-9a-f]{64}", str(provenance.get(key, ""))):
            fail(f"final receipt has invalid digest at provenance.{key}")
    if provenance.get("candidate_fixture_sha256") != sha256_file(fixture_path):
        fail("final receipt fixture digest does not match the fixture on disk")
    if receipt.get("install", {}).get("source_checkout_denied") is not True:
        fail("final receipt does not prove source-checkout denial")
    if receipt.get("install", {}).get("observed_source_fallback_paths_hidden") is not True:
        fail("final receipt does not prove observed fallback paths were hidden")
    journey = receipt.get("journey", {})
    validate_steps(journey)
    artifact_ids = set(journey.get("artifact_schema_ids", []))
    if not EXPECTED_ARTIFACT_SCHEMAS.issubset(artifact_ids):
        fail("final receipt omitted a validated journey artifact schema")
    negatives = negative_map(receipt, "final_receipt")
    required_final_negatives = {
        "source_checkout_denied_during_exact_install": "CheckoutIsolated",
        "observed_source_fallback_paths_hidden": "CheckoutIsolated",
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


def validate_binding(package: dict[str, Any], source: dict[str, Any], final: dict[str, Any],
                     args: argparse.Namespace) -> None:
    inputs = {
        "package_set_receipt_sha256": args.package_receipt,
        "journey_receipt_sha256": args.journey_receipt,
        "candidate_fixture_sha256": args.fixture,
        "candidate_topology_sha256": args.topology,
        "installed_binary_sha256": args.binary,
    }
    digests = {key: sha256_file(path) for key, path in inputs.items()}
    # Publication must validate the current files and topology, rather than
    # attach their latest digests to objects validated before the journey ran.
    current_package = validate_package_receipt(
        args.package_receipt, args.package_schema, args.fixture, args.topology)
    current_source = validate_journey_receipt(args.journey_receipt, args.journey_schema)
    for current, previous in ((current_package, package), (current_source, source)):
        # Python object equality equates JSON false with 0 (and true with 1).
        if json.dumps(current, sort_keys=True) != json.dumps(previous, sort_keys=True):
            fail("input receipt changed after validation")
    for label, receipt in (("package", package), ("source", source), ("final", final)):
        if receipt["candidate"]["git_head"] != args.current_head:
            fail(f"{label} candidate head does not match the requested head")
        if receipt["candidate"]["workspace_version"] != package["candidate"]["workspace_version"]:
            fail(f"{label} candidate workspace version disagrees with package")
    if source["candidate"]["install_method"] != "prebuilt_override":
        fail("source journey did not use the supplied installed binary")
    digest = digests["installed_binary_sha256"]
    if digest != package["install"]["binary_sha256"] or digest != source["installed_binary"]["binary_sha256"]:
        fail("actual installed binary bytes disagree with package/source receipts")
    if source["installed_binary"]["version_output"] != package["install"]["version_output"]:
        fail("package/source installed versions disagree")
    if final["install"]["version_output"] != source["installed_binary"]["version_output"]:
        fail("final/source installed versions disagree")
    for key, expected in digests.items():
        if final["provenance"][key] != expected:
            fail(f"final receipt {key} does not match the actual input")
    for key in ("steps_expected", "steps_executed"):
        if final["journey"][key] != source["journey"][key]:
            fail(f"final/source journey {key} disagree")
    schemas = sorted({row["artifact_schema_id"] for row in source["journey"]["steps_executed"]
                      if row.get("artifact_schema_id")})
    if final["journey"]["artifact_schema_ids"] != schemas:
        fail("final/source journey artifact schemas disagree")
    if any(sha256_file(inputs[key]) != digest for key, digest in digests.items()):
        fail("input changed during binding validation")


def assemble_receipt(package: dict[str, Any], source: dict[str, Any],
                     args: argparse.Namespace) -> dict[str, Any]:
    """Build a candidate in memory; no Passed output is visible before validation."""
    return {
        "schema_version": 1,
        "schema_id": "cargo-allow.exact-candidate-install-journey.v1",
        "tool": "cargo-allow",
        "result": "Passed",
        "claim_boundary": [
            "canonical_thirteen_crate_candidate", "isolated_exact_candidate_install",
            "source_checkout_denied_during_install",
            "observed_source_fallback_paths_hidden_for_ordinary_scan",
            "digest_bound_package_source_and_binary_evidence",
            "first_hour_finding_repair_and_policy_rollback",
            "temporary_consumer_and_journey_state_cleanup", "not_published_or_registry_release",
        ],
        "candidate": {**package["candidate"], "crate_count": len(package["package_set"]["order"])},
        "provenance": {
            "package_set_schema_id": package["schema_id"],
            "journey_schema_id": source["schema_id"],
            "package_set_receipt_sha256": sha256_file(args.package_receipt),
            "journey_receipt_sha256": sha256_file(args.journey_receipt),
            "candidate_fixture_sha256": sha256_file(args.fixture),
            "candidate_topology_sha256": sha256_file(args.topology),
            "installed_binary_sha256": sha256_file(args.binary),
            "crate_order": package["package_set"]["order"],
        },
        "install": {
            "method": package["install"]["method"],
            "version_output": source["installed_binary"]["version_output"],
            "source_checkout_denied": True,
            "observed_source_fallback_paths_hidden": True,
            "path_redacted": True,
        },
        "journey": {
            "finding_step": "audit_with_finding",
            "repair_or_rollback_step": "policy_rollback_after_prune",
            "artifact_schema_ids": sorted({row["artifact_schema_id"] for row in source["journey"]["steps_executed"]
                                           if row.get("artifact_schema_id")}),
            "steps_expected": source["journey"]["steps_expected"],
            "steps_executed": source["journey"]["steps_executed"],
        },
        "negative_controls": [
            {"id": identifier, "result_class": classification, "passed": True}
            for identifier, classification in (
                ("source_checkout_denied_during_exact_install", "CheckoutIsolated"),
                ("observed_source_fallback_paths_hidden", "CheckoutIsolated"),
                ("missing_candidate_sibling_rejected", "PackageMissing"),
                ("wrong_candidate_sibling_version_rejected", "InternalVersionConflict"),
            )
        ],
        "cleanup": {
            "temporary_consumer_removed": True, "temporary_config_removed": True,
            "journey_artifacts_removed": True, "durable_exact_candidate_install_preserved": True,
        },
        "limitations": [
            "observed_source_fallback_paths_only_not_universal_read_tracing",
            "package_fetch_warm_may_use_crates_io_before_offline_install",
            "hosted_package_smoke_is_the_primary_linux_evidence", "no_publication_tagging_or_registry_release",
        ],
    }


def publish_receipt(package: dict[str, Any], source: dict[str, Any], args: argparse.Namespace) -> None:
    if args.receipt.exists():
        fail("refusing to overwrite an existing final receipt")
    final = assemble_receipt(package, source, args)
    validate_binding(package, source, final, args)
    pending = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=args.receipt.parent,
                                         prefix=".pending-receipt-", delete=False) as handle:
            pending = Path(handle.name)
            json.dump(final, handle, indent=2)
            handle.write("\n")
        validate_final_receipt(pending, args.schema, args.fixture)
        os.replace(pending, args.receipt)
    finally:
        if pending is not None:
            pending.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("package", "source", "journey", "final"))
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--schema", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--topology", type=Path, default=Path(__file__).resolve().parent.parent / "policy/product-package-topology-v2.toml")
    parser.add_argument("--package-receipt", type=Path)
    parser.add_argument("--package-schema", type=Path)
    parser.add_argument("--journey-receipt", type=Path)
    parser.add_argument("--journey-schema", type=Path)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--current-head")
    args = parser.parse_args()

    if args.mode == "package":
        validate_package_receipt(args.receipt, args.schema, args.fixture, args.topology)
        print(f"validated exact candidate package receipt: {args.receipt}")
        return 0

    if args.mode == "source":
        validate_journey_receipt(args.receipt, args.schema)
        print(f"validated source candidate journey receipt: {args.receipt}")
        return 0

    if args.mode == "final":
        validate_final_receipt(args.receipt, args.schema, args.fixture)
        print(f"validated final receipt structure only (no input binding): {args.receipt}")
        return 0

    if args.package_receipt is None or args.package_schema is None:
        fail("journey mode requires --package-receipt and --package-schema")
    if args.journey_receipt is None or args.journey_schema is None:
        fail("journey mode requires --journey-receipt and --journey-schema")

    if args.binary is None or args.current_head is None:
        fail("cross-receipt validation requires --binary and --current-head")
    package = validate_package_receipt(args.package_receipt, args.package_schema, args.fixture, args.topology)
    journey = validate_journey_receipt(args.journey_receipt, args.journey_schema)
    final = validate_final_receipt(args.receipt, args.schema, args.fixture)
    validate_binding(package, journey, final, args)
    print(f"validated exact candidate install journey receipt: {args.receipt}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, tomllib.TOMLDecodeError) as error:
        fail(str(error))
