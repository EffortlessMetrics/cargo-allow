#!/usr/bin/env python3
"""Validate the stateful exact installed upgrade/rollback receipt (#2485/#3853).

Uses only the Python standard library so hosted package-smoke lanes do not gain
an external validation dependency. Enforces the receipt JSON Schema plus exact
binary identity, typed per-surface compatibility dispositions, previewed write
and migration invariants, and owned-state rollback controls.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path
from typing import Any


EXPECTED_FROM_VERSION = "cargo-allow 0.1.11"
EXPECTED_CANDIDATE_VERSION = "cargo-allow 0.2.0"

REQUIRED_FIXTURE_IDS = {
    "clean_strict",
    "stateful_brownfield_lifecycle",
}

REQUIRED_SURFACE_DISPOSITIONS = {
    "policy/allow.toml",
    "doctor",
    "audit",
    "check",
    "list",
    "explain",
    "worklist",
    "diff",
    "refresh",
    "prune",
    "migrate",
    "why_add_from_plan_stale_input",
    "shallow_missing_base_diff",
}

VALID_DISPOSITIONS = {
    "ReadUnchanged",
    "ReadWithCompatibleProjection",
    "ExplicitMigrationRequired",
    "TransactionInputRejectedAsStale",
    "UnsupportedGeneration",
    "RemovedWithGuidance",
}

REQUIRED_NEGATIVE_CONTROLS = {
    "old_binary_exact_version": "ExactVersion",
    "candidate_binary_exact_version": "ExactVersion",
    "checkout_binary_not_used": "CheckoutIsolated",
    "read_only_commands_no_mutation": "ReadOnlyPreserved",
    "preview_apply_subject_agreement": "PreviewApplyAgree",
    "stale_plan_rejected_as_write_authority": "TransactionInputRejectedAsStale",
    "missing_base_diff_fails_closed": "MissingBaseFailClosed",
    "unrelated_file_survives_rollback": "UnrelatedStatePreserved",
    "unrelated_output_root_file_survives_rollback": "UnrelatedStatePreserved",
    "owned_state_restore_matches_preimage": "OwnedStateRestored",
}

REQUIRED_FROM_STEPS = {
    "old-0.1.11:clean:doctor",
    "old-0.1.11:clean:check",
    "old-0.1.11:doctor",
    "old-0.1.11:audit",
    "old-0.1.11:check",
    "old-0.1.11:list",
    "old-0.1.11:explain",
    "old-0.1.11:worklist",
    "old-0.1.11:diff",
}

REQUIRED_CANDIDATE_STEPS = {
    "candidate-0.2.0:clean:doctor",
    "candidate-0.2.0:clean:check",
    "candidate-0.2.0:doctor",
    "candidate-0.2.0:audit",
    "candidate-0.2.0:check",
    "candidate-0.2.0:list",
    "candidate-0.2.0:explain",
    "candidate-0.2.0:worklist",
    "candidate-0.2.0:diff",
    "candidate-0.2.0:diff-missing-base",
    "candidate-0.2.0:refresh-preview",
    "candidate-0.2.0:refresh-write",
    "candidate-0.2.0:prune-preview",
    "candidate-0.2.0:prune-write",
    "candidate-0.2.0:migrate",
    "candidate-0.2.0:why-plan-stale-reject",
    "candidate-0.2.0:post-mutation-check",
}

REQUIRED_ROLLBACK_STEPS = {
    "rollback-0.1.11:clean:doctor",
    "rollback-0.1.11:clean:check",
    "rollback-0.1.11:doctor",
    "rollback-0.1.11:audit",
    "rollback-0.1.11:check",
    "rollback-0.1.11:list",
    "rollback-0.1.11:explain",
    "rollback-0.1.11:worklist",
    "rollback-0.1.11:diff",
}


def fail(message: str) -> None:
    raise SystemExit(f"exact-upgrade-rollback: {message}")


def load(path: Path) -> dict[str, Any]:
    def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                fail(f"duplicate JSON property {key!r} in {path}")
            result[key] = value
        return result

    try:
        value = json.loads(
            Path(path).read_text(encoding="utf-8"),
            object_pairs_hook=unique_object,
            parse_constant=lambda token: fail(f"invalid JSON constant {token} in {path}"),
        )
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {path}: {error}")
    if not isinstance(value, dict):
        fail("receipt must be an object")
    return value


def digest(path: Path) -> str:
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


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
        fail(f"{path}: value {value!r} is not in enum {schema['enum']!r}")

    expected_type = schema.get("type")
    if expected_type is not None:
        types = expected_type if isinstance(expected_type, list) else [expected_type]
        if not any(schema_type(value, item) for item in types):
            fail(f"{path}: expected type {expected_type!r}")

    if isinstance(value, dict):
        for key in schema.get("required", []):
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
            fail(f"{path}: string {value!r} does not match {pattern!r}")

    if isinstance(value, (int, float)) and not isinstance(value, bool):
        if "minimum" in schema and value < schema["minimum"]:
            fail(f"{path}: number is below minimum")


def validate_receipt(receipt: dict[str, Any], schema: dict[str, Any], fixture_path: Path) -> None:
    validate_schema(receipt, schema)

    if receipt["result"] != "Passed":
        fail(f"receipt result is {receipt['result']!r}, expected 'Passed'")
    if receipt["upgrade_verdict"] not in {"SupportedUpgrade", "MigrationRequired"}:
        fail(f"Passed receipt cannot carry upgrade_verdict={receipt['upgrade_verdict']!r}")
    if receipt["rollback_verdict"] not in {"RollbackSupported", "RollbackRequiresRestore"}:
        fail(f"Passed receipt cannot carry rollback_verdict={receipt['rollback_verdict']!r}")

    # Exact binary identity law: reject wildcard tails or non-final suffixes.
    if receipt["from"]["version"] != EXPECTED_FROM_VERSION:
        fail(f"from leg must be exact {EXPECTED_FROM_VERSION!r}, got {receipt['from']['version']!r}")
    if receipt["candidate"]["version"] != EXPECTED_CANDIDATE_VERSION:
        fail(
            f"candidate leg must be exact {EXPECTED_CANDIDATE_VERSION!r}, "
            f"got {receipt['candidate']['version']!r}"
        )
    if receipt["rollback"]["version"] != EXPECTED_FROM_VERSION:
        fail(
            f"rollback leg must be exact {EXPECTED_FROM_VERSION!r}, "
            f"got {receipt['rollback']['version']!r}"
        )
    if receipt["rollback"]["binary_sha256"] != receipt["from"]["binary_sha256"]:
        fail("rollback binary identity changed")
    if receipt["from"]["binary_sha256"] == receipt["candidate"]["binary_sha256"]:
        fail("from and candidate binary digests must be distinct")

    # Repository and fixture binding.
    repo = receipt["repository"]
    if repo["fixture_sha256"] != digest(fixture_path):
        fail("fixture digest mismatch")
    if repo["unrelated_file_preserved"] is not True:
        fail("unrelated-file control missing")
    if repo["unrelated_output_root_file_preserved"] is not True:
        fail("unrelated output-root file control missing")
    if repo["source_checkout_not_used_as_binary"] is not True:
        fail("source checkout binary isolation not confirmed")
    if repo["optional_profiles_absent"] is not True:
        fail("optional profiles absent posture not confirmed")

    # Steps completeness and uniqueness.
    seen_steps: set[tuple[str, str]] = set()
    step_ids_by_leg: dict[str, set[str]] = {"from": set(), "candidate": set(), "rollback": set()}
    all_step_ids: set[str] = set()
    for step in receipt["steps"]:
        key = (step["leg"], step["id"])
        if key in seen_steps:
            fail(f"duplicate step entry {key!r}")
        seen_steps.add(key)
        step_ids_by_leg[step["leg"]].add(step["id"])
        all_step_ids.add(step["id"])
        # Expected-failure negative probe steps may record non-zero exit codes;
        # all positive leg steps must exit 0.
        if step["id"] not in {
            "candidate-0.2.0:diff-missing-base",
            "candidate-0.2.0:why-plan-stale-reject",
        } and step["exit_code"] != 0:
            fail(f"step {step['id']!r} exited non-zero ({step['exit_code']})")
        if step["id"] in {
            "candidate-0.2.0:diff-missing-base",
            "candidate-0.2.0:why-plan-stale-reject",
        } and step["exit_code"] == 0:
            fail(f"negative probe step {step['id']!r} unexpectedly exited 0")

    for leg_name, required_steps in (
        ("from", REQUIRED_FROM_STEPS),
        ("candidate", REQUIRED_CANDIDATE_STEPS),
        ("rollback", REQUIRED_ROLLBACK_STEPS),
    ):
        missing = sorted(required_steps - step_ids_by_leg[leg_name])
        if missing:
            fail(f"{leg_name} leg missing required steps: {missing}")

    # Fixtures and unrelated-control preservation invariants.
    fixtures_by_id: dict[str, dict[str, Any]] = {}
    for row in receipt["fixtures"]:
        fid = row["id"]
        if fid in fixtures_by_id:
            fail(f"duplicate fixture id {fid!r}")
        fixtures_by_id[fid] = row
        if row["optional_profiles_absent"] is not True:
            fail(f"fixture {fid!r} did not confirm optional_profiles_absent")
        if row["pre_upgrade_allow_ids"] != row["post_rollback_allow_ids"]:
            fail(f"fixture {fid!r} allow IDs drifted after rollback")
        for control in row["unrelated_controls"]:
            if control["preserved"] is not True:
                fail(f"unrelated control {control['path']!r} was not preserved")
            if not (
                control["pre_sha256"]
                == control["post_candidate_sha256"]
                == control["post_rollback_sha256"]
            ):
                fail(f"unrelated control {control['path']!r} digest changed across journey")

    missing_fixtures = sorted(REQUIRED_FIXTURE_IDS - set(fixtures_by_id))
    if missing_fixtures:
        fail(f"missing required fixtures: {missing_fixtures}")

    stateful = fixtures_by_id["stateful_brownfield_lifecycle"]
    if stateful["policy_sha256"] != repo["fixture_sha256"]:
        fail("stateful fixture policy_sha256 does not match committed fixture digest")
    if not stateful["baseline_debt_ids"]:
        fail("stateful fixture must retain at least one baseline_debt entry")
    if not stateful["location_drift_ids"]:
        fail("stateful fixture must exercise at least one location_drift entry")
    if not stateful["stale_ids"]:
        fail("stateful fixture must exercise at least one stale entry")
    control_paths = {c["path"] for c in stateful["unrelated_controls"]}
    if "unrelated.txt" not in control_paths:
        fail("stateful fixture missing repo-root unrelated.txt control")
    if not any(path.startswith("target/cargo-allow/") for path in control_paths):
        fail("stateful fixture missing unrelated control inside target/cargo-allow/")

    # Per-surface compatibility dispositions.
    seen_surfaces: dict[str, dict[str, Any]] = {}
    for row in receipt["surface_dispositions"]:
        surface = row["surface"]
        if surface in seen_surfaces:
            fail(f"duplicate surface disposition {surface!r}")
        if row["disposition"] not in VALID_DISPOSITIONS:
            fail(f"invalid disposition {row['disposition']!r} for {surface!r}")
        for step_id in row["evidence_step_ids"]:
            if step_id not in all_step_ids:
                fail(f"surface {surface!r} cites unknown evidence step {step_id!r}")
        seen_surfaces[surface] = row

    missing_surfaces = sorted(REQUIRED_SURFACE_DISPOSITIONS - set(seen_surfaces))
    if missing_surfaces:
        fail(f"missing required surface dispositions: {missing_surfaces}")
    if (
        seen_surfaces["why_add_from_plan_stale_input"]["disposition"]
        != "TransactionInputRejectedAsStale"
    ):
        fail("why_add_from_plan_stale_input must carry disposition TransactionInputRejectedAsStale")

    # Previewed mutation and migration invariants.
    mutation = receipt["mutation_journey"]
    if mutation["pre_policy_sha256"] != repo["fixture_sha256"]:
        fail("mutation_journey.pre_policy_sha256 does not match committed fixture")
    if mutation["post_readonly_policy_sha256"] != mutation["pre_policy_sha256"]:
        fail("read-only candidate commands mutated policy/allow.toml")
    if mutation["post_mutation_policy_sha256"] == mutation["pre_policy_sha256"]:
        fail("previewed write journey did not mutate policy/allow.toml")

    refresh = mutation["refresh"]
    if refresh["preview_unchanged_policy"] is not True or refresh["preview_apply_agree"] is not True:
        fail("refresh preview/apply invariant failed")
    if refresh["target_allow_id"] not in stateful["location_drift_ids"]:
        fail("refresh target_allow_id is not a declared location_drift fixture entry")

    prune = mutation["prune"]
    if prune["preview_unchanged_policy"] is not True or prune["preview_apply_agree"] is not True:
        fail("prune preview/apply invariant failed")
    if not set(prune["removed_allow_ids"]) <= set(stateful["stale_ids"]):
        fail("prune removed_allow_ids must match declared stale fixture entries")

    migrate = mutation["migrate"]
    if migrate["validated_with_check"] is not True:
        fail("migrated policy output was not validated with check")
    if not set(stateful["baseline_debt_ids"]) <= set(migrate["preserved_allow_ids"]):
        fail("migrate dropped baseline_debt entry")

    stale_plan = mutation["stale_plan_rejection"]
    if stale_plan["rejected_nonzero"] is not True or stale_plan["policy_unchanged"] is not True:
        fail("stale plan rejection control failed")

    # Owned-state rollback and unrelated preservation invariants.
    rollback = receipt["rollback"]
    write_set = mutation["owned_write_set"]
    if rollback["restored_preimage"] is not True or rollback["owned_restore_only"] is not True:
        fail("rollback must use owned-state preimage restoration only")
    if rollback["semantic_parity_restored"] is not True:
        fail("rollback did not confirm post-rollback 0.1.11 semantic parity")
    if rollback["preimage_policy_sha256"] != mutation["pre_policy_sha256"]:
        fail("rollback preimage_policy_sha256 drifted from pre-upgrade fixture digest")
    if rollback["restored_policy_sha256"] != rollback["preimage_policy_sha256"]:
        fail("rollback restored_policy_sha256 does not match preimage")
    if set(rollback["restored_modified_files"]) != set(write_set["modified_files"]):
        fail("rollback restored_modified_files must exactly match owned_write_set.modified_files")
    if set(rollback["removed_owned_files"]) != set(write_set["created_owned_files"]):
        fail("rollback removed_owned_files must exactly match owned_write_set.created_owned_files")
    if set(rollback["removed_owned_files"]) & set(rollback["preserved_unrelated_files"]):
        fail("rollback removed_owned_files overlaps preserved_unrelated_files")
    if not control_paths <= set(rollback["preserved_unrelated_files"]):
        fail("rollback preserved_unrelated_files missing fixture unrelated controls")

    # Negative controls: unique IDs, all required controls passed with exact class.
    seen_controls: dict[str, dict[str, Any]] = {}
    for row in receipt["negative_controls"]:
        cid = row["id"]
        if cid in seen_controls:
            fail(f"duplicate negative control id {cid!r}")
        seen_controls[cid] = row

    for control_id, expected_class in REQUIRED_NEGATIVE_CONTROLS.items():
        row = seen_controls.get(control_id)
        if row is None or row["passed"] is not True:
            fail(f"required negative control missing or failed: {control_id}")
        if row["result_class"] != expected_class:
            fail(
                f"negative control {control_id} has result_class {row['result_class']!r}, "
                f"expected {expected_class!r}"
            )

    # Honesty boundary: stateful slice must not still claim read-only disclaimer.
    if "migration_write_surfaces_are_not_claimed_by_this_read_only_slice" in receipt["limitations"]:
        fail("stale read-only limitation disclaimer retained in stateful receipt")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--schema", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    args = parser.parse_args()
    receipt = load(args.receipt)
    schema = load(args.schema)
    validate_receipt(receipt, schema, args.fixture)
    print(f"validated exact upgrade/rollback receipt: {args.receipt}")


if __name__ == "__main__":
    main()
