#!/usr/bin/env bash
# Offline positive and negative (falsifying) unit tests for the stateful
# exact upgrade/rollback journey receipt, schema, and validator (#2485/#3853).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 - "${ROOT}" <<'PY'
import copy
import hashlib
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(sys.argv[1]).resolve()
VALIDATOR_PATH = ROOT / "scripts/validate-upgrade-rollback-journey.py"
SCHEMA_PATH = ROOT / "docs/dogfood/fixtures/release/exact-upgrade-rollback-journey.v1.schema.json"
FIXTURE_PATH = ROOT / "docs/dogfood/fixtures/release/upgrade-rollback-repository.toml"

SPEC = importlib.util.spec_from_file_location("validate_upgrade_rollback", VALIDATOR_PATH)
VALIDATOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VALIDATOR)


def make_valid_receipt(fixture_path: Path) -> dict:
    fixture_digest = hashlib.sha256(fixture_path.read_bytes()).hexdigest()
    from_steps = [
        {"leg": "from", "id": step_id, "exit_code": 0}
        for step_id in sorted(VALIDATOR.REQUIRED_FROM_STEPS)
    ]
    candidate_steps = [
        {
            "leg": "candidate",
            "id": step_id,
            "exit_code": 1
            if step_id
            in {
                "candidate-0.2.0:diff-missing-base",
                "candidate-0.2.0:why-plan-stale-reject",
            }
            else 0,
        }
        for step_id in sorted(VALIDATOR.REQUIRED_CANDIDATE_STEPS)
    ]
    rollback_steps = [
        {"leg": "rollback", "id": step_id, "exit_code": 0}
        for step_id in sorted(VALIDATOR.REQUIRED_ROLLBACK_STEPS)
    ]
    surface_rows = [
        ("policy/allow.toml", "policy", "ReadWithCompatibleProjection", ["old-0.1.11:check", "candidate-0.2.0:check", "rollback-0.1.11:check"]),
        ("doctor", "command", "ReadWithCompatibleProjection", ["old-0.1.11:doctor", "candidate-0.2.0:doctor", "rollback-0.1.11:doctor"]),
        ("audit", "command", "ReadUnchanged", ["old-0.1.11:audit", "candidate-0.2.0:audit", "rollback-0.1.11:audit"]),
        ("check", "command", "ReadWithCompatibleProjection", ["old-0.1.11:check", "candidate-0.2.0:check", "candidate-0.2.0:post-mutation-check", "rollback-0.1.11:check"]),
        ("list", "command", "ReadWithCompatibleProjection", ["old-0.1.11:list", "candidate-0.2.0:list", "rollback-0.1.11:list"]),
        ("explain", "command", "ReadWithCompatibleProjection", ["old-0.1.11:explain", "candidate-0.2.0:explain", "rollback-0.1.11:explain"]),
        ("worklist", "command", "ReadWithCompatibleProjection", ["old-0.1.11:worklist", "candidate-0.2.0:worklist", "rollback-0.1.11:worklist"]),
        ("diff", "diff", "ReadUnchanged", ["old-0.1.11:diff", "candidate-0.2.0:diff", "rollback-0.1.11:diff"]),
        ("shallow_missing_base_diff", "diff", "ReadUnchanged", ["candidate-0.2.0:diff-missing-base"]),
        ("refresh", "mutation", "ReadWithCompatibleProjection", ["candidate-0.2.0:refresh-preview", "candidate-0.2.0:refresh-write"]),
        ("prune", "mutation", "ReadWithCompatibleProjection", ["candidate-0.2.0:prune-preview", "candidate-0.2.0:prune-write"]),
        ("migrate", "mutation", "ReadWithCompatibleProjection", ["candidate-0.2.0:migrate"]),
        ("why_add_from_plan_stale_input", "artifact", "TransactionInputRejectedAsStale", ["candidate-0.2.0:why-plan-stale-reject"]),
    ]
    created_owned = [
        "target/cargo-allow/refresh-preview.json",
        "target/cargo-allow/refresh-write.json",
        "target/cargo-allow/prune-preview.json",
        "target/cargo-allow/prune-write.json",
        "target/cargo-allow/migrated-allow.toml",
        "target/cargo-allow/migrate-summary.json",
        "target/cargo-allow/stale-probe-plan.json",
    ]
    return {
        "schema_version": 1,
        "schema_id": "cargo-allow.exact-upgrade-rollback-journey.v1",
        "tool": "cargo-allow",
        "result": "Passed",
        "upgrade_verdict": "SupportedUpgrade",
        "rollback_verdict": "RollbackRequiresRestore",
        "claim_boundary": [
            "exact_public_0.1.11",
            "exact_candidate_0.2.0",
            "stateful_previewed_mutation_and_migrate",
            "owned_state_preimage_restore_preserving_unrelated_files",
        ],
        "from": {
            "version": "cargo-allow 0.1.11",
            "binary_sha256": "a" * 64,
            "install_root_id": "old-install",
        },
        "candidate": {
            "version": "cargo-allow 0.2.0",
            "binary_sha256": "b" * 64,
            "package_set_receipt_sha256": "1" * 64,
            "git_head": "2" * 40,
        },
        "rollback": {
            "version": "cargo-allow 0.1.11",
            "binary_sha256": "a" * 64,
            "install_root_id": "old-install",
            "restored_preimage": True,
            "owned_restore_only": True,
            "preimage_policy_sha256": fixture_digest,
            "restored_policy_sha256": fixture_digest,
            "restored_modified_files": ["policy/allow.toml"],
            "removed_owned_files": list(created_owned),
            "preserved_unrelated_files": [
                "unrelated.txt",
                "target/cargo-allow/unrelated-user.txt",
            ],
            "semantic_parity_restored": True,
        },
        "repository": {
            "fixture_sha256": fixture_digest,
            "unrelated_file_preserved": True,
            "unrelated_output_root_file_preserved": True,
            "source_checkout_not_used_as_binary": True,
            "optional_profiles_absent": True,
        },
        "fixtures": [
            {
                "id": "clean_strict",
                "policy_sha256": "3" * 64,
                "pre_upgrade_allow_ids": [],
                "post_rollback_allow_ids": [],
                "baseline_debt_ids": [],
                "location_drift_ids": [],
                "stale_ids": [],
                "optional_profiles_absent": True,
                "unrelated_controls": [
                    {
                        "path": "unrelated.txt",
                        "pre_sha256": "4" * 64,
                        "post_candidate_sha256": "4" * 64,
                        "post_rollback_sha256": "4" * 64,
                        "preserved": True,
                    }
                ],
            },
            {
                "id": "stateful_brownfield_lifecycle",
                "policy_sha256": fixture_digest,
                "pre_upgrade_allow_ids": ["allow-0001", "allow-0002", "allow-0003", "allow-0004"],
                "post_rollback_allow_ids": ["allow-0001", "allow-0002", "allow-0003", "allow-0004"],
                "baseline_debt_ids": ["allow-0002"],
                "location_drift_ids": ["allow-0003"],
                "stale_ids": ["allow-0004"],
                "optional_profiles_absent": True,
                "unrelated_controls": [
                    {
                        "path": "unrelated.txt",
                        "pre_sha256": "5" * 64,
                        "post_candidate_sha256": "5" * 64,
                        "post_rollback_sha256": "5" * 64,
                        "preserved": True,
                    },
                    {
                        "path": "target/cargo-allow/unrelated-user.txt",
                        "pre_sha256": "6" * 64,
                        "post_candidate_sha256": "6" * 64,
                        "post_rollback_sha256": "6" * 64,
                        "preserved": True,
                    },
                ],
            },
        ],
        "surface_dispositions": [
            {
                "surface": surface,
                "category": category,
                "disposition": disposition,
                "evidence_step_ids": evidence_steps,
                "notes": f"verified {surface}",
            }
            for surface, category, disposition, evidence_steps in surface_rows
        ],
        "mutation_journey": {
            "pre_policy_sha256": fixture_digest,
            "post_readonly_policy_sha256": fixture_digest,
            "post_mutation_policy_sha256": "7" * 64,
            "refresh": {
                "target_allow_id": "allow-0003",
                "preview_unchanged_policy": True,
                "preview_apply_agree": True,
                "mutation_receipt_schema_id": "cargo-allow.refresh.v1",
                "mutation_result": "written",
            },
            "prune": {
                "removed_allow_ids": ["allow-0004"],
                "preview_unchanged_policy": True,
                "preview_apply_agree": True,
                "mutation_receipt_schema_id": "cargo-allow.prune.v1",
                "mutation_result": "written",
            },
            "migrate": {
                "output_path": "target/cargo-allow/migrated-allow.toml",
                "output_sha256": "8" * 64,
                "summary_schema_id": "cargo-allow.migrate.v1",
                "preserved_allow_ids": ["allow-0001", "allow-0002", "allow-0003"],
                "validated_with_check": True,
            },
            "stale_plan_rejection": {
                "plan_path": "target/cargo-allow/stale-probe-plan.json",
                "rejected_nonzero": True,
                "policy_unchanged": True,
                "disposition": "TransactionInputRejectedAsStale",
            },
            "owned_write_set": {
                "modified_files": ["policy/allow.toml"],
                "created_owned_files": list(created_owned),
            },
        },
        "steps": from_steps + candidate_steps + rollback_steps,
        "negative_controls": [
            {"id": cid, "result_class": rclass, "passed": True}
            for cid, rclass in sorted(VALIDATOR.REQUIRED_NEGATIVE_CONTROLS.items())
        ],
        "limitations": [
            "candidate_leg_consumes_the_existing_exact_candidate_package_receipt",
            "post_publication_crates_io_0_2_0_leg_requires_separate_refresh_after_publish",
            "rust_1_85_msrv_guidance_is_governed_by_the_separate_msrv_lane",
        ],
    }


class UpgradeRollbackValidatorTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="upgrade-rollback-unit-")
        self.addCleanup(self.tmp.cleanup)
        self.dir = Path(self.tmp.name)
        self.schema = VALIDATOR.load(SCHEMA_PATH)
        self.base = make_valid_receipt(FIXTURE_PATH)

    def write_receipt(self, data: dict) -> Path:
        path = self.dir / "receipt.json"
        path.write_text(json.dumps(data), encoding="utf-8")
        return path

    def test_valid_stateful_receipt_passes_cli_and_module(self):
        path = self.write_receipt(self.base)
        VALIDATOR.validate_receipt(self.base, self.schema, FIXTURE_PATH)
        proc = subprocess.run(
            [
                sys.executable,
                str(VALIDATOR_PATH),
                "--receipt",
                str(path),
                "--schema",
                str(SCHEMA_PATH),
                "--fixture",
                str(FIXTURE_PATH),
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertIn("validated exact upgrade/rollback receipt", proc.stdout)

    def test_rejects_version_and_binary_identity_forgeries(self):
        cases = {
            "candidate_rc_suffix_rejected": lambda r: r["candidate"].__setitem__("version", "cargo-allow 0.2.0-rc.1"),
            "from_wildcard_suffix_rejected": lambda r: r["from"].__setitem__("version", "cargo-allow 0.1.11-dev"),
            "rollback_latest_version_rejected": lambda r: r["rollback"].__setitem__("version", "cargo-allow 0.2.0"),
            "rollback_binary_digest_changed": lambda r: r["rollback"].__setitem__("binary_sha256", "c" * 64),
            "same_binary_masquerading_both_legs": lambda r: r["candidate"].__setitem__("binary_sha256", r["from"]["binary_sha256"]),
        }
        for label, mutate in cases.items():
            with self.subTest(label=label):
                forged = copy.deepcopy(self.base)
                mutate(forged)
                with self.assertRaises(SystemExit):
                    VALIDATOR.validate_receipt(forged, self.schema, FIXTURE_PATH)

    def test_rejects_verdict_and_readonly_disclaimer_contradictions(self):
        cases = {
            "passed_with_blocked_upgrade_verdict": lambda r: r.__setitem__("upgrade_verdict", "UpgradeBlocked"),
            "passed_with_blocked_rollback_verdict": lambda r: r.__setitem__("rollback_verdict", "RollbackBlocked"),
            "non_passed_result": lambda r: r.__setitem__("result", "UpgradeBlocked"),
            "stale_readonly_limitation_retained": lambda r: r["limitations"].append(
                "migration_write_surfaces_are_not_claimed_by_this_read_only_slice"
            ),
            "unexpected_top_level_property": lambda r: r.__setitem__("extra_field", True),
        }
        for label, mutate in cases.items():
            with self.subTest(label=label):
                forged = copy.deepcopy(self.base)
                mutate(forged)
                with self.assertRaises(SystemExit):
                    VALIDATOR.validate_receipt(forged, self.schema, FIXTURE_PATH)

    def test_rejects_surface_disposition_forgeries(self):
        def set_surface_disposition(r, name, value):
            for row in r["surface_dispositions"]:
                if row["surface"] == name:
                    row["disposition"] = value

        cases = {
            "missing_surface": lambda r: r["surface_dispositions"].pop(),
            "duplicate_surface": lambda r: r["surface_dispositions"].append(copy.deepcopy(r["surface_dispositions"][0])),
            "invalid_disposition_enum": lambda r: set_surface_disposition(r, "check", "CompatibleEnough"),
            "stale_plan_wrong_disposition": lambda r: set_surface_disposition(
                r, "why_add_from_plan_stale_input", "ReadUnchanged"
            ),
            "unsupported_generation_on_passed_receipt": lambda r: set_surface_disposition(
                r, "doctor", "UnsupportedGeneration"
            ),
            "explicit_migration_required_under_supported_upgrade": lambda r: set_surface_disposition(
                r, "policy/allow.toml", "ExplicitMigrationRequired"
            ),
            "unknown_evidence_step_ref": lambda r: r["surface_dispositions"][0]["evidence_step_ids"].append(
                "candidate-0.2.0:invented-step"
            ),
        }
        for label, mutate in cases.items():
            with self.subTest(label=label):
                forged = copy.deepcopy(self.base)
                mutate(forged)
                with self.assertRaises(SystemExit):
                    VALIDATOR.validate_receipt(forged, self.schema, FIXTURE_PATH)

    def test_rejects_mutation_and_owned_rollback_forgeries(self):
        cases = {
            "readonly_mutated_policy": lambda r: r["mutation_journey"].__setitem__("post_readonly_policy_sha256", "9" * 64),
            "no_actual_mutation_occurred": lambda r: r["mutation_journey"].__setitem__(
                "post_mutation_policy_sha256", r["mutation_journey"]["pre_policy_sha256"]
            ),
            "refresh_preview_mutated": lambda r: r["mutation_journey"]["refresh"].__setitem__("preview_unchanged_policy", False),
            "refresh_preview_apply_disagree": lambda r: r["mutation_journey"]["refresh"].__setitem__("preview_apply_agree", False),
            "prune_preview_apply_disagree": lambda r: r["mutation_journey"]["prune"].__setitem__("preview_apply_agree", False),
            "migrate_dropped_baseline_debt": lambda r: r["mutation_journey"]["migrate"].__setitem__(
                "preserved_allow_ids", ["allow-0001", "allow-0003"]
            ),
            "migrate_preserved_overlaps_prune_removed": lambda r: r["mutation_journey"]["migrate"]["preserved_allow_ids"].append(
                "allow-0004"
            ),
            "migrate_wrong_summary_schema_id": lambda r: r["mutation_journey"]["migrate"].__setitem__(
                "summary_schema_id", "cargo-allow.other.v1"
            ),
            "fixture_baseline_debt_not_in_pre_upgrade": lambda r: r["fixtures"][1]["baseline_debt_ids"].append(
                "allow-9999"
            ),
            "fixture_location_drift_not_in_pre_upgrade": lambda r: r["fixtures"][1]["location_drift_ids"].append(
                "allow-9999"
            ),
            "fixture_stale_not_in_pre_upgrade": lambda r: r["fixtures"][1]["stale_ids"].append(
                "allow-9999"
            ),
            "stale_plan_accepted": lambda r: r["mutation_journey"]["stale_plan_rejection"].__setitem__("rejected_nonzero", False),
            "rollback_not_owned_only": lambda r: r["rollback"].__setitem__("owned_restore_only", False),
            "rollback_policy_mismatch": lambda r: r["rollback"].__setitem__("restored_policy_sha256", "f" * 64),
            "rollback_deleted_unrelated_output_file": lambda r: r["rollback"]["removed_owned_files"].append(
                "target/cargo-allow/unrelated-user.txt"
            ),
            "rollback_unmatched_created_owned_set": lambda r: r["rollback"]["removed_owned_files"].pop(),
            "unrelated_control_digest_changed": lambda r: r["fixtures"][1]["unrelated_controls"][1].__setitem__(
                "post_rollback_sha256", "e" * 64
            ),
            "fixture_allow_ids_drifted_after_rollback": lambda r: r["fixtures"][1]["post_rollback_allow_ids"].pop(),
            "missing_output_root_unrelated_control": lambda r: r["fixtures"][1]["unrelated_controls"].pop(),
        }
        for label, mutate in cases.items():
            with self.subTest(label=label):
                forged = copy.deepcopy(self.base)
                mutate(forged)
                with self.assertRaises(SystemExit):
                    VALIDATOR.validate_receipt(forged, self.schema, FIXTURE_PATH)

    def test_rejects_step_and_negative_control_and_json_forgeries(self):
        cases = {
            "missing_step": lambda r: r["steps"].pop(),
            "duplicate_step": lambda r: r["steps"].append(copy.deepcopy(r["steps"][0])),
            "boolean_exit_code": lambda r: r["steps"][0].__setitem__("exit_code", False),
            "negative_probe_exited_zero": lambda r: next(
                s for s in r["steps"] if s["id"] == "candidate-0.2.0:why-plan-stale-reject"
            ).__setitem__("exit_code", 0),
            "missing_negative_control": lambda r: r["negative_controls"].pop(),
            "duplicate_negative_control": lambda r: r["negative_controls"].append(copy.deepcopy(r["negative_controls"][0])),
            "wrong_negative_result_class": lambda r: r["negative_controls"][0].__setitem__("result_class", "Passed"),
            "fixture_digest_mismatch": lambda r: r["repository"].__setitem__("fixture_sha256", "0" * 64),
        }
        for label, mutate in cases.items():
            with self.subTest(label=label):
                forged = copy.deepcopy(self.base)
                mutate(forged)
                with self.assertRaises(SystemExit):
                    VALIDATOR.validate_receipt(forged, self.schema, FIXTURE_PATH)

        # Duplicate JSON key and non-finite constant rejection at loader boundary.
        for bad_json in (
            '{"schema_version": 1, "schema_version": 1}',
            '{"schema_version": NaN}',
        ):
            with self.subTest(bad_json=bad_json):
                bad_path = self.dir / "bad.json"
                bad_path.write_text(bad_json, encoding="utf-8")
                with self.assertRaises(SystemExit):
                    VALIDATOR.load(bad_path)


suite = unittest.defaultTestLoader.loadTestsFromTestCase(UpgradeRollbackValidatorTests)
result = unittest.TextTestRunner(verbosity=2).run(suite)
if not result.wasSuccessful():
    sys.exit(1)
PY

printf 'ok exact upgrade/rollback stateful receipt, schema, and negative characterization\n'
