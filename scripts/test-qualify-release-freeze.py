#!/usr/bin/env python3
"""Intercept the real qualifier/provider; --cargo-allow adds actual Rust gates.

The ordinary Python suite tests HTTP selection and bounded transport only. Its
bridge probe records inputs and makes no eligibility decision. The separately
invoked native suite prepares and qualifies through the actual Rust binary;
missing native tooling is an error, never a successful skip.
"""

from __future__ import annotations

import argparse
import contextlib
import copy
from email.utils import format_datetime
from datetime import datetime, timezone
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest import mock
from urllib.parse import urlsplit

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent.parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError("fixture module unavailable")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


BASE_FIXTURE = load("freeze_provider_fixture", ROOT / "scripts/test-release-operation-store.py")
DRIVER = load("freeze_qualifier", ROOT / "scripts/qualify-release-freeze.py")
STORE = DRIVER.store
BASE_FIXTURE.STORE = STORE
NOW = BASE_FIXTURE.NOW
HEAD = "a" * 40
TREE = "b" * 40
BASE = "c" * 40
REVIEWED = "d" * 40
API_BASE = BASE_FIXTURE.BASE


class World(BASE_FIXTURE.Provider):
    def __init__(self, directory, *, head=HEAD, tree=TREE, base=BASE, reviewed=REVIEWED,
                 graph=None, graph_digest="sha256:v1:" + "1" * 64):
        super().__init__()
        self.directory = directory
        self.head, self.tree, self.base, self.reviewed = head, tree, base, reviewed
        self.refs["refs/heads/main"] = head
        self.commits.update({sha:{"sha":sha, "tree":{"sha":tree}, "parents":[{"sha":base}]}
                             for sha in {head, base, reviewed}})
        self.commits[head]["parents"] = [{"sha":base}, {"sha":reviewed}]
        self.objects = {}
        self.routes = {}
        self.review = {"id":606, "commit_id":reviewed, "state":"COMMENTED",
                       "user":{"id":404, "login":"release-operator"},
                       "body":"Selected exact synthetic independent source review.",
                       "submitted_at":BASE_FIXTURE.date(NOW - 20)}
        self.record = {
            "schema_id":"cargo-allow.post-merge-qualification.v1", "schema_version":1,
            "qualification_id":"synthetic-qualified-source",
            "reviewed":{"base_sha":base, "head_sha":reviewed, "merge_base_sha":base, "tree_sha":tree},
            "merged":{"pr_number":1, "merge_commit_sha":head, "merge_tree_sha":tree,
                      "current_main_commit_sha":head, "current_main_tree_sha":tree,
                      "merge_method":"MergeCommit", "merge_parents":[base, reviewed]},
            "changed_files":[], "semantic_owners":["#2501"], "premerge_evidence_digest":graph_digest,
            "preserved_evidence_nodes":([] if graph is None else graph["required_node_ids"]),
            "invalidated_evidence_nodes":[], "required_rerun_set":[],
            "created_at_utc":BASE_FIXTURE.date(NOW - 3), "claim_boundary":["synthetic typed fixture"], "limitations":[],
        }
        graph = {"protocol_probe":"the bridge is not evaluated in Python-only controls"} if graph is None else graph
        reviewed_object = self.add_object(501, {"reviewed-graph.json":BASE_FIXTURE.encode(graph)},
                                         [{"logical_id":"reviewed-graph", "role":"EvidenceGraph", "path":"reviewed-graph.json"}],
                                         commit=reviewed, run=91, job=93)
        original = self.add_object(505, {"one.json":b"same", "two.json":b"same"},
                                  [{"logical_id":"first", "role":"BoundedData", "path":"one.json"},
                                   {"logical_id":"second", "role":"BoundedData", "path":"two.json"}])
        self.selection = {
            "repository_id":41, "anchor_commit":head, "anchor_tree":tree,
            "download_hosts":["artifacts.example.test"], "qualification_actor":{"id":404, "login":"release-operator"},
            "native_review":{"pr_number":1, "review_id":606, "actor_id":404, "actor_login":"release-operator",
                             "body_digest":BASE_FIXTURE.digest(self.review["body"].encode())},
            "reviewed_graph":reviewed_object, "artifacts":[original],
            "authorization_window_end_utc":BASE_FIXTURE.date(NOW + 300),
            "maximum_observation_age_seconds":600,
        }
        self.sync_source()

    def sync_source(self):
        self.source_body = BASE_FIXTURE.encode(self.record)
        self.source["body"] = self.source_body.decode()
        self.selection["qualification_source"] = self.source_input()

    def add_object(self, object_id, files, members, *, commit=None, run=101, job=303, prepared=False):
        identity = {**BASE_FIXTURE.producer(), "commit_sha":commit or self.head, "tree_sha":self.tree,
                    "run_id":run, "job_id":str(job)}
        metadata = copy.deepcopy(self.artifact)
        metadata.update({"id":object_id, "name":f"selected-{object_id}",
                         "created_at":BASE_FIXTURE.date(NOW if prepared else NOW - 10),
                         "workflow_run":{"id":run, "repository_id":41, "head_repository_id":41,
                                         "head_sha":identity["commit_sha"]}})
        attempt = {**self.run, "id":run, "head_sha":identity["commit_sha"]}
        job_record = {**self.jobs[0], "id":job, "run_id":run, "head_sha":identity["commit_sha"]}
        if prepared:
            job_record.update({"status":"in_progress", "conclusion":None, "completed_at":None})
        transfer = {**self.transfer(), "transfer_id":f"original-{object_id}", "role":"SelectedEvidenceBundle",
                    "stable_artifact_id":str(object_id), "producer":identity,
                    "provider_artifact_name":metadata["name"], "created_at_utc":metadata["created_at"],
                    "files":[{"path":path, "size_bytes":len(data), "sha256":BASE_FIXTURE.digest(data)}
                             for path, data in files.items()]}
        path = self.directory / f"transfer-{object_id}.json"
        path.write_bytes(BASE_FIXTURE.encode(transfer))
        self.objects[object_id] = {"files":dict(files), "zip":BASE_FIXTURE.archive(files), "metadata":metadata,
                                   "transfer":transfer, "attempt":attempt, "job":job_record}
        return {"artifact_id":object_id, "transfer_path":str(path), "expected_producer":copy.deepcopy(identity), "members":members}

    def refresh_object(self, object_id):
        obj = self.objects[object_id]
        obj["zip"] = BASE_FIXTURE.archive(obj["files"])
        obj["transfer"]["files"] = [{"path":path, "size_bytes":len(data), "sha256":BASE_FIXTURE.digest(data)}
                                    for path, data in obj["files"].items()]
        (self.directory / f"transfer-{object_id}.json").write_bytes(BASE_FIXTURE.encode(obj["transfer"]))

    def route(self, request):
        method, url, headers, body, timeout, limit = request
        parsed = urlsplit(url)
        if method != "GET" or body is not None:
            raise AssertionError("qualifier attempted a mutation")
        if parsed.hostname == "artifacts.example.test":
            object_id = int(parsed.path.strip("/").removesuffix(".zip"))
            if "Authorization" in headers:
                raise AssertionError("credential forwarded to storage")
            return STORE.HttpResponse(200, {}, self.objects[object_id]["zip"])
        path = parsed.path.removeprefix(API_BASE)
        if path in self.routes:
            value = self.routes[path]
        elif path == "":
            value = {"id":41, "full_name":STORE.REPOSITORY}
        elif path.startswith("/git/commits/"):
            value = self.commits[path.rsplit("/", 1)[-1]]
        elif path.startswith("/compare/"):
            value = {"merge_base_commit":{"sha":self.base}}
        elif path == "/pulls/1":
            value = {"number":1, "merged":True, "merge_commit_sha":self.head,
                     "head":{"sha":self.reviewed}, "base":{"ref":"main", "repo":{"id":41}}}
        elif path == "/pulls/1/reviews/606":
            value = self.review
        elif path.startswith("/actions/artifacts/"):
            tail = path.removeprefix("/actions/artifacts/").split("/")
            object_id = int(tail[0])
            if len(tail) == 2 and tail[1] == "zip":
                return STORE.HttpResponse(302, {
                    "Location":f"https://artifacts.example.test/{object_id}.zip?sig=synthetic",
                    "Date":format_datetime(datetime.fromtimestamp(NOW, timezone.utc), usegmt=True),
                }, b"{}")
            value = self.objects[object_id]["metadata"]
        elif path.startswith("/actions/runs/"):
            run = int(path.split("/")[3])
            objects = [item for item in self.objects.values() if item["attempt"]["id"] == run]
            if path.endswith("/jobs"):
                jobs = {item["job"]["id"]:item["job"] for item in objects}
                value = {"total_count":len(jobs), "jobs":list(jobs.values())}
            else:
                value = objects[0]["attempt"]
        else:
            original = super().route(request)
            return STORE.HttpResponse(original.status, {**original.headers, "Date":format_datetime(datetime.fromtimestamp(NOW, timezone.utc), usegmt=True)}, original.body)
        return BASE_FIXTURE.response(200, value, {"Date":format_datetime(datetime.fromtimestamp(NOW, timezone.utc), usegmt=True)})

    def qualifier(self, *, clock=lambda: NOW, **kwargs):
        return DRIVER.Qualifier(self.selection, selection_dir=self.directory,
                                credential=lambda:BASE_FIXTURE.FAKE_CREDENTIAL, transport=self, clock=clock, **kwargs)


class ProtocolTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="qualifier-protocol-")
        self.root = Path(self.directory.name)
        self.world = World(self.root)
        self.bridge_calls = []

    def tearDown(self):
        self.assertEqual(self.world.unexpected, [])
        self.assertTrue(all(call[0] == "GET" for call in self.world.calls))
        self.directory.cleanup()

    def bridge_probe(self, phase, wire, evidence, out):
        self.bridge_calls.append((phase, copy.deepcopy(wire), evidence))
        out.mkdir(parents=True, exist_ok=True)
        # No typed result or eligibility is invented by this protocol probe.
        return {"stage":"prepared" if phase == "prepare" else "qualified-computation", "freeze_state":"Incomplete"}

    def test_real_read_sequence_and_original_numeric_member_mapping(self):
        result = self.world.qualifier().run("prepare", self.root / "out", self.bridge_probe)
        self.assertEqual(result["freeze_state"], "Incomplete")
        self.assertEqual(len(self.bridge_calls), 1)
        _, wire, _ = self.bridge_calls[0]
        self.assertEqual(wire["artifacts"][0]["transfer"], self.world.objects[505]["transfer"])
        self.assertEqual([member["path"] for member in wire["artifacts"][0]["members"]], ["one.json", "two.json"])
        self.assertEqual([member["bytes"] for member in wire["artifacts"][0]["members"]], [list(b"same"), list(b"same")])
        self.assertNotIn("readback_verified", wire["artifacts"][0])
        self.assertEqual(json.loads((self.root / "out/final-freeze.readback-input.json").read_bytes()), wire)
        main_reads = [call for call in self.world.calls if call[1].endswith("/git/ref/heads/main")]
        self.assertGreaterEqual(len(main_reads), 4)
        self.assertNotIn(BASE_FIXTURE.FAKE_CREDENTIAL, json.dumps(result))

    def test_main_review_merge_source_and_date_are_observed_independently(self):
        for defect in ("main", "review_head", "review_actor", "review_body", "parents", "merge_base", "source", "date", "job"):
            with self.subTest(defect=defect):
                self.world = World(self.root)
                if defect == "main": self.world.refs["refs/heads/main"] = "e" * 40
                elif defect == "review_head": self.world.review["commit_id"] = "e" * 40
                elif defect == "review_actor": self.world.review["user"]["id"] = 405
                elif defect == "review_body": self.world.review["body"] += " changed"
                elif defect == "parents": self.world.commits[HEAD]["parents"] = [{"sha":BASE}]
                elif defect == "merge_base": self.world.routes[f"/compare/{BASE}...{REVIEWED}"] = {"merge_base_commit":{"sha":"e" * 40}}
                elif defect == "source": self.world.source["body"] += " changed"
                elif defect == "date":
                    self.world.after = lambda request, result: STORE.HttpResponse(result.status, {}, result.body)
                else: self.world.objects[505]["job"]["status"] = "in_progress"
                with self.assertRaises(STORE.StoreError):
                    self.world.qualifier().run("prepare", self.root / ("out-" + defect), self.bridge_probe)
                self.assertEqual(self.bridge_calls, [])

    def test_reloaded_matching_artifact_never_substitutes_for_second_main_read(self):
        def moves_main(phase, wire, evidence, out):
            result = self.bridge_probe(phase, wire, evidence, out)
            self.world.refs["refs/heads/main"] = "e" * 40
            return result
        with self.assertRaises(STORE.StoreError):
            self.world.qualifier().run("prepare", self.root / "out", moves_main)
        self.assertFalse((self.root / "out/final-freeze.provider-observation.json").exists())

    def test_factual_flags_and_omitted_mapping_cannot_be_supplied_as_selection(self):
        for field, value in (("Current", True), ("readback_verified", True), ("replay_feasible", True)):
            with self.subTest(field=field):
                self.world.selection[field] = value
                with self.assertRaises(STORE.StoreError): self.world.qualifier()
                del self.world.selection[field]
        self.world.selection["artifacts"][0]["members"].pop()
        with self.assertRaises(STORE.StoreError):
            self.world.qualifier().run("prepare", self.root / "out", self.bridge_probe)
        self.assertEqual(self.bridge_calls, [])

    def test_no_ambient_credential_or_transport_error_is_reported(self):
        def unavailable(_request):
            raise OSError(BASE_FIXTURE.FAKE_CREDENTIAL)
        self.world.before = unavailable
        with mock.patch.dict(os.environ, {"GH_TOKEN":"ambient-must-not-be-used"}), self.assertRaises(STORE.StoreError) as failure:
            self.world.qualifier().run("prepare", self.root / "out", self.bridge_probe)
        self.assertNotIn(BASE_FIXTURE.FAKE_CREDENTIAL, str(failure.exception))
        self.assertNotIn("ambient-must-not-be-used", str(failure.exception))

    def test_second_member_corruption_and_expiry_are_not_masked(self):
        for defect in ("bytes", "expired", "alias", "producer", "oversize"):
            with self.subTest(defect=defect):
                self.world = World(self.root)
                if defect == "bytes": self.world.objects[505]["zip"] = BASE_FIXTURE.archive({"one.json":b"same", "two.json":b"evil"})
                elif defect == "expired": self.world.objects[505]["metadata"]["expires_at"] = BASE_FIXTURE.date(NOW)
                elif defect == "alias": self.world.selection["artifacts"][0]["members"][1]["path"] = "one.json"
                elif defect == "producer": self.world.selection["artifacts"][0]["expected_producer"]["run_attempt"] = 2
                else:
                    self.world.objects[505]["files"]["two.json"] = b"x" * (STORE.Limits().file_bytes + 1)
                    self.world.refresh_object(505)
                with self.assertRaises(STORE.StoreError):
                    self.world.qualifier().run("prepare", self.root / ("out-" + defect), self.bridge_probe)
                self.assertEqual(self.bridge_calls, [])

    def test_prior_success_cannot_survive_a_reused_output_directory(self):
        out = self.root / "out"
        self.world.qualifier().run("prepare", out, self.bridge_probe)
        before = (out / "final-freeze.provider-observation.json").read_bytes()
        calls = len(self.world.calls)
        with self.assertRaises(STORE.StoreError):
            self.world.qualifier().run("prepare", out, self.bridge_probe)
        self.assertEqual(len(self.world.calls), calls)
        self.assertEqual((out / "final-freeze.provider-observation.json").read_bytes(), before)

    def test_each_authenticated_nested_read_requires_fresh_uncached_provider_time(self):
        targets = (
            ("main", lambda url: url.endswith("/git/ref/heads/main"), 1),
            ("repository", lambda url: url == STORE.API + API_BASE, 1),
            ("source", lambda url: "/issues/comments/" in url, 1),
            ("metadata-before", lambda url: url.endswith("/actions/artifacts/505"), 1),
            ("metadata-after", lambda url: url.endswith("/actions/artifacts/505"), 2),
            ("attempt", lambda url: url.endswith("/actions/runs/101/attempts/1"), 1),
            ("job-before", lambda url: "/actions/runs/101/attempts/1/jobs?" in url, 1),
            ("job-completed", lambda url: "/actions/runs/101/attempts/1/jobs?" in url, 2),
            ("download-redirect", lambda url: url.endswith("/actions/artifacts/505/zip"), 1),
        )
        for label, matches, occurrence in targets:
            for defect in ("valid", "missing", "malformed", "stale", "future", "cached"):
                with self.subTest(route=label, defect=defect):
                    self.world = World(self.root)
                    self.bridge_calls = []
                    seen = []
                    injected = []

                    def change_selected_response(request, response):
                        if not matches(request[1]):
                            return response
                        seen.append(request[1])
                        if len(seen) != occurrence:
                            return response
                        injected.append(True)
                        if defect == "valid":
                            return response
                        headers = {key:value for key, value in response.headers.items()
                                   if key.lower() not in {"date", "age"}}
                        if defect == "malformed":
                            headers["Date"] = "not a provider date"
                        elif defect == "stale":
                            headers["Date"] = "Thu, 01 Jan 1970 00:00:00 GMT"
                        elif defect == "future":
                            headers["Date"] = format_datetime(datetime.fromtimestamp(NOW + 10, timezone.utc), usegmt=True)
                        elif defect == "cached":
                            headers["Date"] = format_datetime(datetime.fromtimestamp(NOW, timezone.utc), usegmt=True)
                            headers["Age"] = "86400"
                        return STORE.HttpResponse(response.status, headers, response.body)

                    self.world.after = change_selected_response
                    out = self.root / (label + "-" + defect)
                    if defect == "valid":
                        result = self.world.qualifier().run("prepare", out, self.bridge_probe)
                        self.assertEqual(result["freeze_state"], "Incomplete")
                        self.assertEqual(len(self.bridge_calls), 1)
                        self.assertTrue((out / "final-freeze.provider-observation.json").is_file())
                    else:
                        with self.assertRaises(STORE.StoreError) as failure:
                            self.world.qualifier().run("prepare", out, self.bridge_probe)
                        self.assertEqual(failure.exception.kind, "instrument_failure" if defect in {"missing", "malformed"} else "provider_unavailable")
                        self.assertEqual(self.bridge_calls, [])
                        self.assertFalse((out / "final-freeze.provider-observation.json").exists())
                    self.assertEqual(injected, [True])
                    self.assertEqual(self.world.unexpected, [])
                    self.assertTrue(all(call[0] == "GET" for call in self.world.calls))

    def test_computation_and_final_observations_reject_clock_rollback_and_elapsed_time(self):
        for defect in ("forward", "wall-rollback", "monotonic-elapsed", "monotonic-rollback", "window-ended",
                       "wall-unavailable", "monotonic-unavailable"):
            with self.subTest(defect=defect):
                self.world = World(self.root)
                self.bridge_calls = []
                state = {"wall":NOW, "tick":10.0, "main_reads":0}

                def advance_before_computation(request):
                    if request[1].endswith("/git/ref/heads/main"):
                        state["main_reads"] += 1
                        if state["main_reads"] == 3:
                            state["wall"] = NOW + 100

                def after_computation(phase, wire, evidence, out):
                    result = self.bridge_probe(phase, wire, evidence, out)
                    self.assertEqual(wire["observed_at_utc"], BASE_FIXTURE.date(NOW + 100))
                    if defect == "forward": state["wall"] = NOW + 101
                    elif defect == "wall-rollback": state["wall"] = NOW + 5
                    elif defect == "monotonic-elapsed": state["tick"] = 611.0
                    elif defect == "monotonic-rollback": state["tick"] = 9.0
                    elif defect == "window-ended": state["wall"] = NOW + 300
                    elif defect == "wall-unavailable": state["wall"] = float("nan")
                    else: state["tick"] = float("nan")
                    return result

                self.world.before = advance_before_computation
                qualifier = self.world.qualifier(clock=lambda:state["wall"], monotonic=lambda:state["tick"])
                out = self.root / defect
                if defect == "forward":
                    result = qualifier.run("prepare", out, after_computation)
                    self.assertEqual(result["provider_observed_at_utc"], BASE_FIXTURE.date(NOW + 101))
                else:
                    with self.assertRaises(STORE.StoreError):
                        qualifier.run("prepare", out, after_computation)
                    self.assertFalse((out / "final-freeze.provider-observation.json").exists())
                self.assertEqual(len(self.bridge_calls), 1)
                self.assertEqual(self.world.unexpected, [])
                self.assertTrue(all(call[0] == "GET" for call in self.world.calls))

    def test_qualifier_provider_rejects_mutating_methods_before_transport(self):
        qualifier = self.world.qualifier()
        for method in ("POST", "PATCH", "DELETE"):
            with self.subTest(method=method), self.assertRaises(STORE.StoreError):
                qualifier.client._api(method, "/git/refs", {"ref":"refs/heads/forbidden"})
        self.assertEqual(self.world.calls, [])


def git(root, *args, input=None):
    clean = {key:value for key, value in os.environ.items() if key not in {
        "GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES"}}
    result = subprocess.run(["git", "-C", str(root), *args], input=input, capture_output=True, env=clean, timeout=20, check=True)
    return result.stdout.decode().strip()


def rehearsal(subject):
    producer = load("qualifier_rehearsal_producer", ROOT / "scripts/release-rehearsal.py")
    def identity(receipt, **_kwargs):
        receipt["release_identity"] = {"schema":"cargo-allow.release-identity.v1", "version":"0.2.0", "tag":"v0.2.0",
            "tag_source":"derived_from_version", "channel":"stable", "rc_ordinal":None, "github_prerelease":False}
        return producer.PHASE_COMPLETE
    with contextlib.ExitStack() as stack:
        stack.enter_context(mock.patch.dict(os.environ, {}, clear=True))
        stack.enter_context(mock.patch.object(producer, "resolve_commit", return_value=subject["commit"]))
        stack.enter_context(mock.patch.object(producer, "require_clean_checkout"))
        stack.enter_context(mock.patch.object(producer, "compute_sha256", side_effect=[subject["lock"], subject["topology"]]))
        stack.enter_context(mock.patch.object(producer, "run_phase_release_identity", side_effect=identity))
        for name in ("candidate_package_set", "shared_prerequisites", "publisher_state_machine", "docs_and_support", "manifest_and_assets", "workflow_graph_permissions"):
            stack.enter_context(mock.patch.object(producer, "run_phase_" + name, return_value=producer.PHASE_COMPLETE))
        return producer.build_rehearsal_receipt("HEAD")


PRODUCTS = ["allow-core", "allow-policy", "allow-policy-legacy", "allow-inventory", "allow-files", "allow-rust",
            "allow-match", "allow-report", "allow-diff", "cargo-allow"]


def source_files(root, commit, tree):
    digest = lambda path: BASE_FIXTURE.digest((root / path).read_bytes())
    subject = {"commit":commit, "tree":tree, "lock":digest("Cargo.lock"), "topology":digest("policy/product-package-topology-v2.toml")}
    files = {f"packages/{name}-0.2.0.crate":f"synthetic exact archive {name}".encode() for name in PRODUCTS}
    rows = [{"name":name, "version":"0.2.0", "crate_file":f"{name}-0.2.0.crate",
             "size_bytes":len(files[f"packages/{name}-0.2.0.crate"]), "sha256":BASE_FIXTURE.digest(files[f"packages/{name}-0.2.0.crate"])} for name in PRODUCTS]
    topology = tomllib.loads((root / "policy/product-package-topology-v2.toml").read_text())
    rows.extend({"name":row["cargo_package_name"], "version":row["package_version"], "crate_file":"shared",
                 "size_bytes":0, "sha256":row["expected_registry_checksum"]}
                for row in topology["package"] if row.get("product_family") == "shared" and row.get("candidate_inclusion"))
    documents = {
        "candidate-preparation":{"readiness":"stale", "reasons":["no transition to prepare"], "input_identity":{"head_commit":commit}},
        "package-set":{"schema_id":"cargo-allow.exact-candidate-package-set.v1", "result":"Passed",
                       "candidate":{"workspace_version":"0.2.0"}, "package_set":{"crates":rows}},
        "package-docs":{"basis":{"commit":commit, "tree":tree, "cargo_lock_sha256":subject["lock"],
                                    "topology_sha256":subject["topology"], "release_identity":{"version":"0.2.0"}}},
        "rehearsal":rehearsal(subject),
        "install-journey":{"candidate":{"version":"0.2.0"}, "result":"Passed"},
        "upgrade-rollback":{"candidate":{"version":"0.2.0"}, "result":"Passed"},
        "controls":{"state":"Feasible", "commit":commit, "tree":tree},
    }
    members = [{"logical_id":name, "role":"PackageArchive", "path":f"packages/{name}-0.2.0.crate"} for name in PRODUCTS]
    for role, document in documents.items():
        path = role + ".json"
        files[path] = BASE_FIXTURE.encode(document)
        members.append({"logical_id":"evidence:" + role, "role":"Evidence:" + role, "path":path})
    files["release-manifest-v2.json"] = b'{"version":"0.2.0","fixture":"exact bytes only; no manifest semantic claim"}'
    members.append({"logical_id":"release-manifest-v2", "role":"ReleaseManifest", "path":"release-manifest-v2.json"})
    return files, members


def compose_diagnostic(binary, root, files, members, out):
    evidence = root / "target/local-evidence"
    for path, data in files.items():
        target = evidence / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    argv = [str(binary), "release-freeze", "compose", "--out-dir", str(out)]
    for member in members:
        role = member["role"]
        if role.startswith("Evidence:"):
            argv.extend(["--evidence", role.removeprefix("Evidence:") + "=" + str(evidence / member["path"])])
        elif role == "ReleaseManifest":
            argv.extend(["--evidence", "release-manifest=" + str(evidence / member["path"])])
    clean = {key:value for key, value in os.environ.items() if key not in {
        "GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "CARGO_ALLOW_ROOT", "CARGO_ALLOW_CONFIG", "GH_TOKEN", "GITHUB_TOKEN", "CARGO_REGISTRY_TOKEN"}}
    result = subprocess.run(argv, cwd=root, env=clean, capture_output=True, timeout=60, check=False)
    if result.returncode == 0 or not (out / "final-freeze.replay-inputs.json").is_file():
        raise AssertionError("actual unqualified composer failed outside its retained Incomplete verdict: " + result.stderr.decode(errors="replace"))
    readiness = json.loads((out / "final-freeze.readiness.json").read_bytes())
    rows = json.loads((out / "final-freeze.readiness-rows.json").read_bytes())
    if readiness is not None or {row["evidence_id"] for row in rows} != {
        "post-merge-qualification", "custody-readback", "evaluation-clock", "authorization-window"}:
        raise AssertionError("unobserved facts acquired full Boolean readiness")
    custody = json.loads((out / "final-freeze.custody.json").read_bytes())
    if any(item["readback_verified"] or item["retention_expiry_utc"] or item["storage_locator"] for item in custody["items"]):
        raise AssertionError("local diagnostic bytes acquired provider custody")
    if custody["claim_boundary"] != ["local_diagnostic_bytes_only", "provider_retention_not_established",
                                     "independent_readback_not_verified", "release_authorization_not_granted"]:
        raise AssertionError("local diagnostic claims acquired provider custody or verified readback")
    return json.loads((out / "final-freeze.evidence-graph.json").read_bytes()), json.loads((out / "final-freeze.receipt.json").read_bytes())


def native_controls(binary):
    with tempfile.TemporaryDirectory(prefix="qualifier-native-") as directory:
        directory = Path(directory)
        root = directory / "repo"
        root.mkdir()
        git(root, "init", "-b", "main")
        git(root, "config", "user.email", "fixture@example.invalid")
        git(root, "config", "user.name", "qualifier fixture")
        (root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.2.0"\n')
        (root / "Cargo.lock").write_bytes(b"synthetic committed lock\n")
        (root / ".gitignore").write_text("target/\n")
        for path in ("policy/product-package-topology-v2.toml", "docs/support-matrix.toml", "docs/release/evidence/rc1-publication-incident.v1.json"):
            target = root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((ROOT / path).read_bytes())
        git(root, "add", "--all")
        git(root, "commit", "-m", "base")
        base = git(root, "rev-parse", "HEAD")
        git(root, "commit", "--allow-empty", "-m", "reviewed subject")
        reviewed = git(root, "rev-parse", "HEAD")
        tree = git(root, "rev-parse", "HEAD^{tree}")
        files, members = source_files(root, reviewed, tree)
        graph, receipt = compose_diagnostic(binary, root, files, members, root / "target/diagnostic")
        head = git(root, "commit-tree", tree, "-p", base, "-p", reviewed, input=b"synthetic merge\n")
        git(root, "update-ref", "refs/heads/main", head)
        world = World(directory, head=head, tree=tree, base=base, reviewed=reviewed, graph=graph, graph_digest=receipt["recorded_graph_digest"])
        files, members = source_files(root, head, tree)
        # Two original objects with distinct producers; package/docs receipt
        # and archives keep their actual paths and jobs across retention.
        docs = {"package-docs.json":files.pop("package-docs.json")}
        docs_members = [member for member in members if member["logical_id"] == "evidence:package-docs"]
        members = [member for member in members if member["logical_id"] != "evidence:package-docs"]
        world.selection["artifacts"] = [world.add_object(505, files, members), world.add_object(506, docs, docs_members, run=102, job=304)]
        bridge = DRIVER.native_bridge(binary, root)
        prepared = directory / "prepared"
        world.qualifier().run("prepare", prepared, bridge)
        receipt_bytes = (prepared / "final-freeze.receipt.json").read_bytes()
        graph_bytes = (prepared / "final-freeze.evidence-graph.json").read_bytes()
        prepared_selection = world.add_object(507, {"final-freeze.receipt.json":receipt_bytes, "final-freeze.evidence-graph.json":graph_bytes},
            [{"logical_id":"final-freeze-receipt", "role":"FreezeReceipt", "path":"final-freeze.receipt.json"},
             {"logical_id":"final-freeze-evidence-graph", "role":"EvidenceGraph", "path":"final-freeze.evidence-graph.json"}], run=103, job=305, prepared=True)
        world.selection["artifacts"].append(prepared_selection)
        qualified = directory / "qualified"
        result = world.qualifier().run("qualify", qualified, bridge)
        readiness = json.loads((qualified / "final-freeze.readiness.json").read_bytes())
        replay_inputs = json.loads((qualified / "final-freeze.replay-inputs.json").read_bytes())
        if result["post_merge_qualification"] != "EquivalentTree" or result["custody_disposition"] != "Complete" \
                or result.get("persisted_replay_verified") is not True \
                or readiness["post_merge_qualification"] != "current" or not readiness["custody_replay_feasible"] \
                or readiness["custody_expires_before_authorization_window"]:
            raise AssertionError("the actual positive qualification/custody boundary did not pass independently of remaining holds")
        if result["freeze_state"] != "Incomplete" or not any(row.get("evidence_id") == "registry-observation" for row in readiness["rows"]):
            raise AssertionError("separate real registry/rehearsal holds were erased")
        if (qualified / "final-freeze.receipt.json").read_bytes() != receipt_bytes:
            raise AssertionError("prepared receipt bytes were rewritten")
        if replay_inputs["retained_transfers"] != [world.objects[number]["transfer"] for number in (505, 506, 507)]:
            raise AssertionError("numeric original envelopes were relabeled")
        control_count = 0
        for defect in ("window", "rerun", "missing_qualification_node", "producer", "second_member", "pretty_receipt", "missing_clock", "forged_flag"):
            selection_before = copy.deepcopy(world.selection)
            objects_before = copy.deepcopy(world.objects)
            record_before = copy.deepcopy(world.record)
            if defect == "window":
                world.objects[506]["metadata"]["expires_at"] = BASE_FIXTURE.date(NOW + 100)
            elif defect == "rerun": world.record["required_rerun_set"] = ["release-rehearsal"]
            elif defect == "missing_qualification_node": world.record["preserved_evidence_nodes"].pop()
            elif defect == "producer":
                for target in (world.objects[506]["transfer"]["producer"], world.selection["artifacts"][1]["expected_producer"]):
                    target["producer_generation"] = 2
                world.refresh_object(506)
            elif defect == "second_member":
                world.selection["artifacts"][0]["members"][1]["logical_id"] = "different-logical-package"
            elif defect == "pretty_receipt":
                world.objects[507]["files"]["final-freeze.receipt.json"] = json.dumps(json.loads(receipt_bytes), indent=2).encode()
                world.refresh_object(507)
            world.sync_source()
            def changed_bridge(phase, wire, evidence, out):
                if defect == "missing_clock": wire["observed_at_utc"] = ""
                if defect == "forged_flag": wire["readback_verified"] = True
                return bridge(phase, wire, evidence, out)
            try:
                output = world.qualifier().run("qualify", directory / ("negative-" + defect), changed_bridge)
            except STORE.StoreError:
                if defect == "window": raise AssertionError("expiry control did not reach its actual typed custody-expiring row")
            else:
                if defect != "window" or not output["readiness"]["custody_expires_before_authorization_window"] \
                        or not any(row["kind"] == "custody_expiring" for row in output["readiness"]["rows"]):
                    raise AssertionError("actual consumer admitted unproven control: " + defect)
            control_count += 1
            world.selection, world.objects, world.record = selection_before, objects_before, record_before
            for number in world.objects: world.refresh_object(number)
            world.sync_source()
        if world.unexpected or any(call[0] != "GET" for call in world.calls):
            raise AssertionError("native qualifier made an unexpected or mutating provider request")
        print(f"native qualifier: actual prepare/readback/typed-compose/serialized-replay positive; {control_count} discriminating controls; zero provider mutations")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--cargo-allow", type=Path)
    options, remaining = parser.parse_known_args()
    if options.cargo_allow is not None:
        if remaining: raise SystemExit("native fixture accepts only --cargo-allow")
        native_controls(options.cargo_allow.resolve(strict=True))
    else:
        unittest.main(argv=[sys.argv[0], *remaining])
