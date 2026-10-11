#!/usr/bin/env python3
"""Transport controls and actual native admission rejection tests for #3149.

The Python harness checks observations from the Rust reader. It does not
implement a second semantic validator or grant release qualification.
"""

import argparse
import copy
from contextlib import redirect_stdout
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


sys.dont_write_bytecode = True
SCRIPT = Path(__file__).resolve().with_name("command-case-evidence.py")
REPOSITORY = SCRIPT.parent.parent
CATALOGUE = REPOSITORY / "docs/release/core-command-migration-cases.v1.json"
SPEC = importlib.util.spec_from_file_location("command_case_evidence", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise SystemExit("cannot load the production collector")
COLLECT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COLLECT)
NATIVE = None
SOURCE_GENERATION = None
EXPORT = None


def git_path():
    value = shutil.which("git")
    if value is None:
        raise RuntimeError("Git is required for the controlled tracked fixtures")
    return Path(value).resolve()


def arguments(root, binary, cases=None):
    return argparse.Namespace(
        binary=Path(binary), git=git_path(), catalogue=CATALOGUE,
        output_dir=root / "collection", collection_id="command-case-test",
        tool_version="0.2.0", source_generation=SOURCE_GENERATION or "1" * 40,
        provenance="source_build", candidate_identity=None, install_identity=None,
        case=cases, timeout=30.0,
    )


class TransportTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="command-case-transport-")
        self.root = Path(self.temporary.name).resolve()

    def tearDown(self):
        self.temporary.cleanup()

    def test_exclusive_root_preserves_prior_owner_and_refuses_checkout(self):
        owned = COLLECT.owned_root(self.root / "owned")
        canary = owned / "prior-owner"
        canary.write_bytes(b"prior owner")
        with self.assertRaises(FileExistsError):
            COLLECT.owned_root(owned)
        self.assertEqual(canary.read_bytes(), b"prior owner")
        (self.root / ".git").mkdir()
        with self.assertRaisesRegex(ValueError, "outside a repository"):
            COLLECT.owned_root(self.root / "inside")
        self.assertFalse((self.root / "inside").exists())

    def test_non_directory_and_duplicate_member_preserve_bytes(self):
        target = self.root / "existing"
        target.write_bytes(b"keep")
        with self.assertRaises(FileExistsError):
            COLLECT.owned_root(target)
        store = COLLECT.Store(self.root)
        member = store.put("member.json", b"first")
        with self.assertRaises(FileExistsError):
            store.put("member.json", b"second")
        self.assertEqual((self.root / member["path"]).read_bytes(), b"first")
        self.assertEqual(target.read_bytes(), b"keep")

    def test_member_paths_duplicate_json_and_nonregular_outputs_are_refused(self):
        for path in ("../escape", "/absolute", "x//y", "x/./y", "x\\y", "C:alias"):
            with self.subTest(path=path), self.assertRaises(ValueError):
                COLLECT.safe_relative(path)
        duplicate = self.root / "duplicate.json"
        duplicate.write_bytes(b'{"a":1,"a":2}')
        with self.assertRaisesRegex(ValueError, "duplicate"):
            COLLECT.load_json(duplicate)
        directory = self.root / "directory"
        directory.mkdir()
        store = COLLECT.Store(self.root)
        with self.assertRaisesRegex(ValueError, "nonregular"):
            COLLECT.capture_optional(store, "case", "summary", directory)
        self.assertIsNone(COLLECT.capture_optional(store, "case", "detail", directory, True))
        self.assertIsNone(COLLECT.capture_optional(store, "case", "summary", self.root / "missing"))

    @unittest.skipUnless(os.name != "nt", "symlink fixture needs no Windows privilege")
    def test_symlink_inputs_outputs_and_parent_are_refused(self):
        target = self.root / "target"
        target.write_bytes(b"keep")
        link = self.root / "link"
        link.symlink_to(target)
        with self.assertRaisesRegex(ValueError, "symlink"):
            COLLECT.regular_bytes(link, 20)
        with self.assertRaisesRegex(ValueError, "symlink"):
            COLLECT.capture_optional(COLLECT.Store(self.root), "case", "summary", link)
        parent = self.root / "parent"
        parent.symlink_to(self.root, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symlink"):
            COLLECT.owned_root(parent / "escape")
        self.assertEqual(target.read_bytes(), b"keep")

    def test_child_environment_is_explicit_without_mutating_parent(self):
        parent = dict(os.environ)
        env = COLLECT.child_environment(self.root / "home", git_path())
        self.assertEqual(os.environ, parent)
        self.assertNotIn("GIT_DIR", env)
        self.assertNotIn("GIT_WORK_TREE", env)
        self.assertFalse(any(key.startswith("CARGO_ALLOW") for key in env))
        self.assertEqual(env["GIT_CONFIG_GLOBAL"], os.devnull)
        store = COLLECT.Store(self.root)
        process, stdout, stderr = COLLECT.run_process(
            [sys.executable, "-c", "import json,os,sys; print(json.dumps(dict(os.environ),sort_keys=True)); print(sys.stdin.read(),file=sys.stderr)"],
            self.root, env, 10, store, "process",
        )
        self.assertEqual(process["exit_code"], 0)
        observed = json.loads((self.root / stdout["path"]).read_bytes())
        for key, value in env.items():
            self.assertEqual(observed[key], value)
        self.assertNotIn("GIT_DIR", observed)
        self.assertEqual((self.root / stderr["path"]).read_bytes(), b"\n")

    def test_one_supplied_binary_invocation_and_fresh_absence_are_retained(self):
        # Python deliberately refuses cargo-allow argv. This tests process
        # transport and absence only; native semantic tests use cargo-allow.
        args = arguments(self.root, Path(sys.executable).resolve(), ["A.audit.clean_no_policy"])
        with mock.patch.object(COLLECT, "run_process", wraps=COLLECT.run_process) as calls:
            with redirect_stdout(io.StringIO()):
                COLLECT.collect(args)
        binary_calls = [call for call in calls.call_args_list if call.args[0][0] == str(args.binary)]
        self.assertEqual(len(binary_calls), 1)
        bundle = json.loads((args.output_dir / "bundle.json").read_bytes())
        case = bundle["cases"][0]
        self.assertTrue(case["process"]["started"])
        self.assertNotEqual(case["process"]["exit_code"], 0)
        self.assertIsNone(case["detail"])
        self.assertIsNone(case["summary"])
        self.assertIsNone(case["receipt"])
        self.assertEqual(bundle["binary_digest_after"], bundle["binary"]["digest"])
        self.assertEqual(case["context"]["source_snapshot_digest"], case["before"]["digest"])

    def test_unstarted_and_timed_out_processes_remain_distinct(self):
        store = COLLECT.Store(self.root)
        env = COLLECT.child_environment(self.root / "home", git_path())
        unstarted, _, _ = COLLECT.run_process(
            [str(self.root / "absent-binary")], self.root, env, 1, store, "unstarted",
        )
        self.assertFalse(unstarted["started"])
        self.assertIsNone(unstarted["exit_code"])
        self.assertIsNotNone(unstarted["launch_error"])
        timeout, _, _ = COLLECT.run_process(
            [sys.executable, "-c", "import time; time.sleep(2)"],
            self.root, env, 0.05, store, "timeout",
        )
        self.assertTrue(timeout["started"])
        self.assertTrue(timeout["timed_out"])
        self.assertIsNotNone(timeout["exit_code"])
        self.assertIsNone(timeout["launch_error"])

    def test_case_and_provenance_rejections_happen_before_root_allocation(self):
        for cases in (["unknown"], ["A.audit.clean_no_policy"] * 2, ["D.diff.exact_range"]):
            args = arguments(self.root, Path(sys.executable).resolve(), cases)
            with self.subTest(cases=cases), self.assertRaises(ValueError):
                COLLECT.collect(args)
            self.assertFalse(args.output_dir.exists())
        args = arguments(self.root, Path(sys.executable).resolve(), ["A.audit.clean_no_policy"])
        args.install_identity = "sha256:v1:" + "a" * 64
        with self.assertRaisesRegex(ValueError, "source build"):
            COLLECT.collect(args)
        self.assertFalse(args.output_dir.exists())


@unittest.skipIf(NATIVE is None, "native binary supplied by the Rust integration target")
class NativeAdmissionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix="command-case-native-tests-")
        cls.root = Path(cls.temporary.name).resolve()
        args = arguments(cls.root, NATIVE)
        with redirect_stdout(io.StringIO()):
            COLLECT.collect(args)
        cls.collection = args.output_dir
        cls.context = json.loads((cls.collection / "expected-context.json").read_bytes())
        cls.bundle = json.loads((cls.collection / "bundle.json").read_bytes())

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def setUp(self):
        self.restore = {}
        self.remove = []

    def tearDown(self):
        for path in self.remove:
            path.unlink()
        for path, data in self.restore.items():
            path.write_bytes(data)

    def selected(self, ids):
        context, bundle = copy.deepcopy(self.context), copy.deepcopy(self.bundle)
        context["cases"] = [case for case in context["cases"] if case["case_id"] in ids]
        bundle["cases"] = [case for case in bundle["cases"] if case["context"]["case_id"] in ids]
        return context, bundle

    def read_member(self, member):
        return (self.collection / member["path"]).read_bytes()

    def replace_member(self, context, case, role, data, repin=True):
        member = case[role]
        path = self.collection / member["path"]
        self.restore.setdefault(path, path.read_bytes())
        path.write_bytes(data)
        member["size_bytes"], member["digest"] = len(data), COLLECT.digest(data)
        if repin:
            case["context"]["output_digests"][role] = member["digest"]
            expected = next(item for item in context["cases"] if item["case_id"] == case["context"]["case_id"])
            expected["output_digests"][role] = member["digest"]

    def admit(self, context, bundle, catalogue=None, raw_context=None, raw_bundle=None, output=None):
        context_bytes = COLLECT.json_bytes(context) if raw_context is None else raw_context
        bundle["context_digest"] = COLLECT.digest(context_bytes)
        bundle_bytes = COLLECT.json_bytes(bundle) if raw_bundle is None else raw_bundle
        context_path = self.collection / "test-context.json"
        bundle_path = self.collection / "test-bundle.json"
        catalogue_path = CATALOGUE
        if catalogue is not None:
            catalogue_path = self.collection / "test-catalogue.json"
            catalogue_path.write_bytes(catalogue)
        context_path.write_bytes(context_bytes)
        bundle_path.write_bytes(bundle_bytes)
        argv = [str(NATIVE), "command-migration-evidence", "--catalogue", str(catalogue_path),
                "--expected-context", str(context_path), "--bundle", str(bundle_path)]
        if output is not None:
            argv.extend(["--output", str(output)])
        env = dict(os.environ)
        for key in list(env):
            if key.startswith("GIT_") or key.startswith("CARGO_ALLOW"):
                del env[key]
        observed = subprocess.run(argv, cwd=self.collection, env=env, capture_output=True, timeout=30)
        if output is not None:
            return observed, None
        self.assertTrue(observed.stdout, observed.stderr.decode(errors="replace"))
        return observed, json.loads(observed.stdout)

    def invalid(self, context, bundle, text=None, **kwargs):
        observed, result = self.admit(context, bundle, **kwargs)
        self.assertEqual(observed.returncode, 1, observed.stderr.decode(errors="replace"))
        self.assertEqual(result["semantic_validity"], "invalid", result)
        self.assertEqual(result["qualification"], "partial")
        if text:
            self.assertIn(text, json.dumps(result))
        return result

    def test_actual_first_family_valid_observations_remain_partial_qualification(self):
        # These live routes work both before and after the separately owned
        # #4460 sidecar repair. Its three domain-error routes are not silently
        # assumed fixed here; their full producer proof is retained separately.
        ids = [case["context"]["case_id"] for case in self.bundle["cases"]
               if not any(part in case["context"]["case_id"] for part in
                          ("instrument_failure", "invalid_policy", "partial_require_clean"))
               and case["context"]["case_id"] != "A.adopt.partial_inventory"
               and not case["context"]["case_id"].startswith("A.check.")]
        ids.extend(["A.check.healthy_policy", "A.check.new_finding", "A.check.partial_inventory"])
        context, bundle = self.selected(ids)
        observed, result = self.admit(context, bundle)
        self.assertEqual(observed.returncode, 0, result)
        self.assertEqual(result["semantic_validity"], "valid", result)
        self.assertEqual(result["qualification"], "partial")
        self.assertEqual(len(result["dimensions"]), 18)
        self.assertGreater(len(result["missing_cases"]), 70)
        self.assertTrue(all(item["detail_reconciled"] for item in result["cases"]))
        by_id = {item["case_id"]: item for item in result["cases"]}
        self.assertEqual(by_id["A.check.healthy_policy"]["observed_result_class"], "findings")
        self.assertEqual(by_id["A.doctor.new_finding"]["observed_result_class"], "completed")
        self.assertEqual(by_id["A.check.partial_inventory"]["observed_completeness"], "partial")
        self.assertTrue(all(item["binding_gaps"] for item in result["cases"]))
        if EXPORT is not None:
            EXPORT.mkdir()
            for name, data in [("catalogue", CATALOGUE.read_bytes()), ("context", COLLECT.json_bytes(context)),
                               ("bundle", COLLECT.json_bytes(bundle)), ("admission", observed.stdout)]:
                (EXPORT / (name + ".json")).write_bytes(data)

    def test_actual_hard_failures_retain_typed_error_receipt_and_no_detail(self):
        ids = ["A." + command + ".instrument_failure" for command in ("adopt", "doctor", "audit", "check")]
        ids += ["A.check.clean_no_policy", "A.check.findings_no_policy", "A.check.invalid_policy", "A.audit.invalid_policy"]
        context, bundle = self.selected(ids)
        observed, result = self.admit(context, bundle)
        self.assertEqual(observed.returncode, 0, result)
        self.assertEqual(result["semantic_validity"], "incomplete", result)
        self.assertTrue(all(item["process_observed"] and not item["detail_reconciled"] for item in result["cases"]))
        self.assertTrue(all(item["semantic_validity"] == "incomplete" for item in result["cases"]))

    def test_missing_duplicate_unknown_required_cases_and_false_na_are_not_admitted(self):
        for mode in ("missing", "duplicate", "unknown", "false_na"):
            context, bundle = self.selected(["A.audit.healthy_policy"])
            if mode == "missing":
                bundle["cases"] = []
            elif mode == "duplicate":
                bundle["cases"].append(copy.deepcopy(bundle["cases"][0]))
            elif mode == "unknown":
                bundle["cases"][0]["context"]["case_id"] = "A.unknown"
            else:
                bundle["not_applicable"] = ["A.audit.healthy_policy"]
            with self.subTest(mode=mode):
                self.invalid(context, bundle)

    def test_shrunken_catalogue_and_schema_or_duplicate_json_are_rejected(self):
        context, bundle = self.selected(["A.audit.healthy_policy"])
        smaller = json.loads(CATALOGUE.read_bytes())
        smaller["cases"] = smaller["cases"][:1]
        catalogue = COLLECT.json_bytes(smaller)
        context["catalogue_digest"] = bundle["catalogue_digest"] = COLLECT.digest(catalogue)
        self.invalid(context, bundle, "accepted denominator", catalogue=catalogue)
        context, bundle = self.selected(["A.audit.healthy_policy"])
        bundle["schema_version"] = 2
        self.invalid(context, bundle, "schema generation")
        context, bundle = self.selected(["A.audit.healthy_policy"])
        self.invalid(context, bundle, "duplicate JSON", raw_bundle=b'{"schema_id":"one","schema_id":"two"}')

    def test_binary_source_install_context_and_ambient_selector_mismatches(self):
        for key, value in [("digest", "sha256:v1:" + "a" * 64), ("source_generation", "a" * 40),
                           ("path", str(self.root / "foreign")), ("tool_version", "99.0")]:
            context, bundle = self.selected(["A.audit.healthy_policy"])
            bundle["binary"][key] = value
            with self.subTest(binary_key=key):
                self.invalid(context, bundle, "identity mismatch")
        for key, value in [("cwd", str(self.root)), ("mode", "audit"), ("profile", "invented"),
                           ("resolved_config_identity", "sha256:v1:" + "a" * 64),
                           ("policy_digest", "sha256:v1:" + "a" * 64)]:
            context, bundle = self.selected(["A.audit.healthy_policy"])
            bundle["cases"][0]["context"][key] = value
            with self.subTest(case_key=key):
                self.invalid(context, bundle, "identity mismatch")
        context, bundle = self.selected(["A.audit.healthy_policy"])
        for item in [context["cases"][0], bundle["cases"][0]["context"]]:
            item["environment"]["GIT_DIR"] = "/foreign"
        self.invalid(context, bundle, "ambient")
        context, bundle = self.selected(["A.audit.healthy_policy"])
        for binary in (context["binary"], bundle["binary"]):
            binary["install_identity"] = "sha256:v1:" + "a" * 64
        self.invalid(context, bundle, "source build")

    def test_rehashed_foreign_detail_is_rejected_even_after_transport_repin(self):
        context, bundle = self.selected(["A.doctor.healthy_policy"])
        case = bundle["cases"][0]
        foreign = next(item for item in self.bundle["cases"] if item["context"]["case_id"] == "A.doctor.partial_inventory")
        self.replace_member(context, case, "detail", self.read_member(foreign["detail"]))
        self.invalid(context, bundle)

    def test_rehashed_class_coverage_actions_effects_and_proof_contradictions(self):
        for key, value in [("result_class", "completed"), ("completeness", "partial"),
                           ("primary_action", None), ("next_proof", None),
                           ("mode", "strict"), ("profile", "invented")]:
            context, bundle = self.selected(["A.audit.healthy_policy"])
            case = bundle["cases"][0]
            summary = json.loads(self.read_member(case["summary"]))
            summary[key] = value
            self.replace_member(context, case, "summary", COLLECT.json_bytes(summary))
            with self.subTest(key=key):
                self.invalid(context, bundle)
            self.tearDown()
            self.setUp()
        context, bundle = self.selected(["A.audit.healthy_policy"])
        case = bundle["cases"][0]
        summary = json.loads(self.read_member(case["summary"]))
        summary["operation_effects"]["writes_repository"] = True
        summary["operation_effects"]["write_paths"] = ["policy/allow.toml"]
        self.replace_member(context, case, "summary", COLLECT.json_bytes(summary))
        self.invalid(context, bundle)

    def test_hard_error_receipt_and_claim_boundary_cannot_be_rehashed_into_authority(self):
        for role in ("receipt", "summary"):
            context, bundle = self.selected(["A.check.instrument_failure"])
            case = bundle["cases"][0]
            value = json.loads(self.read_member(case[role]))
            if role == "receipt":
                value["diagnostic"] = "foreign diagnostic"
            else:
                value["claim_boundary"]["statement"] = "release qualification"
            self.replace_member(context, case, role, COLLECT.json_bytes(value))
            with self.subTest(role=role):
                self.invalid(context, bundle)
            self.tearDown()
            self.setUp()

    def test_size_digest_truncation_member_alias_and_escape_are_rejected(self):
        for mode in ("size", "digest", "missing", "alias", "escape", "truncated"):
            context, bundle = self.selected(["A.audit.healthy_policy"])
            case = bundle["cases"][0]
            if mode == "size":
                case["detail"]["size_bytes"] += 1
            elif mode == "digest":
                case["detail"]["digest"] = "sha256:v1:" + "0" * 64
            elif mode == "missing":
                case["detail"]["path"] = "absent.json"
            elif mode == "alias":
                case["detail"] = copy.deepcopy(case["summary"])
            elif mode == "escape":
                case["detail"]["path"] = "../outside.json"
            else:
                self.replace_member(context, case, "detail", b'{"schema_id":')
            with self.subTest(mode=mode):
                self.invalid(context, bundle)
            self.tearDown()
            self.setUp()

    @unittest.skipUnless(os.name != "nt", "symlink/hardlink control requires Unix link support")
    def test_symlink_hardlink_and_new_output_prior_owner_are_preserved(self):
        context, bundle = self.selected(["A.audit.healthy_policy"])
        case = bundle["cases"][0]
        original = self.collection / case["detail"]["path"]
        alias = self.collection / "alias.json"
        alias.symlink_to(original)
        self.remove.append(alias)
        case["detail"]["path"] = "alias.json"
        self.invalid(context, bundle, "symlink")
        alias.unlink()
        self.remove.remove(alias)
        os.link(original, alias)
        self.remove.append(alias)
        self.invalid(context, bundle, "hard-link")
        alias.unlink()
        self.remove.remove(alias)
        prior = self.collection / "prior-result.json"
        prior.write_bytes(b"prior owner")
        observed, _ = self.admit(context, bundle, output=prior)
        self.assertNotEqual(observed.returncode, 0)
        self.assertEqual(prior.read_bytes(), b"prior owner")
        parent = self.collection / "output-parent"
        parent.symlink_to(self.root, target_is_directory=True)
        self.remove.append(parent)
        observed, _ = self.admit(context, bundle, output=parent / "escaped-result.json")
        self.assertNotEqual(observed.returncode, 0)
        self.assertFalse((self.root / "escaped-result.json").exists())

    def test_unstarted_cancelled_and_missing_sidecar_cannot_become_green(self):
        for mode in ("unstarted", "cancelled", "missing_summary"):
            context, bundle = self.selected(["A.audit.healthy_policy"])
            case = bundle["cases"][0]
            if mode == "unstarted":
                case["process"].update(started=False, exit_code=None, launch_error="test launch refused", timed_out=False)
                for role in ("detail", "summary", "receipt"):
                    case[role] = None
                    case["context"]["output_digests"][role] = context["cases"][0]["output_digests"][role] = None
            elif mode == "cancelled":
                case["process"].update(exit_code=-9, timed_out=True)
            else:
                case["summary"] = None
                case["context"]["output_digests"]["summary"] = context["cases"][0]["output_digests"]["summary"] = None
            observed, result = self.admit(context, bundle)
            with self.subTest(mode=mode):
                self.assertEqual(observed.returncode, 0, result)
                self.assertEqual(result["semantic_validity"], "incomplete", result)
                self.assertEqual(result["qualification"], "partial")
                self.assertFalse(result["cases"][0]["detail_reconciled"])
                self.assertIn("A.audit.healthy_policy", result["missing_cases"])

    def test_unexpected_retained_source_mutation_is_not_accepted(self):
        context, bundle = self.selected(["A.audit.healthy_policy"])
        case = bundle["cases"][0]
        snapshot = json.loads(self.read_member(case["after"]))
        source = next(item for item in snapshot["entries"] if item["path"] == "src/lib.rs")
        source["mode"] ^= 0o100
        path = self.collection / case["after"]["path"]
        self.restore[path] = path.read_bytes()
        data = COLLECT.json_bytes(snapshot)
        path.write_bytes(data)
        case["after"]["size_bytes"], case["after"]["digest"] = len(data), COLLECT.digest(data)
        self.invalid(context, bundle, "mutation")


def main():
    global NATIVE, SOURCE_GENERATION, EXPORT
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-bin", type=Path)
    parser.add_argument("--source-generation")
    parser.add_argument("--export-observations", type=Path)
    args = parser.parse_args()
    NATIVE = args.native_bin.resolve() if args.native_bin is not None else None
    SOURCE_GENERATION = args.source_generation
    EXPORT = args.export_observations
    NativeAdmissionTests.__unittest_skip__ = NATIVE is None
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(TransportTests)
    if NATIVE is not None:
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(NativeAdmissionTests))
    return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())
