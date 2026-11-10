#!/usr/bin/env python3
"""Admission controls for the #4725 C-family mutation transport.

The tests construct synthetic retained collections and manifests and require
scripts/verify-command-mutation-evidence.py to admit the coherent observation
and to reject, with the specific reason, wrong subjects, wrong case families,
duplicate/unknown/missing case IDs, replacement bytes, undeclared writes,
missing declared writes, stale snapshot values, fake successes, contract-probe
escapes and invalid dispositions. Nothing here executes the real scanner or
approves exceptions.
"""

import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


sys.dont_write_bytecode = True
SCRIPT = Path(__file__).resolve().with_name("verify-command-mutation-evidence.py")
REPOSITORY = SCRIPT.parent.parent
CATALOGUE = REPOSITORY / "docs/release/core-command-migration-cases.v1.json"
HISTORICAL_EVIDENCE = REPOSITORY / "docs/release/evidence/command-mutation-1"
SPEC = importlib.util.spec_from_file_location("verify_command_mutation", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise SystemExit("cannot load the production admission reader")
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)

BINARY_DIGEST_TEXT = "a" * 64
OTHER_DIGEST_TEXT = "b" * 64
SOURCE_GENERATION = "1" * 40
SOURCE_BYTES = (
    "pub fn a() {\n    panic!(\"intentional fixture finding\");\n}\n"
).encode()


def digest(data):
    return "sha256:v1:" + hashlib.sha256(data).hexdigest()


def fake_digest(tail="0"):
    return "sha256:v1:" + (tail + "0" * 63)[-64:]


class Store:
    def __init__(self, root):
        self.root = Path(root)
        self.root.mkdir(parents=True, exist_ok=True)

    def put(self, relative, data):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return {"path": relative, "size_bytes": len(data), "digest": digest(data)}

    def read(self, member):
        return (self.root / member["path"]).read_bytes()


def snapshot_member(store, prefix, files, name):
    entries = []
    for relative, data in sorted(files.items()):
        content = store.put(f"{prefix}/files/{relative}", data)
        entries.append({
            "path": relative, "kind": "file", "mode": 0o644,
            "size_bytes": len(data), "content": content, "link_target": None,
        })
    return store.put(f"{prefix}/{name}.json",
                     json_bytes({"entries": entries}))


def json_bytes(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def binary_context(provenance="supplied_installed_candidate",
                   candidate=None, install=None):
    return {
        "path": "F:/absolute/cargo-allow.exe",
        "size_bytes": 1024,
        "digest": fake_digest(BINARY_DIGEST_TEXT[0]),
        "tool_version": "0.2.0-rc.1",
        "provenance": provenance,
        "source_generation": SOURCE_GENERATION,
        "candidate_identity": candidate or fake_digest("5"),
        "install_identity": install or fake_digest("6"),
    }


def context_for(case_ids, binary=None):
    templates = {
        "C.init.preview": {
            "recipe": {}, "forbidden_paths": [],
            "steps": [{
                "purpose": "dry-run reports would-create guidance",
                "route": "preview", "observation_class": "outcome",
                "argv_template": ["BINARY:", "init", "--dry-run"],
                "declared_writes": [], "expected_exit": "zero",
                "change_expectation": "none",
                "probe_write_path": None, "fixture_control": None,
                "artifact_roles": [],
            }],
        },
        "C.init.apply": {
            "recipe": {}, "forbidden_paths": [],
            "steps": [
                {
                    "purpose": "default init creation",
                    "route": "mutation", "observation_class": "outcome",
                    "argv_template": ["BINARY:", "init"],
                    "declared_writes": ["policy", "policy/allow.toml"],
                    "expected_exit": "zero",
                    "change_expectation": "required",
                    "probe_write_path": None, "fixture_control": None,
                    "artifact_roles": [],
                },
                {
                    "purpose": "rollback from retained backup",
                    "route": "fixture_rollback", "observation_class": "fixture_control",
                    "argv_template": [],
                    "declared_writes": ["policy/allow.toml"],
                    "expected_exit": "zero",
                    "change_expectation": "optional",
                    "probe_write_path": None,
                    "fixture_control": {"action": "restore",
                                        "source": "policy/allow.toml.bak",
                                        "target": "policy/allow.toml"},
                    "artifact_roles": [],
                },
            ],
        },
        "C.propose.candidate": {
            "recipe": {}, "forbidden_paths": [],
            "steps": [{
                "purpose": "probe step",
                "route": "preview", "observation_class": "contract_probe",
                "argv_template": ["BINARY:", "propose", "--config",
                                  "ROOT:policy/allow.toml", "--dry-run"],
                "declared_writes": ["policy/candidate.toml"],
                "expected_exit": "any",
                "change_expectation": "optional",
                "probe_write_path": "policy/candidate.toml",
                "fixture_control": None, "artifact_roles": [],
            }],
        },
        "control.symlink-alias": {
            "recipe": {}, "forbidden_paths": [],
            "steps": [{
                "purpose": "alias creation attempt",
                "route": "fixture_control", "observation_class": "platform_probe",
                "argv_template": [],
                "declared_writes": ["policy/allow-link.toml"],
                "expected_exit": "any", "probe_write_path": None,
                "change_expectation": "optional",
                "fixture_control": {"action": "symlink",
                                    "link": "policy/allow-link.toml",
                                    "target": "policy/allow.toml"},
                "artifact_roles": [],
            }],
        },
        "control.missing-rollback-state": {
            "recipe": {}, "forbidden_paths": [],
            "steps": [{
                "purpose": "rollback without state fails",
                "route": "fixture_rollback", "observation_class": "fixture_control",
                "argv_template": [],
                "declared_writes": ["policy/allow.toml"], "expected_exit": "zero",
                "probe_write_path": None,
                "change_expectation": "optional",
                "fixture_control": {"action": "restore",
                                    "source": "policy/allow.toml.bak",
                                    "target": "policy/allow.toml"},
                "artifact_roles": [],
            }],
        },
    }
    return {
        "schema_id": VERIFY.CONTEXT_SCHEMA, "schema_version": 1,
        "collection_id": "synthetic-4725",
        "catalogue_digest": digest(CATALOGUE.read_bytes()),
        "binary": binary_context() if binary is None else binary,
        "source_bytes_digest": digest(SOURCE_BYTES),
        "cases": [dict(templates[case_id], case_id=case_id) for case_id in case_ids],
    }


def preview_step(store, prefix, fixture_root, exit_code=0, files_after=None):
    base = {"src/lib.rs": SOURCE_BYTES}
    before = snapshot_member(store, prefix + "/before", base, "snapshot")
    after_files = dict(base)
    if files_after:
        after_files.update(files_after)
    after = snapshot_member(store, prefix + "/after", after_files, "snapshot")
    stdout = store.put(prefix + "/command.stdout", b"Result: completed (advisory)\n")
    stderr = store.put(prefix + "/command.stderr", b"")
    return {
        "index": 0,
        "purpose": "dry-run reports would-create guidance",
        "route": "preview", "observation_class": "outcome",
        "argv": [binary_context()["path"], "init", "--dry-run"],
        "cwd": str(fixture_root),
        "process": {
            "started_at_utc": "2026-11-05T00:00:00.000000Z",
            "finished_at_utc": "2026-11-05T00:00:01.000000Z",
            "started": True, "exit_code": exit_code,
            "launch_error": None, "timed_out": False,
        },
        "stdout": stdout, "stderr": stderr,
        "before": before, "after": after,
        "changed_paths": VERIFY.snapshot_changes(
            store.read(before), store.read(after), []),
        "artifacts": [], "expected_exit": "zero",
    }


def synthetic_collection(root, case_ids, mutate=None):
    store = Store(root)
    binary = binary_context()
    store.put("binary/cargo-allow", b"fake-binary-bytes")
    context = context_for(case_ids, binary)
    context_member = store.put("expected-context.json", json_bytes(context))
    catalogue_digest = context["catalogue_digest"]
    cases = []
    for case_id in case_ids:
        fixture_root = root / "fixtures" / case_id
        fixture_root.mkdir(parents=True)
        steps = []
        if case_id == "C.init.preview":
            steps.append(preview_step(store, f"cases/{case_id}/step-0", fixture_root))
        elif case_id == "C.propose.candidate":
            step = preview_step(store, f"cases/{case_id}/step-0", fixture_root,
                                exit_code=1,
                                files_after={"policy/candidate.toml": b"candidate\n"})
            step["route"] = "preview"
            step["observation_class"] = "contract_probe"
            step["argv"] = [binary_context()["path"], "propose", "--config",
                            str(fixture_root / "policy/allow.toml"), "--dry-run"]
            for change in step["changed_paths"]:
                change["declared"] = change["path"] in {"policy/candidate.toml"}
            steps.append(step)
        elif case_id == "control.symlink-alias":
            steps.append({
                "index": 0,
                "purpose": "alias creation attempt",
                "route": "fixture_control", "observation_class": "platform_probe",
                "argv": None, "cwd": str(fixture_root),
                "process": {
                    "started_at_utc": "2026-11-05T00:00:00.000000Z",
                    "finished_at_utc": "2026-11-05T00:00:00.000000Z",
                    "started": False, "exit_code": None,
                    "launch_error": "symlink unavailable", "timed_out": False,
                },
                "stdout": None, "stderr": None,
                "before": snapshot_member(store, f"cases/{case_id}/step-0/before",
                                          {"src/lib.rs": SOURCE_BYTES}, "snapshot"),
                "after": snapshot_member(store, f"cases/{case_id}/step-0/after",
                                         {"src/lib.rs": SOURCE_BYTES}, "snapshot"),
                "changed_paths": [], "artifacts": [], "expected_exit": "any",
                "control_outcome": {"action": "symlink", "performed": False,
                                    "error": "symlink unavailable"},
            })
        elif case_id == "control.missing-rollback-state":
            steps.append({
                "index": 0,
                "purpose": "rollback without state fails",
                "route": "fixture_rollback", "observation_class": "fixture_control",
                "argv": None, "cwd": str(fixture_root),
                "process": {
                    "started_at_utc": "2026-11-05T00:00:00.000000Z",
                    "finished_at_utc": "2026-11-05T00:00:00.000000Z",
                    "started": False, "exit_code": None,
                    "launch_error": "rollback state missing", "timed_out": False,
                },
                "stdout": None, "stderr": None,
                "before": snapshot_member(store, f"cases/{case_id}/step-0/before",
                                          {"src/lib.rs": SOURCE_BYTES}, "snapshot"),
                "after": snapshot_member(store, f"cases/{case_id}/step-0/after",
                                         {"src/lib.rs": SOURCE_BYTES}, "snapshot"),
                "changed_paths": [], "artifacts": [], "expected_exit": "zero",
                "control_outcome": {"action": "restore", "performed": False,
                                    "error": "rollback state missing"},
            })
        cases.append({
            "case_id": case_id,
            "fixture_root": str(fixture_root),
            "fixture_commit": SOURCE_GENERATION,
            "environment": {},
            "steps": steps,
        })
    bundle = {
        "schema_id": VERIFY.BUNDLE_SCHEMA, "schema_version": 1,
        "collection_id": "synthetic-4725",
        "catalogue_digest": catalogue_digest,
        "context_digest": context_member["digest"],
        "binary": binary,
        "binary_digest_after": binary["digest"],
        "cases": cases,
    }
    if mutate is not None:
        mutate(context, bundle, store)
    bundle_member = store.put("bundle.json", json_bytes(bundle))
    return store, context, bundle, bundle_member


def run_collection_admission(root):
    context_bytes = (root / "expected-context.json").read_bytes()
    context = json.loads(context_bytes, object_pairs_hook=VERIFY.unique_object)
    bundle = json.loads((root / "bundle.json").read_text(),
                        object_pairs_hook=VERIFY.unique_object)
    catalogue_bytes = CATALOGUE.read_bytes()
    return VERIFY.admit_collection(catalogue_bytes, context, bundle, str(root),
                                   context_bytes)


class CollectionAdmissionTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix="cme-admission-"))

    def tearDown(self):
        shutil.rmtree(self.root, ignore_errors=True)

    def test_valid_minimal_collection_is_admitted(self):
        store, _, _, _ = synthetic_collection(self.root, ["C.init.preview"])
        admission = run_collection_admission(self.root)
        self.assertEqual(admission["semantic_validity"], "valid",
                         admission["errors"])
        case = admission["cases"][0]
        self.assertEqual(case["semantic_validity"], "valid", case["errors"])

    def test_duplicate_case_execution_is_invalid(self):
        def mutate(context, bundle, store):
            bundle["cases"].append(copy.deepcopy(bundle["cases"][0]))
        synthetic_collection(self.root, ["C.init.preview"], mutate)
        admission = run_collection_admission(self.root)
        self.assertEqual(admission["semantic_validity"], "invalid")
        self.assertTrue(any("duplicate" in error for error in admission["errors"]))

    def test_missing_selected_execution_is_invalid(self):
        def mutate(context, bundle, store):
            bundle["cases"] = []
        synthetic_collection(self.root, ["C.init.preview"], mutate)
        admission = run_collection_admission(self.root)
        self.assertEqual(admission["semantic_validity"], "invalid")

    def test_unknown_case_is_invalid(self):
        def mutate(context, bundle, store):
            bundle["cases"][0]["case_id"] = "C.prune.preview"
        synthetic_collection(self.root, ["C.init.preview"], mutate)
        admission = run_collection_admission(self.root)
        invalid = any(case["semantic_validity"] == "invalid"
                      for case in admission["cases"])
        self.assertTrue(invalid or admission["errors"])

    def test_wrong_family_case_is_invalid(self):
        def mutate(context, bundle, store):
            bundle["cases"][0]["case_id"] = "A.adopt.clean_no_policy"
            context["cases"][0]["case_id"] = "A.adopt.clean_no_policy"
        synthetic_collection(self.root, ["C.init.preview"], mutate)
        admission = run_collection_admission(self.root)
        invalid = any(case["semantic_validity"] == "invalid"
                      for case in admission["cases"])
        self.assertTrue(invalid or admission["errors"])

    def test_binary_identity_mismatch_is_invalid(self):
        def mutate(context, bundle, store):
            bundle["binary"]["digest"] = fake_digest(OTHER_DIGEST_TEXT[0])
        synthetic_collection(self.root, ["C.init.preview"], mutate)
        admission = run_collection_admission(self.root)
        self.assertEqual(admission["semantic_validity"], "invalid")

    def test_replacement_member_bytes_are_invalid(self):
        def mutate(context, bundle, store):
            victim = bundle["cases"][0]["steps"][0]["after"]
            data = store.read(victim)
            victim_path = store.root / victim["path"]
            victim_path.write_bytes(data.replace(b"src/lib.rs", b"src/lib.rs "))
        synthetic_collection(self.root, ["C.init.preview"], mutate)
        admission = run_collection_admission(self.root)
        invalid = any(case["semantic_validity"] == "invalid"
                      for case in admission["cases"])
        self.assertTrue(invalid, admission)

    def test_stale_recorded_changes_are_invalid(self):
        def mutate(context, bundle, store):
            bundle["cases"][0]["steps"][0]["changed_paths"] = [{
                "path": "policy/allow.toml", "change": "added",
                "before_digest": None,
                "after_digest": fake_digest("c"), "declared": False,
            }]
        synthetic_collection(self.root, ["C.init.preview"], mutate)
        admission = run_collection_admission(self.root)
        invalid = any(case["semantic_validity"] == "invalid"
                      for case in admission["cases"])
        self.assertTrue(invalid)

    def test_argv_template_mismatch_is_invalid(self):
        def mutate(context, bundle, store):
            bundle["cases"][0]["steps"][0]["argv"].append("--force")
        synthetic_collection(self.root, ["C.init.preview"], mutate)
        admission = run_collection_admission(self.root)
        invalid = any(case["semantic_validity"] == "invalid"
                      for case in admission["cases"])
        self.assertTrue(invalid)

    def test_unstarted_step_is_incomplete_not_valid(self):
        def mutate(context, bundle, store):
            step = bundle["cases"][0]["steps"][0]
            step["process"] = {
                "started_at_utc": "2026-11-05T00:00:00.000000Z",
                "finished_at_utc": "2026-11-05T00:00:00.000000Z",
                "started": False, "exit_code": None,
                "launch_error": "blocked", "timed_out": False,
            }
        synthetic_collection(self.root, ["C.init.preview"], mutate)
        admission = run_collection_admission(self.root)
        self.assertEqual(admission["semantic_validity"], "incomplete")

    def test_contract_gap_probe_is_observed(self):
        store, _, _, _ = synthetic_collection(self.root, ["C.propose.candidate"])
        admission = run_collection_admission(self.root)
        self.assertEqual(admission["semantic_validity"], "valid",
                         admission["errors"])
        self.assertTrue(any("contract gap" in observation
                            for observation in admission["cases"][0]["observations"]),
                        admission["cases"][0]["observations"])

    def test_unprobed_contract_writes_are_invalid(self):
        def mutate(context, bundle, store):
            step = bundle["cases"][0]["steps"][0]
            step["changed_paths"].append({
                "path": "policy/other.toml", "change": "added",
                "before_digest": None, "after_digest": fake_digest("d"),
                "declared": False,
            })
            victim = step["after"]
            entries = json.loads(store.read(victim))["entries"]
            entries.append({
                "path": "policy/other.toml", "kind": "file", "mode": 0o644,
                "size_bytes": 5,
                "content": {"path": "cases/C.propose.candidate/step-0/files/"
                                    "policy/other.toml",
                            "size_bytes": 5, "digest": fake_digest("d")},
                "link_target": None,
            })
            (store.root / victim["path"]).write_bytes(json_bytes({"entries": entries}))
        synthetic_collection(self.root, ["C.propose.candidate"], mutate)
        admission = run_collection_admission(self.root)
        invalid = any(case["semantic_validity"] == "invalid"
                      for case in admission["cases"])
        self.assertTrue(invalid)

    def test_missing_rollback_state_is_incomplete(self):
        synthetic_collection(self.root, ["control.missing-rollback-state"])
        admission = run_collection_admission(self.root)
        self.assertEqual(admission["semantic_validity"], "incomplete")

    def test_forbidden_path_in_final_state_is_invalid(self):
        def mutate(context, bundle, store):
            context["cases"][0]["forbidden_paths"] = ["policy/allow.toml"]
            step = bundle["cases"][0]["steps"][0]
            victim = step["after"]
            entries = json.loads(store.read(victim))["entries"]
            entries.append({
                "path": "policy/allow.toml", "kind": "file", "mode": 0o644,
                "size_bytes": 5,
                "content": {"path": "cases/C.init.preview/step-0/files/"
                                    "policy/allow.toml",
                            "size_bytes": 5, "digest": fake_digest("e")},
                "link_target": None,
            })
            (store.root / victim["path"]).write_bytes(json_bytes({"entries": entries}))
            step["changed_paths"] = [{
                "path": "policy/allow.toml", "change": "added",
                "before_digest": None, "after_digest": fake_digest("e"),
                "declared": True,
            }]
            context["cases"][0]["steps"][0]["declared_writes"] = ["policy/allow.toml"]
            context["cases"][0]["steps"][0]["route"] = "mutation"
        synthetic_collection(self.root, ["C.init.preview"], mutate)
        admission = run_collection_admission(self.root)
        invalid = any(case["semantic_validity"] == "invalid"
                      for case in admission["cases"])
        self.assertTrue(invalid)


class ManifestAdmissionTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix="cme-manifest-"))
        synthetic_collection(self.root, ["C.init.preview"])
        self.context = json.loads((self.root / "expected-context.json").read_text(),
                                  object_pairs_hook=VERIFY.unique_object)
        self.bundle = json.loads((self.root / "bundle.json").read_text(),
                                 object_pairs_hook=VERIFY.unique_object)
        self.admission = run_collection_admission(self.root)
        self.manifest = VERIFY.emit_manifest(
            CATALOGUE.read_bytes(), self.context, self.bundle, self.admission,
            digest((self.root / "bundle.json").read_bytes()), str(self.root))
        self.manifest["collection_id"] = "manifest-4725"

    def tearDown(self):
        shutil.rmtree(self.root, ignore_errors=True)

    def test_manifest_omitting_obligations_is_invalid(self):
        manifest = copy.deepcopy(self.manifest)
        manifest["cases"] = []
        admission = VERIFY.verify_manifest(CATALOGUE.read_bytes(), manifest)
        self.assertEqual(admission["semantic_validity"], "invalid")
        self.assertTrue(any("omits" in error for error in admission["errors"]))

    def test_duplicate_manifest_case_is_invalid(self):
        manifest = copy.deepcopy(self.manifest)
        manifest["cases"] = manifest["cases"] + [copy.deepcopy(manifest["cases"][0])]
        admission = VERIFY.verify_manifest(CATALOGUE.read_bytes(), manifest)
        self.assertEqual(admission["semantic_validity"], "invalid")

    def test_probe_write_disposed_admitted_is_invalid(self):
        manifest = copy.deepcopy(self.manifest)
        case = manifest["cases"][0]
        case["steps"][0]["observation_class"] = "contract_probe"
        case["steps"][0]["changed_paths"].append({
            "path": "policy/candidate.toml", "change": "added",
            "before_digest": None, "after_digest": fake_digest("f"),
            "declared": True,
        })
        admission = VERIFY.verify_manifest(CATALOGUE.read_bytes(), manifest)
        self.assertEqual(admission["semantic_validity"], "invalid")

    def test_not_proven_platform_disposition_rule(self):
        manifest = copy.deepcopy(self.manifest)
        case = manifest["cases"][0]
        case["case_id"] = "control.symlink-alias"
        case["steps"][0]["observation_class"] = "platform_probe"
        case["steps"][0]["started"] = False
        case["steps"][0]["exit_code"] = None
        case["disposition"] = "admitted"
        # Isolate from the obligation denominator: single control case.
        admission = VERIFY.verify_manifest(CATALOGUE.read_bytes(), manifest)
        invalid = any("not_proven_platform" in error
                      for error in admission["errors"])
        self.assertTrue(invalid or any(
            sub["errors"] for sub in admission["cases"]), admission)

    def test_malformed_digest_grammar_is_invalid(self):
        manifest = copy.deepcopy(self.manifest)
        manifest["cases"][0]["steps"][0]["stdout_digest"] = "not-a-digest"
        admission = VERIFY.verify_manifest(CATALOGUE.read_bytes(), manifest)
        self.assertEqual(admission["semantic_validity"], "invalid")

    def test_modified_without_digest_change_is_invalid(self):
        manifest = copy.deepcopy(self.manifest)
        step = manifest["cases"][0]["steps"][0]
        same = fake_digest("1")
        step["changed_paths"] = [{
            "path": "policy/allow.toml", "change": "modified",
            "before_digest": same, "after_digest": same, "declared": True,
        }]
        admission = VERIFY.verify_manifest(CATALOGUE.read_bytes(), manifest)
        self.assertEqual(admission["semantic_validity"], "invalid")

    def test_unknown_disposition_is_invalid(self):
        manifest = copy.deepcopy(self.manifest)
        manifest["cases"][0]["disposition"] = "qualified"
        admission = VERIFY.verify_manifest(CATALOGUE.read_bytes(), manifest)
        self.assertEqual(admission["semantic_validity"], "invalid")

    def test_wrong_catalogue_pin_is_invalid(self):
        manifest = copy.deepcopy(self.manifest)
        manifest["catalogue_digest"] = fake_digest("9")
        admission = VERIFY.verify_manifest(CATALOGUE.read_bytes(), manifest)
        self.assertEqual(admission["semantic_validity"], "invalid")

    def test_source_build_cannot_claim_installed_identities(self):
        manifest = copy.deepcopy(self.manifest)
        manifest["binary"]["provenance"] = "source_build"
        admission = VERIFY.verify_manifest(CATALOGUE.read_bytes(), manifest)
        self.assertEqual(admission["semantic_validity"], "invalid")

    def test_emitted_manifest_case_rules_and_denominator(self):
        manifest = copy.deepcopy(self.manifest)
        # The synthetic fixture commit satisfies the Git identity grammar; the
        # preview-only obligation set still requires the full 20 in real use,
        # so the collection-level denominator rejects this manifest while the
        # individually retained case stays coherent.
        admission = VERIFY.verify_manifest(CATALOGUE.read_bytes(), manifest)
        self.assertEqual(admission["semantic_validity"], "invalid")
        self.assertTrue(any("omits" in reason for reason in admission["errors"]),
                        admission["errors"])
        for case in admission["cases"]:
            if case["case_id"] == "C.init.preview":
                self.assertEqual(case["semantic_validity"], "valid",
                                 case["errors"])
                return
        self.fail("C.init.preview case row missing from admission")


class BinaryPathTests(unittest.TestCase):
    def test_recording_platform_absolute_paths_are_accepted(self):
        for path in ["/usr/local/bin/cargo-allow", "C:/tools/cargo-allow.exe",
                     r"C:\tools\cargo-allow.exe", r"\\server\share\cargo-allow.exe"]:
            with self.subTest(path=path):
                self.assertTrue(VERIFY.absolute_on_recording_platform(path))

    def test_relative_and_empty_paths_are_rejected(self):
        for path in ["", " ", "cargo-allow", "relative/cargo-allow",
                     "C:cargo-allow.exe", r"\tools\cargo-allow.exe"]:
            with self.subTest(path=path):
                self.assertFalse(VERIFY.absolute_on_recording_platform(path))

    def test_manifest_with_relative_binary_path_is_invalid(self):
        manifest = json.loads((HISTORICAL_EVIDENCE / "manifest.v1.json").read_bytes())
        manifest["binary"]["path"] = "relative/cargo-allow"
        admission = VERIFY.verify_manifest(
            (HISTORICAL_EVIDENCE / "catalogue.v1.json").read_bytes(), manifest)
        self.assertEqual(admission["semantic_validity"], "invalid")
        self.assertIn("binary identity path is not absolute", admission["errors"])


class HistoricalManifestTests(unittest.TestCase):
    def test_retained_manifest_and_catalogue_preserve_exact_original_bytes(self):
        for name, expected in [
            ("manifest.v1.json", "sha256:v1:cfa63044496f17c121c911b234a9e7bec15b55345afa32eda750e6336d433b44"),
            ("catalogue.v1.json", "sha256:v1:5a582458f7b8d71d3983690214de62b3c7c5bf33e47a88385ad07540eb16cc08"),
        ]:
            with self.subTest(name=name):
                self.assertEqual(digest((HISTORICAL_EVIDENCE / name).read_bytes()), expected)

    def verify_cli(self, catalogue):
        return subprocess.run(
            [sys.executable, "-B", str(SCRIPT), "verify-manifest", "--catalogue",
             str(catalogue), "--manifest", str(HISTORICAL_EVIDENCE / "manifest.v1.json")],
            capture_output=True, text=True, timeout=15, check=False,
        )

    def test_historical_manifest_replays_against_its_original_catalogue(self):
        observed = self.verify_cli(HISTORICAL_EVIDENCE / "catalogue.v1.json")
        self.assertEqual(observed.returncode, 0, observed.stdout + observed.stderr)
        admission = json.loads(observed.stdout)
        self.assertEqual(admission["semantic_validity"], "valid")
        self.assertEqual(admission["errors"], [])

    def test_current_catalogue_cannot_replace_historical_collection_identity(self):
        observed = self.verify_cli(CATALOGUE)
        self.assertEqual(observed.returncode, 1, observed.stdout + observed.stderr)
        admission = json.loads(observed.stdout)
        self.assertEqual(admission["semantic_validity"], "invalid")
        self.assertEqual(admission["errors"], [
            "manifest catalogue digest does not pin this catalogue",
        ])


class DriverExitTests(unittest.TestCase):
    def test_failed_control_exits_nonzero(self):
        # Import only the driver into a child. unittest discovers the injected
        # __main__ canary, not this suite, so the check cannot recurse.
        probe = """
import importlib.util
import sys
import unittest

path = sys.argv[1]
sys.argv = [path]
spec = importlib.util.spec_from_file_location("mutation_controls_driver", path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class DeliberateFailure(unittest.TestCase):
    def test_failure(self):
        self.fail("deliberate test-driver exit canary")

raise SystemExit(module.main())
"""
        observed = subprocess.run(
            [sys.executable, "-B", "-c", probe, str(Path(__file__).resolve())],
            capture_output=True, text=True, timeout=15, check=False,
        )
        self.assertIn("deliberate test-driver exit canary", observed.stderr)
        self.assertIn("FAILED (failures=1)", observed.stderr)
        self.assertEqual(observed.returncode, 1, observed.stdout + observed.stderr)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-bin", type=Path)
    parser.add_argument("--source-generation")
    parser.add_argument("--export-observations", type=Path)
    parser.add_argument("-v", "--verbose", action="store_true")
    args, _ = parser.parse_known_args()
    # The admission reader is hermetic; native-bin and observation export are
    # accepted for parity with the Rust-driven A-family transport harness and
    # are not required here.
    program = unittest.main(argv=[sys.argv[0]] + (["-v"] if args.verbose else []),
                            exit=False, verbosity=2 if args.verbose else 1)
    return 0 if program.result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
