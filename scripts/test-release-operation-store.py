#!/usr/bin/env python3
"""Run the production adapter against intercepted provider exchanges only."""

from __future__ import annotations

import argparse
import base64
from concurrent.futures import ThreadPoolExecutor
import copy
from datetime import datetime, timezone
from email.message import Message
import hashlib
import importlib.util
import io
import json
import multiprocessing
import os
from pathlib import Path
import pickle
import stat
import subprocess
import sys
import tempfile
import threading
import unittest
from unittest import mock
from urllib.parse import urlsplit
import zipfile


sys.dont_write_bytecode = True
MODULE_PATH = Path(__file__).with_name("release_operation_store.py")
SPEC = importlib.util.spec_from_file_location("release_operation_store", MODULE_PATH)
if SPEC is None or SPEC.loader is None:
    raise SystemExit("could not load production release operation store")
STORE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = STORE
SPEC.loader.exec_module(STORE)

REPOSITORY = "EffortlessMetrics/cargo-allow"
BASE = "/repos/" + REPOSITORY
ANCHOR = "a" * 40
ANCHOR_TREE = "b" * 40
SUBJECT = "sha256:" + "1" * 64
OPERATION = "sha256:" + "2" * 64
PREFIX = "refs/heads/cargo-allow-release-control/"
REF = PREFIX + "1" * 64
NOW = 1_786_000_200
FAKE_CREDENTIAL = "synthetic-credential-never-read-from-environment"


def encode(value):
    return json.dumps(value, separators=(",", ":"), sort_keys=True).encode()


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def object_id(kind, data):
    return hashlib.sha1(kind.encode() + b" " + str(len(data)).encode() + b"\0" + data,
                        usedforsecurity=False).hexdigest()


def date(seconds):
    return datetime.fromtimestamp(seconds, timezone.utc).isoformat().replace("+00:00", "Z")


def response(status, value, headers=None):
    return STORE.HttpResponse(status, headers or {}, encode(value))


def producer():
    return {
        "repository": REPOSITORY, "workflow_path": ".github/workflows/release.yml",
        "git_ref": "refs/heads/main", "run_id": 101, "run_attempt": 1,
        "job_id": "303", "commit_sha": ANCHOR, "tree_sha": ANCHOR_TREE,
        "release_version": "0.2.0", "tool_name": "cargo-allow",
        "schema_id": "cargo-allow.release-operation-identity.v1",
        "producer_generation": 1,
    }


def archive(files, *, mode=None, duplicate=None):
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as result:
        for name, data in files.items():
            entry = zipfile.ZipInfo(name)
            entry.create_system = 3
            entry.external_attr = ((stat.S_IFREG | 0o600) if mode is None else mode) << 16
            result.writestr(entry, data)
        if duplicate is not None:
            with mock.patch("warnings.warn"):
                result.writestr(duplicate, b"duplicate")
    return output.getvalue()


class Provider:
    """GitHub protocol simulator, not an authorization/eligibility evaluator."""

    instances = []

    def __init__(self):
        self.instances.append(self)
        self.unexpected = []
        self.lock = threading.RLock()
        self.calls = []
        self.refs = {}
        self.blobs = {}
        self.trees = {}
        self.commits = {}
        self.before = None
        self.after = None
        self.source_body = b"bounded exact synthetic maintainer source"
        self.source = {
            "id": 202, "issue_url": "https://api.github.com" + BASE + "/issues/3760",
            "user": {"id": 404, "login": "release-operator"},
            "body": self.source_body.decode(),
        }
        self.artifact_files = {"authorization.json": b'{"synthetic":true}\n'}
        self.artifact_zip = archive(self.artifact_files)
        self.artifact = {
            "id": 505, "name": "selected-release-authorization", "expired": False,
            "created_at": date(NOW - 10), "expires_at": date(NOW + 1000),
            "digest": "sha256:" + "8" * 64,
            "workflow_run": {"id": 101, "repository_id": 41, "head_repository_id": 41,
                             "head_sha": ANCHOR},
        }
        self.run = {
            "id": 101, "run_attempt": 1, "head_sha": ANCHOR,
            "repository": {"id": 41}, "head_repository": {"id": 41},
            "path": ".github/workflows/release.yml", "event": "workflow_dispatch",
            "head_branch": "main",
        }
        self.jobs = [{
            "id": 303, "run_id": 101, "head_sha": ANCHOR, "status": "completed",
            "conclusion": "success", "started_at": date(NOW - 20),
            "completed_at": date(NOW - 5),
        }]
        self.download_url = "https://artifacts.example.test/bundle.zip?sig=synthetic-signed-url"

    def client(self, **kwargs):
        return STORE.GitHubReleaseStore(
            repository_id=41, anchor_commit=ANCHOR, anchor_tree=ANCHOR_TREE,
            control_prefix=PREFIX, credential=lambda: FAKE_CREDENTIAL,
            download_hosts=frozenset({"artifacts.example.test"}),
            transport=self, clock=lambda: NOW, monotonic=lambda: 100.0, **kwargs)

    def transfer(self):
        return {
            "schema_id": "cargo-allow.release-artifact-transfer.v1", "schema_version": 1,
            "transfer_id": "selected-transfer-505", "role": "release-authorization",
            "stable_artifact_id": "505", "producer": producer(),
            "provider_id": "github-actions-artifact",
            "provider_artifact_name": self.artifact["name"],
            "files": [{"path": path, "size_bytes": len(data), "sha256": digest(data)}
                      for path, data in self.artifact_files.items()],
            "semantic_payload_digest": OPERATION, "trust_class": "ManualDispatch",
            "untrusted_input_posture": "StrictByteMatch", "created_at_utc": date(NOW - 10),
            "claim_boundary": ["exact_producer_identity"], "limitations": [],
        }

    def source_input(self):
        return {"kind": "issue_comment", "repository": REPOSITORY,
                "reference": "issue:3760#comment:202", "author": "release-operator",
                "body_digest": digest(self.source_body),
                "statement": "Authorize publish_cargo_allow_final_0_2_0 for v0.2.0."}

    def ref_record(self, ref):
        return {"ref": ref, "object": {"type": "commit", "sha": self.refs[ref]}}

    def is_ancestor(self, old, new):
        while new != old:
            record = self.commits.get(new)
            if record is None or len(record["parents"]) != 1:
                return False
            new = record["parents"][0]["sha"]
        return True

    def __call__(self, method, url, headers, body, timeout, limit):
        request = (method, url, dict(headers), body, timeout, limit)
        with self.lock:
            self.calls.append(request)
        if self.before is not None:
            override = self.before(request)
            if override is not None:
                return override
        with self.lock:
            try:
                result = self.route(request)
            except Exception:
                self.unexpected.append((method, urlsplit(url).path))
                raise
        if self.after is not None:
            replacement = self.after(request, result)
            if replacement is not None:
                result = replacement
        return result

    def route(self, request):
        method, url, headers, raw, _, _ = request
        parsed = urlsplit(url)
        if parsed.hostname == "artifacts.example.test":
            if method != "GET" or url != self.download_url or "Authorization" in headers:
                raise AssertionError("unexpected signed-download exchange")
            return STORE.HttpResponse(200, {}, self.artifact_zip)
        if parsed.hostname != "api.github.com" or headers.get("Authorization") != "Bearer " + FAKE_CREDENTIAL:
            raise AssertionError("unexpected authenticated destination")
        path = parsed.path
        body = None if raw is None else json.loads(raw)
        if method == "GET" and path == BASE:
            return response(200, {"id": 41, "full_name": REPOSITORY})
        if method == "GET" and path == BASE + "/git/commits/" + ANCHOR:
            return response(200, {"sha": ANCHOR, "tree": {"sha": ANCHOR_TREE}})
        if method == "GET" and path == BASE + "/issues/comments/202":
            return response(200, self.source)
        if method == "GET" and path == BASE + "/actions/artifacts/505":
            return response(200, self.artifact)
        if method == "GET" and path == BASE + "/actions/runs/101/attempts/1":
            return response(200, self.run)
        if method == "GET" and path == BASE + "/actions/runs/101/attempts/1/jobs":
            if parsed.query != "per_page=100&page=1":
                raise AssertionError("unexpected job pagination")
            return response(200, {"total_count": len(self.jobs), "jobs": self.jobs})
        if method == "GET" and path == BASE + "/actions/artifacts/505/zip":
            return response(302, {}, {"Location": self.download_url})
        if method == "GET" and path.startswith(BASE + "/git/ref/"):
            ref = "refs/" + path.removeprefix(BASE + "/git/ref/")
            return response(200, self.ref_record(ref)) if ref in self.refs else response(404, {})
        if method == "POST" and path == BASE + "/git/blobs":
            if set(body) != {"content", "encoding"} or body["encoding"] != "base64":
                raise AssertionError("unexpected blob creation request")
            data = base64.b64decode(body["content"], validate=True)
            sha = object_id("blob", data)
            self.blobs[sha] = data
            return response(201, {"sha": sha})
        if method == "GET" and path.startswith(BASE + "/git/blobs/"):
            sha = path.rsplit("/", 1)[-1]
            data = self.blobs[sha]
            return response(200, {"sha": sha, "size": len(data), "encoding": "base64",
                                  "content": base64.b64encode(data).decode() + "\n"})
        if method == "POST" and path == BASE + "/git/trees":
            if set(body) != {"tree"}:
                raise AssertionError("unexpected tree creation request")
            entries = []
            wire = b""
            for item in sorted(body["tree"], key=lambda item: item["path"]):
                if item["mode"] != "100644" or item["type"] != "blob":
                    raise AssertionError("unexpected store mode")
                wire += b"100644 " + item["path"].encode() + b"\0" + bytes.fromhex(item["sha"])
                entries.append({**item, "size": len(self.blobs[item["sha"]])})
            sha = object_id("tree", wire)
            self.trees[sha] = entries
            return response(201, {"sha": sha})
        if method == "GET" and path.startswith(BASE + "/git/trees/"):
            sha = path.rsplit("/", 1)[-1]
            return response(200, {"sha": sha, "truncated": False, "tree": self.trees[sha]})
        if method == "POST" and path == BASE + "/git/commits":
            if set(body) != {"message", "tree", "parents", "author", "committer"}:
                raise AssertionError("unexpected commit creation request")
            wire = "tree " + body["tree"] + "\n"
            wire += "".join("parent " + parent + "\n" for parent in body["parents"])
            for role in ("author", "committer"):
                actor = body[role]
                timestamp = int(datetime.fromisoformat(actor["date"].replace("Z", "+00:00")).timestamp())
                wire += f'{role} {actor["name"]} <{actor["email"]}> {timestamp} +0000\n'
            wire += "\n" + body["message"]
            sha = object_id("commit", wire.encode())
            self.commits[sha] = {
                "sha": sha, "tree": {"sha": body["tree"]},
                "parents": [{"sha": parent} for parent in body["parents"]],
                "author": body["author"], "committer": body["committer"],
                "message": body["message"],
            }
            return response(201, {"sha": sha})
        if method == "GET" and path.startswith(BASE + "/git/commits/"):
            return response(200, self.commits[path.rsplit("/", 1)[-1]])
        if method == "POST" and path == BASE + "/git/refs":
            if set(body) != {"ref", "sha"} or not body["ref"].startswith(PREFIX):
                raise AssertionError("unexpected ref creation")
            if body["ref"] in self.refs:
                return response(422, {"message": "already exists"})
            self.refs[body["ref"]] = body["sha"]
            return response(201, self.ref_record(body["ref"]))
        if method == "PATCH" and path.startswith(BASE + "/git/refs/"):
            ref = "refs/" + path.removeprefix(BASE + "/git/refs/")
            if not ref.startswith(PREFIX) or set(body) != {"sha", "force"} or body["force"] is not False:
                raise AssertionError("forced or unexpected ref update")
            if ref not in self.refs or not self.is_ancestor(self.refs[ref], body["sha"]):
                return response(422, {"message": "not a fast forward"})
            self.refs[ref] = body["sha"]
            return response(200, self.ref_record(ref))
        raise AssertionError("unexpected intercepted provider exchange")

    def mutations(self):
        return [(method, url, body) for method, url, _, body, _, _ in self.calls
                if method in ("POST", "PATCH", "DELETE") and "/git/refs" in url]


class StoreTests(unittest.TestCase):
    def setUp(self):
        self.providers_from = len(Provider.instances)
        self.provider = Provider()
        self.store = self.provider.client()
        self.initial = self.store.read(SUBJECT)
        self.files = {"operation.json": b'{"existing_domain_record":"initial"}\n'}

    def tearDown(self):
        for provider in Provider.instances[self.providers_from:]:
            self.assertEqual(provider.unexpected, [], "unexpected intercepted request")

    def attempt(self, observed=None, files=None, store=None):
        client = self.store if store is None else store
        return client.prepare_append(
            self.initial if observed is None else observed,
            self.files if files is None else files, operation_digest=OPERATION,
            producer_bytes=encode(producer()), request_boundary="tag-push-intent",
            valid_until=NOW + 60)

    def initial_commit(self):
        return self.attempt().append().consume(lambda snapshot: snapshot)

    def test_real_create_append_and_independent_object_readback(self):
        first = self.initial_commit()
        second_files = {**self.files, "lease.json": b'{"state":"synthetic-held"}\n'}
        second = self.attempt(first, second_files).append().consume(lambda snapshot: snapshot)
        self.assertEqual(second.parent, first.commit)
        self.assertEqual(dict(second.files), second_files)
        self.assertNotEqual(first.commit, second.commit)
        self.assertEqual([item[0] for item in self.provider.mutations()], ["POST", "PATCH"])
        patch = json.loads(self.provider.mutations()[-1][2])
        self.assertIs(patch["force"], False)
        tail = [urlsplit(call[1]).path for call in self.provider.calls[-7:]]
        self.assertTrue(any("/git/blobs/" in path for path in tail))
        self.assertIn("/git/ref/", tail[-1])

    def test_attempt_and_witness_are_consumed_once_even_after_callback_failure(self):
        attempt = self.attempt()
        witness = attempt.append()
        with self.assertRaises(STORE.StoreError):
            attempt.append()
        called = []
        def fails(snapshot):
            called.append(snapshot.commit)
            raise RuntimeError("synthetic callback failure")
        with self.assertRaises(RuntimeError):
            witness.consume(fails)
        with self.assertRaises(STORE.StoreError):
            witness.consume(lambda _: called.append("duplicate"))
        self.assertEqual(len(called), 1)
        self.assertEqual(len(self.provider.mutations()), 1)

    def test_duplicate_threads_get_one_callback(self):
        witness = self.attempt().append()
        barrier = threading.Barrier(2)
        called = []
        def invoke(_):
            barrier.wait(timeout=3)
            try:
                return witness.consume(lambda snapshot: called.append(snapshot.commit))
            except STORE.StoreError:
                return "refused"
        with ThreadPoolExecutor(max_workers=2) as executor:
            results = list(executor.map(invoke, range(2)))
        self.assertEqual(len(called), 1)
        self.assertEqual(results.count("refused"), 1)

    def test_same_job_contenders_have_distinct_proposals_and_only_one_witness(self):
        parent = self.initial_commit()
        other = self.provider.client()
        other_parent = other.read(SUBJECT)
        files = {**self.files, "lease.json": b'{"state":"synthetic-started"}'}
        left = self.attempt(parent, files)
        right = self.attempt(other_parent, files, other)
        self.assertNotEqual(left._message, right._message)
        barrier = threading.Barrier(2)
        def wait_before_patch(request):
            if request[0] == "PATCH":
                barrier.wait(timeout=3)
        self.provider.before = wait_before_patch
        def append(attempt):
            try:
                return attempt.append().consume(lambda snapshot: snapshot.commit)
            except STORE.StoreError:
                return None
        with ThreadPoolExecutor(max_workers=2) as executor:
            winners = list(executor.map(append, (left, right)))
        self.assertEqual(sum(item is not None for item in winners), 1)
        proposals = [json.loads(body)["sha"] for method, _, body in self.provider.mutations()
                     if method == "PATCH"]
        self.assertEqual(len(set(proposals)), 2)

    def test_create_contenders_have_only_one_witness(self):
        other = self.provider.client()
        contender = self.attempt(other.read(SUBJECT), store=other)
        barrier = threading.Barrier(2)
        def wait_before_create(request):
            if request[0] == "POST" and urlsplit(request[1]).path == BASE + "/git/refs":
                barrier.wait(timeout=3)
        self.provider.before = wait_before_create
        def append(attempt):
            try:
                return attempt.append().consume(lambda snapshot: snapshot.commit)
            except STORE.StoreError:
                return None
        with ThreadPoolExecutor(max_workers=2) as executor:
            winners = list(executor.map(append, (self.attempt(), contender)))
        self.assertEqual(sum(item is not None for item in winners), 1)

    def test_unchanged_payload_stale_parent_and_foreign_snapshot_cannot_append(self):
        parent = self.initial_commit()
        with self.assertRaises(STORE.StoreError):
            self.attempt(parent, self.files)
        with self.assertRaises(STORE.StoreError):
            self.attempt().append()
        with self.assertRaises(STORE.StoreError):
            self.attempt(parent, {"lease.json": b"{}"}, self.provider.client())
        self.assertEqual(len(self.provider.mutations()), 1)

    def test_noop_success_does_not_yield_a_witness(self):
        parent = self.initial_commit()
        def no_op(request):
            if request[0] == "PATCH":
                return response(200, self.provider.ref_record(REF))
        self.provider.before = no_op
        with self.assertRaises(STORE.StoreError) as error:
            self.attempt(parent, {"lease.json": b'{"changed":true}'}).append()
        self.assertEqual(error.exception.kind, "uncertain")
        self.assertEqual(self.provider.refs[REF], parent.commit)

    def test_acknowledged_proposal_without_ref_change_has_no_witness(self):
        parent = self.initial_commit()
        def false_acknowledgement(request):
            if request[0] == "PATCH":
                proposed = json.loads(request[3])["sha"]
                return response(200, {"ref": REF, "object": {"type": "commit", "sha": proposed}})
        self.provider.before = false_acknowledgement
        with self.assertRaises(STORE.StoreError) as error:
            self.attempt(parent, {"lease.json": b'{"changed":true}'}).append()
        self.assertEqual(error.exception.kind, "uncertain")
        self.assertEqual(self.provider.refs[REF], parent.commit)

    def test_lost_response_even_after_exact_storage_is_observation_only(self):
        def lose(request, result):
            if request[0] == "POST" and urlsplit(request[1]).path == BASE + "/git/refs":
                raise OSError(FAKE_CREDENTIAL + " private response")
        self.provider.after = lose
        attempt = self.attempt()
        with self.assertRaises(STORE.StoreError) as error:
            attempt.append()
        self.assertEqual(error.exception.kind, "uncertain")
        self.assertNotIn(FAKE_CREDENTIAL, str(error.exception))
        self.provider.after = None
        restarted = self.provider.client()
        observed = restarted.read(SUBJECT)
        self.assertEqual(dict(observed.files), self.files)
        self.assertFalse(hasattr(observed, "consume"))
        with self.assertRaises(STORE.StoreError):
            self.attempt(observed, self.files, restarted)
        with self.assertRaises(STORE.StoreError):
            attempt.append()
        self.assertEqual(len(self.provider.mutations()), 1)

    def test_request_failure_before_acceptance_never_retries(self):
        def lose(request):
            if request[0] == "POST" and urlsplit(request[1]).path == BASE + "/git/refs":
                raise OSError("synthetic pre-acceptance disconnect")
        self.provider.before = lose
        attempt = self.attempt()
        with self.assertRaises(STORE.StoreError) as error:
            attempt.append()
        self.assertEqual(error.exception.kind, "uncertain")
        self.assertNotIn(REF, self.provider.refs)
        with self.assertRaises(STORE.StoreError):
            attempt.append()
        self.assertEqual(len(self.provider.mutations()), 1)

    def test_postwrite_readback_outage_is_not_absence_or_a_witness(self):
        armed = False
        def outage(request):
            if armed and request[0] == "GET" and "/git/ref/" in request[1]:
                return response(503, {"message": FAKE_CREDENTIAL})
        def arm(request, result):
            nonlocal armed
            if request[0] == "POST" and urlsplit(request[1]).path == BASE + "/git/refs":
                armed = True
        self.provider.before, self.provider.after = outage, arm
        with self.assertRaises(STORE.StoreError) as error:
            self.attempt().append()
        self.assertEqual(error.exception.kind, "uncertain")
        self.assertIn(REF, self.provider.refs)

    def test_mutation_malformed_json_duplicate_key_and_wrong_object_fail(self):
        for bad in (b"{", b'{"ref":"x","ref":"y"}', b"[]", b'{"object":{"type":"tag"}}'):
            with self.subTest(bad=bad):
                provider = Provider()
                client = provider.client()
                attempt = self.attempt(client.read(SUBJECT), store=client)
                def change(request, result):
                    if request[0] == "POST" and urlsplit(request[1]).path == BASE + "/git/refs":
                        return STORE.HttpResponse(201, {}, bad)
                provider.after = change
                with self.assertRaises(STORE.StoreError) as error:
                    attempt.append()
                self.assertEqual(error.exception.kind, "uncertain")

    def test_substituted_blob_tree_commit_and_parent_fail_readback(self):
        self.initial_commit()
        controls = (
            ("/git/blobs/", lambda v: {**v, "content": base64.b64encode(b"changed").decode()}),
            ("/git/trees/", lambda v: {**v, "truncated": True}),
            ("/git/trees/", lambda v: {**v, "tree": [{**v["tree"][0], "mode": "120000"}]}),
            ("/git/commits/", lambda v: {**v, "message": v["message"] + "drift"}),
            ("/git/commits/", lambda v: {**v, "parents": [{"sha": "c" * 40}, {"sha": "d" * 40}]}),
        )
        for path, mutate in controls:
            with self.subTest(path=path):
                def change(request, result):
                    if request[0] == "GET" and path in request[1] and not request[1].endswith(ANCHOR):
                        return response(200, mutate(json.loads(result.body)))
                self.provider.after = change
                with self.assertRaises(STORE.StoreError):
                    self.store.read(SUBJECT)
        self.provider.after = None

    def test_ref_drift_between_reads_is_detected(self):
        first = self.initial_commit()
        second = self.attempt(first, {"lease.json": b"{}"}).append().consume(lambda value: value)
        calls = 0
        def move(request, result):
            nonlocal calls
            if request[0] == "GET" and "/git/ref/" in request[1]:
                calls += 1
                if calls == 2:
                    return response(200, {"ref": REF, "object": {"type": "commit", "sha": first.commit}})
        self.provider.after = move
        with self.assertRaises(STORE.StoreError):
            self.store.read(SUBJECT)
        self.assertNotEqual(first.commit, second.commit)

    def test_copy_pickle_fork_and_expiry_cannot_restore_process_objects(self):
        attempt = self.attempt()
        witness = attempt.append()
        for obj in (attempt, witness):
            for copying in (copy.copy, copy.deepcopy, pickle.dumps):
                with self.assertRaises(STORE.StoreError):
                    copying(obj)
            with mock.patch.object(STORE.os, "getpid", return_value=os.getpid() + 1):
                with self.assertRaises(STORE.StoreError):
                    (obj.append() if obj is attempt else obj.consume(lambda _: None))
        with mock.patch.object(witness, "_clock", return_value=1000):
            with self.assertRaises(STORE.StoreError) as error:
                witness.consume(lambda _: self.fail("expired witness callback"))
        self.assertEqual(error.exception.kind, "expired")
        with self.assertRaises(STORE.StoreError):
            witness.consume(lambda _: self.fail("expired witness restored"))

    def test_actual_fork_refuses_before_an_inherited_locked_witness_mutex(self):
        if "fork" not in multiprocessing.get_all_start_methods():
            self.skipTest("native fork is unavailable on this platform")
        context = multiprocessing.get_context("fork")
        reader, writer = context.Pipe(duplex=False)
        witness = self.attempt().append()
        def consume_in_child():
            reader.close()
            try:
                witness.consume(lambda _: writer.send("callback-entered"))
            except STORE.StoreError as error:
                writer.send(error.kind)
            finally:
                writer.close()
        child = context.Process(target=consume_in_child)
        witness._lock.acquire()
        try:
            child.start()
            writer.close()
            self.assertTrue(reader.poll(3), "forked witness blocked on an inherited mutex")
            self.assertEqual(reader.recv(), "already_used")
            child.join(timeout=3)
            self.assertFalse(child.is_alive())
            self.assertEqual(child.exitcode, 0)
        finally:
            witness._lock.release()
            if child.pid is not None and child.is_alive():
                child.terminate()
                child.join(timeout=3)
            reader.close()
            writer.close()
        # The rejected fork did not consume or duplicate the parent's witness.
        self.assertEqual(witness.consume(lambda snapshot: dict(snapshot.files)), self.files)

    def test_preparation_validity_and_nonce_reuse_are_checked_without_http(self):
        count = len(self.provider.calls)
        with self.assertRaises(STORE.StoreError):
            self.store.prepare_append(self.initial, self.files, operation_digest=OPERATION,
                                      producer_bytes=encode(producer()), request_boundary="boundary",
                                      valid_until=NOW)
        with mock.patch.object(STORE.secrets, "token_hex", return_value="e" * 64):
            self.attempt()
            with self.assertRaises(STORE.StoreError):
                self.attempt()
        self.assertEqual(len(self.provider.calls), count)

    def test_restricted_refs_paths_sizes_and_json_are_rejected(self):
        for prefix in ("refs/tags/", "refs/heads/main", "refs/heads/a/../", "refs/heads/-bad/"):
            with self.subTest(prefix=prefix), self.assertRaises(STORE.StoreError):
                STORE.GitHubReleaseStore(repository_id=41, anchor_commit=ANCHOR,
                                        anchor_tree=ANCHOR_TREE, control_prefix=prefix,
                                        credential=lambda: FAKE_CREDENTIAL, transport=self.provider)
        for files in ({}, {"../lease.json": b"{}"}, {"lease.json": b""},
                      {"lease.json": b'{"a":1,"a":2}'}, {"lease.json": b'{"a":NaN}'},
                      {"lease.json": b'{"a":1e9999}'}, {"lease.json": br'{"a":"\ud800"}'},
                      {"lease.json": br'{"\ud800":"a"}'}, {"lease.json": b"9" * 129}):
            with self.subTest(files=files), self.assertRaises(STORE.StoreError):
                self.attempt(files=files)
        self.assertEqual(len(self.provider.mutations()), 0)

    def test_repository_and_known_contents_access_precede_ref_absence(self):
        def wrong_repository(request, result):
            if request[1] == "https://api.github.com" + BASE:
                return response(200, {"id": 99, "full_name": REPOSITORY})
        self.provider.after = wrong_repository
        with self.assertRaises(STORE.StoreError):
            self.store.read(SUBJECT)
        self.provider.after = None
        def no_contents(request):
            if request[1].endswith(ANCHOR):
                return response(404, {})
        self.provider.before = no_contents
        with self.assertRaises(STORE.StoreError):
            self.store.read(SUBJECT)

    def test_git_object_encoding_matches_native_git_without_object_writes(self):
        data = b'{"native_git_oracle":true}\n'
        parent = self.initial_commit()
        second = self.attempt(parent, {"lease.json": data}).append().consume(lambda value: value)
        blob = object_id("blob", data)
        tree_bytes = b"100644 lease.json\0" + bytes.fromhex(blob)
        tree = object_id("tree", tree_bytes)
        commit_bytes = (
            f"tree {tree}\nparent {parent.commit}\n"
            f"author Cargo Allow Release Store <release-store@invalid> {NOW} +0000\n"
            f"committer Cargo Allow Release Store <release-store@invalid> {NOW} +0000\n"
            "\n" + second.message
        ).encode()
        clean = {key: os.environ[key] for key in ("PATH", "SystemRoot", "WINDIR", "TMP", "TEMP")
                 if key in os.environ}
        for kind, wire, expected in (("blob", data, blob), ("tree", tree_bytes, second.tree),
                                     ("commit", commit_bytes, second.commit)):
            with self.subTest(kind=kind):
                try:
                    result = subprocess.run(["git", "hash-object", "-t", kind, "--stdin"],
                                            input=wire, capture_output=True, env=clean,
                                            timeout=5, check=False)
                except FileNotFoundError:
                    self.skipTest("Git is unavailable for the read-only object oracle")
                self.assertEqual(result.returncode, 0)
                self.assertEqual(result.stdout.decode().strip(), expected)


class ReadbackTests(unittest.TestCase):
    def setUp(self):
        self.providers_from = len(Provider.instances)
        self.provider = Provider()
        self.store = self.provider.client()

    def tearDown(self):
        for provider in Provider.instances[self.providers_from:]:
            self.assertEqual(provider.unexpected, [], "unexpected intercepted request")

    def read_source(self):
        return self.store.read_source(self.provider.source_input(),
                                      approved_actor_id=404,
                                      approved_actor_login="release-operator")

    def read_artifact(self, transfer=None):
        return self.store.read_artifact(self.provider.transfer() if transfer is None else transfer,
                                       artifact_id=505, expected_producer=producer())

    def test_exact_source_and_approved_actor_are_observed(self):
        self.assertEqual(self.read_source(), self.provider.source_body)
        self.assertEqual(self.provider.mutations(), [])

    def test_source_claims_cannot_choose_the_approved_actor(self):
        for update in (
            {"id": 203}, {"issue_url": "https://api.github.com" + BASE + "/issues/99"},
            {"user": {"id": 405, "login": "release-operator"}},
            {"user": {"id": 404, "login": "unapproved-actor"}},
            {"body": "edited source"}, {"body": None}, {"body": "\ud800"},
        ):
            with self.subTest(update=update):
                original = copy.deepcopy(self.provider.source)
                self.provider.source.update(update)
                with self.assertRaises(STORE.StoreError):
                    self.read_source()
                self.provider.source = original
        forged = self.provider.source_input()
        forged["author"] = "unapproved-actor"
        with self.assertRaises(STORE.StoreError):
            self.store.read_source(forged, approved_actor_id=404,
                                   approved_actor_login="release-operator")

    def test_unsupported_source_kind_or_repo_ref_is_rejected_before_http(self):
        for key, value in (("kind", "workflow_dispatch"), ("repository", "foreign/repo"),
                           ("reference", "https://attacker.invalid"),
                           ("body_digest", "sha256:" + "A" * 64)):
            with self.subTest(key=key):
                candidate = {**self.provider.source_input(), key: value}
                with self.assertRaises(STORE.StoreError):
                    self.store.read_source(candidate, approved_actor_id=404,
                                           approved_actor_login="release-operator")
        self.assertEqual(self.provider.calls, [])

    def test_source_outage_and_credential_failure_are_sanitized(self):
        def outage(request):
            if "/issues/comments/" in request[1]:
                return response(503, {"message": FAKE_CREDENTIAL})
        self.provider.before = outage
        with self.assertRaises(STORE.StoreError) as error:
            self.read_source()
        self.assertNotIn(FAKE_CREDENTIAL, str(error.exception))
        self.provider.before = None
        def bad_credential():
            raise RuntimeError(FAKE_CREDENTIAL)
        self.store._credential = bad_credential
        with self.assertRaises(STORE.StoreError) as error:
            self.read_source()
        self.assertNotIn(FAKE_CREDENTIAL, str(error.exception))

    def test_exact_artifact_files_and_provenance_are_independently_read(self):
        files = self.read_artifact()
        self.assertEqual(dict(files), self.provider.artifact_files)
        self.assertEqual(self.provider.mutations(), [])
        paths = [urlsplit(call[1]).path for call in self.provider.calls]
        self.assertEqual(paths.count(BASE + "/actions/artifacts/505"), 2)
        self.assertIn(BASE + "/actions/runs/101/attempts/1/jobs", paths)
        self.assertIn(BASE + "/git/commits/" + ANCHOR, paths)

    def test_selected_workflow_path_accepts_exact_bare_full_and_short_refs(self):
        workflow = ".github/workflows/release.yml"
        for git_ref, short_ref, event, trust in (
            ("refs/heads/main", "main", "workflow_dispatch", "ManualDispatch"),
            ("refs/heads/release/0.2.0", "release/0.2.0", "workflow_dispatch", "ManualDispatch"),
            ("refs/tags/v0.2.0", "v0.2.0", "push", "TagWorkflow"),
        ):
            for path in (workflow, workflow + "@" + git_ref, workflow + "@" + short_ref):
                with self.subTest(git_ref=git_ref, path=path):
                    provider = Provider()
                    selected = {**producer(), "git_ref": git_ref}
                    transfer = {**provider.transfer(), "producer": selected, "trust_class": trust}
                    provider.run.update({"path": path, "head_branch": short_ref, "event": event})
                    files = provider.client().read_artifact(
                        transfer, artifact_id=505, expected_producer=selected)
                    self.assertEqual(dict(files), provider.artifact_files)
                    self.assertEqual(provider.mutations(), [])
                    paths = [urlsplit(call[1]).path for call in provider.calls]
                    self.assertEqual(paths.count(BASE + "/actions/artifacts/505"), 2)
                    self.assertIn(BASE + "/actions/runs/101/attempts/1/jobs", paths)
                    self.assertIn(BASE + "/git/commits/" + ANCHOR, paths)
                    self.assertIn(BASE + "/actions/artifacts/505/zip", paths)
                    self.assertIn(urlsplit(provider.download_url).path, paths)

    def test_short_workflow_ref_keeps_exact_path_ref_and_event_binding(self):
        workflow = ".github/workflows/release.yml"
        for git_ref, short_ref, event, trust in (
            ("refs/heads/main", "main", "workflow_dispatch", "ManualDispatch"),
            ("refs/heads/release/0.2.0", "release/0.2.0", "workflow_dispatch", "ManualDispatch"),
            ("refs/tags/v0.2.0", "v0.2.0", "push", "TagWorkflow"),
        ):
            for update in (
                {"path": workflow + "@other"},
                {"path": workflow + "@refs/heads/other"},
                {"path": workflow + "@refs/tags/other"},
                {"path": ".github/workflows/foreign.yml@" + short_ref},
                {"path": "foreign/repository/" + workflow + "@" + short_ref},
                {"path": workflow + "@" + short_ref + "@other"},
                {"head_branch": "other"},
                {"event": "pull_request"},
                {"event": "workflow_dispatch" if event == "push" else "push"},
            ):
                with self.subTest(git_ref=git_ref, update=update):
                    provider = Provider()
                    selected = {**producer(), "git_ref": git_ref}
                    transfer = {**provider.transfer(), "producer": selected, "trust_class": trust}
                    provider.run.update({"path": workflow + "@" + short_ref,
                                         "head_branch": short_ref, "event": event, **update})
                    with self.assertRaises(STORE.StoreError) as error:
                        provider.client().read_artifact(
                            transfer, artifact_id=505, expected_producer=selected)
                    self.assertEqual(error.exception.kind, "mismatch")
                    self.assertEqual(str(error.exception),
                                     "release operation store: mismatch: workflow attempt provenance differs")
                    self.assertEqual(provider.mutations(), [])
                    paths = [urlsplit(call[1]).path for call in provider.calls]
                    self.assertIn(BASE + "/actions/runs/101/attempts/1", paths)
                    self.assertNotIn(BASE + "/actions/runs/101/attempts/1/jobs", paths)
                    self.assertNotIn(BASE + "/actions/artifacts/505/zip", paths)
                    self.assertNotIn(urlsplit(provider.download_url).path, paths)

    def test_selected_artifact_and_producer_cannot_be_replaced_by_downloaded_claims(self):
        for key, value in (
            ("stable_artifact_id", "506"), ("provider_id", "other-provider"),
            ("trust_class", "PullRequest"), ("untrusted_input_posture", "SanitizedDataOnly"),
            ("producer", {**producer(), "run_attempt": 2}),
            ("producer", {**producer(), "run_attempt": True}), ("schema_version", True),
        ):
            with self.subTest(key=key):
                transfer = {**self.provider.transfer(), key: value}
                with self.assertRaises(STORE.StoreError):
                    self.read_artifact(transfer)
        self.assertEqual(self.provider.calls, [])

    def test_wrong_artifact_id_name_run_repo_commit_and_expiry_fail(self):
        original = copy.deepcopy(self.provider.artifact)
        for update in (
            {"id": 506}, {"name": "wrong"}, {"expired": True},
            {"expires_at": date(NOW)}, {"created_at": date(NOW + 1)},
            {"workflow_run": {**original["workflow_run"], "id": 102}},
            {"workflow_run": {**original["workflow_run"], "repository_id": 99}},
            {"workflow_run": {**original["workflow_run"], "head_repository_id": 99}},
            {"workflow_run": {**original["workflow_run"], "head_sha": "d" * 40}},
        ):
            with self.subTest(update=update):
                self.provider.artifact = {**original, **update}
                with self.assertRaises(STORE.StoreError):
                    self.read_artifact(self.provider.transfer() | {"provider_artifact_name": original["name"]})
        self.provider.artifact = original

    def test_wrong_attempt_workflow_ref_tree_or_job_fail(self):
        for update in (
            {"run_attempt": 2}, {"run_attempt": True}, {"path": ".github/workflows/foreign.yml"},
            {"head_branch": "other"}, {"event": "pull_request"}, {"event": "push"},
            {"head_repository": {"id": 99}}, {"head_sha": "d" * 40},
        ):
            with self.subTest(update=update):
                original = copy.deepcopy(self.provider.run)
                self.provider.run.update(update)
                with self.assertRaises(STORE.StoreError):
                    self.read_artifact()
                self.provider.run = original
        for jobs in (
            [], [{**self.provider.jobs[0], "id": 304}],
            [{**self.provider.jobs[0], "run_id": 102}],
            [{**self.provider.jobs[0], "head_sha": "c" * 40}],
            [{**self.provider.jobs[0], "conclusion": "failure"}],
            [{**self.provider.jobs[0], "started_at": date(NOW)}],
            self.provider.jobs * 2,
        ):
            with self.subTest(jobs=jobs):
                original = self.provider.jobs
                self.provider.jobs = jobs
                with self.assertRaises(STORE.StoreError):
                    self.read_artifact()
                self.provider.jobs = original

    def test_tag_workflow_requires_an_actual_selected_tag_push(self):
        selected = {**producer(), "git_ref": "refs/tags/v0.2.0"}
        transfer = {**self.provider.transfer(), "trust_class": "TagWorkflow", "producer": selected}
        self.provider.run.update({"event": "push", "head_branch": "v0.2.0"})
        files = self.store.read_artifact(transfer, artifact_id=505, expected_producer=selected)
        self.assertEqual(dict(files), self.provider.artifact_files)
        self.provider.run["event"] = "workflow_dispatch"
        with self.assertRaises(STORE.StoreError):
            self.store.read_artifact(transfer, artifact_id=505, expected_producer=selected)
        self.assertEqual(self.provider.mutations(), [])

    def test_job_inventory_truncation_does_not_hide_another_attempt(self):
        def truncate(request, result):
            if "/attempts/1/jobs?" in request[1]:
                return response(200, {"total_count": 101, "jobs": self.provider.jobs})
        self.provider.after = truncate
        with self.assertRaises(STORE.StoreError):
            self.read_artifact()

    def test_signed_download_never_receives_github_authorization(self):
        self.read_artifact()
        signed = [call for call in self.provider.calls if urlsplit(call[1]).hostname == "artifacts.example.test"]
        self.assertEqual(len(signed), 1)
        self.assertNotIn("Authorization", signed[0][2])
        self.assertNotIn("X-GitHub-Api-Version", signed[0][2])

    def test_unselected_insecure_or_credential_bearing_redirect_is_refused(self):
        original = self.provider.download_url
        for url in (
            "http://artifacts.example.test/bundle",
            "https://attacker.invalid/bundle",
            "https://user:password@artifacts.example.test/bundle",
            "https://artifacts.example.test:444/bundle",
            "https://artifacts.example.test:invalid/bundle",
            "https://[artifacts.example.test/bundle",
            "https://artifacts.example.test/bundle#fragment",
            "https://127.0.0.1/bundle", "https://artifacts.example.test/\nsecret",
        ):
            with self.subTest(url=url):
                self.provider.download_url = url
                with self.assertRaises(STORE.StoreError):
                    self.read_artifact()
        self.provider.download_url = original
        self.assertFalse(any(urlsplit(call[1]).hostname != "api.github.com"
                             for call in self.provider.calls))

    def test_archive_traversal_duplicates_symlinks_extra_or_tampered_bytes_fail(self):
        original = self.provider.artifact_zip
        nul_name = archive({"authorization.jsonXhidden": self.provider.artifact_files["authorization.json"]})
        nul_name = nul_name.replace(b"authorization.jsonXhidden", b"authorization.json\0hidden")
        controls = (
            b"not a zip",
            archive({"../authorization.json": b"{}"}),
            archive({"/authorization.json": b"{}"}),
            archive({"authorization.json": b"changed"}),
            archive(self.provider.artifact_files, mode=stat.S_IFLNK | 0o777),
            archive(self.provider.artifact_files, duplicate="authorization.json"),
            archive({**self.provider.artifact_files, "unexpected.json": b"{}"}),
            nul_name,
        )
        for bad in controls:
            with self.subTest(size=len(bad)):
                self.provider.artifact_zip = bad
                with self.assertRaises(STORE.StoreError):
                    self.read_artifact()
        self.provider.artifact_zip = original

    def test_corrupt_deflated_member_is_a_sanitized_instrument_failure(self):
        output = io.BytesIO()
        with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as result:
            for name, data in self.provider.artifact_files.items():
                result.writestr(name, data)
        compressed = output.getvalue()
        with zipfile.ZipFile(io.BytesIO(compressed)) as result:
            member = result.getinfo("authorization.json")
            self.assertEqual(member.compress_type, zipfile.ZIP_DEFLATED)
            self.assertGreater(member.compress_size, 1)
            payload_start = member.header_offset + len(member.FileHeader())
        self.provider.artifact_zip = compressed
        self.assertEqual(dict(self.read_artifact()), self.provider.artifact_files)

        corrupt = bytearray(compressed)
        # Preserve the selected archive framing but make the first deflate
        # block's BTYPE bits 11 (reserved), exercising the real decompressor.
        corrupt[payload_start] = (corrupt[payload_start] & ~0x06) | 0x06
        self.provider.artifact_zip = bytes(corrupt)
        with self.assertRaises(STORE.StoreError) as error:
            self.read_artifact()
        self.assertEqual(error.exception.kind, "instrument_failure")
        self.assertEqual(str(error.exception),
                         "release operation store: instrument_failure: malformed artifact archive")
        self.assertIsNone(error.exception.__cause__)
        self.assertTrue(error.exception.__suppress_context__)
        self.assertEqual(self.provider.mutations(), [])

    def test_inventory_case_collision_and_oversize_cannot_escape_bounds(self):
        transfer = self.provider.transfer()
        transfer["files"] = transfer["files"] * 2
        with self.assertRaises(STORE.StoreError):
            self.read_artifact(transfer)
        transfer = self.provider.transfer()
        transfer["files"][0]["size_bytes"] = 2 * 1024 * 1024 + 1
        with self.assertRaises(STORE.StoreError):
            self.read_artifact(transfer)
        self.provider.artifact_files = {"a.json": b"{}", "A.json": b"{}"}
        self.provider.artifact_zip = archive(self.provider.artifact_files)
        with self.assertRaises(STORE.StoreError):
            self.read_artifact()

    def test_metadata_movement_during_download_is_rejected(self):
        def expire(request, result):
            if urlsplit(request[1]).hostname == "artifacts.example.test":
                self.provider.artifact["expires_at"] = date(NOW)
        self.provider.after = expire
        with self.assertRaises(STORE.StoreError):
            self.read_artifact()

    def test_untrusted_provider_errors_never_echo_tokens_or_signed_urls(self):
        def expose(request):
            if urlsplit(request[1]).hostname == "artifacts.example.test":
                raise RuntimeError(FAKE_CREDENTIAL + self.provider.download_url)
        self.provider.before = expose
        with self.assertRaises(STORE.StoreError) as error:
            self.read_artifact()
        self.assertNotIn(FAKE_CREDENTIAL, str(error.exception))
        self.assertNotIn("sig=", str(error.exception))

    def test_typed_harness_preserves_supplied_bytes_without_an_eligibility_model(self):
        before = b'{\n  "opaque_input": "before"\n}\n'
        after = b'{\n  "opaque_input": "after"\n}\n'
        with tempfile.TemporaryDirectory(prefix="release-store-python-bridge-") as scratch:
            root = Path(scratch)
            source = root / "input"
            output = root / "output"
            for name, payload in (("before", before), ("after", after)):
                (source / name).mkdir(parents=True)
                (source / name / "operation.json").write_bytes(payload)
            self.provider.artifact_files = {"operation.json": before}
            (source / "transfer.json").write_bytes(encode(self.provider.transfer()))
            (source / "producer.json").write_bytes(json.dumps(producer(), indent=2).encode() + b"\n")
            (source / "source.json").write_bytes(encode(self.provider.source_input()))
            (source / "source.body").write_bytes(self.provider.source_body)
            (source / "subject.txt").write_text(SUBJECT)
            (source / "operation.txt").write_text(OPERATION)
            with mock.patch("builtins.print"):
                typed_roundtrip(source, output)
            self.assertEqual((output / "operation.json").read_bytes(), after)
            self.assertEqual((output / "downloaded" / "operation.json").read_bytes(), before)
            self.assertEqual((output / "source.body").read_bytes(), self.provider.source_body)


class HttpTests(unittest.TestCase):
    def native_source_headers(self, header_lines, *, denied=False, raw_header_items=False):
        provider = Provider()
        native_responses = []

        class RawHeaderItems:
            def __init__(self, lines):
                self.lines = list(lines)

            def items(self):
                return list(self.lines)

        class NativeResponse:
            def __init__(self, result):
                self.status = result.status
                if raw_header_items:
                    # Malformed fixtures must reach the transport's header parser;
                    # newer email policies reject them during Message construction.
                    self.headers = RawHeaderItems(header_lines(result))
                else:
                    self.headers = Message()
                    for name, value in header_lines(result):
                        self.headers.add_header(name, value)
                self.stream = io.BytesIO(result.body)
                self.reads = 0

            def __enter__(self):
                return self

            def __exit__(self, *_):
                return False

            def read1(self, count):
                self.reads += 1
                return self.stream.read1(count)

        def open_response(request, timeout):
            result = provider(request.get_method(), request.full_url,
                              dict(request.header_items()), request.data,
                              timeout, 4 * 1024 * 1024)
            native = NativeResponse(result)
            native_responses.append(native)
            return native

        opener = mock.Mock()
        opener.open.side_effect = open_response
        store = STORE.GitHubReleaseStore(
            repository_id=41, anchor_commit=ANCHOR, anchor_tree=ANCHOR_TREE,
            control_prefix=PREFIX, credential=lambda: FAKE_CREDENTIAL,
            transport=STORE.https_transport)
        with mock.patch.object(STORE, "build_opener", return_value=opener), \
                mock.patch.object(STORE, "_headers", wraps=STORE._headers) as normalize:
            if denied:
                with self.assertRaises(STORE.StoreError) as error:
                    store.read_source(provider.source_input(), approved_actor_id=404,
                                      approved_actor_login="release-operator")
                self.assertEqual(error.exception.kind, "instrument_failure")
                self.assertEqual(str(error.exception),
                                 "release operation store: instrument_failure: provider exchange failed")
                self.assertNotIn(FAKE_CREDENTIAL, str(error.exception))
                self.assertEqual([item.reads for item in native_responses], [0])
                self.assertEqual(normalize.call_count, 1)
            else:
                data = store.read_source(provider.source_input(), approved_actor_id=404,
                                         approved_actor_login="release-operator")
                self.assertEqual(data, provider.source_body)
        calls = 1 if denied else 3
        self.assertEqual(opener.open.call_count, calls)
        self.assertEqual([call[0] for call in provider.calls], ["GET"] * calls)
        self.assertEqual(provider.unexpected, [])
        self.assertEqual((provider.refs, provider.blobs, provider.trees, provider.commits),
                         ({}, {}, {}, {}))

    def test_native_source_accepts_repeated_vary_in_received_order(self):
        combined = [("Vary", "Accept, Accept-Encoding, X-Selected"), ("X-Trace", "bounded")]
        repeated = [("vArY", "Accept"), ("X-Trace", "bounded"),
                    ("VARY", "Accept-Encoding"), ("vary", "X-Selected")]
        for lines in (combined, repeated):
            with self.subTest(lines=lines):
                self.native_source_headers(lambda _result: lines)
        self.assertEqual(STORE._headers(iter(repeated)), STORE._headers(iter(combined)))
        self.assertEqual(STORE._headers(iter(repeated))["vary"],
                         "Accept, Accept-Encoding, X-Selected")

    def test_native_source_rejects_every_other_repeated_header(self):
        for name, value in [
            ("Content-Length", None), ("Location", "https://artifacts.example.test/selected"),
            ("Content-Encoding", "identity"), ("Date", "Sat, 10 Oct 2026 12:00:00 GMT"),
            ("Transfer-Encoding", "chunked"), ("Content-Type", "application/json"),
            ("X-Trace", FAKE_CREDENTIAL),
        ]:
            def single(result):
                return [(name, str(len(result.body)) if value is None else value)]

            def repeated(result):
                headers = single(result)
                return headers + [(name.swapcase(), headers[0][1])]

            with self.subTest(header=name):
                self.native_source_headers(single)
                self.native_source_headers(repeated, denied=True)

    def test_native_source_repeated_vary_keeps_physical_header_bounds(self):
        for label, lines, denied in [
            ("128-lines", [("Vary", "Accept")] * 128, False),
            ("129-lines", [("Vary", "Accept")] * 129, True),
            ("65536-characters", [("Vary", "x" * 32764), ("vary", "y" * 32764)], False),
            ("65537-characters", [("Vary", "x" * 32764), ("vary", "y" * 32765)], True),
        ]:
            with self.subTest(bound=label):
                self.native_source_headers(lambda _result: lines, denied=denied)

    def test_native_source_repeated_vary_keeps_malformed_headers_sanitized(self):
        for bad_line in [("Bad Header", FAKE_CREDENTIAL),
                         ("vary", "Accept\n" + FAKE_CREDENTIAL),
                         ("vary", "Accept\x01" + FAKE_CREDENTIAL),
                         ("vary", "non-ascii-\u00e9" + FAKE_CREDENTIAL)]:
            with self.subTest(header=bad_line[0]):
                with mock.patch.object(Message, "add_header", side_effect=AssertionError(
                        "malformed fixtures must preserve raw response-header items")):
                    self.native_source_headers(
                        lambda _result: [("Vary", "Accept-Encoding"), bad_line],
                        denied=True, raw_header_items=True)

    def test_import_and_constructor_do_not_read_environment_or_credentials(self):
        class NoEnvironment(dict):
            def refused(self, *_args, **_kwargs):
                raise AssertionError("ambient environment read")
            __getitem__ = __iter__ = __contains__ = __len__ = refused
            get = copy = items = keys = values = refused
        with mock.patch.object(os, "environ", NoEnvironment()):
            isolated_spec = importlib.util.spec_from_file_location("store_import_fixture", MODULE_PATH)
            isolated = importlib.util.module_from_spec(isolated_spec)
            sys.modules[isolated_spec.name] = isolated
            with mock.patch.object(os, "getenv", side_effect=AssertionError("ambient credential read")):
                isolated_spec.loader.exec_module(isolated)
                isolated.GitHubReleaseStore(
                    repository_id=41, anchor_commit=ANCHOR, anchor_tree=ANCHOR_TREE,
                    control_prefix=PREFIX,
                    credential=lambda: self.fail("constructor called credential provider"))
            del sys.modules[isolated_spec.name]

    def test_native_transport_has_no_redirect_and_bounds_the_actual_stream(self):
        class NativeResponse:
            status = 200
            headers = {}
            def __init__(self, data):
                self.stream = io.BytesIO(data)
            def read1(self, count):
                return self.stream.read(count)
            def __enter__(self):
                return self
            def __exit__(self, *_):
                return False
        opener = mock.Mock()
        opener.open.return_value = NativeResponse(b"abcdef")
        with mock.patch.object(STORE, "build_opener", return_value=opener) as build:
            with self.assertRaises(STORE.StoreError):
                STORE.https_transport("GET", "https://api.github.com" + BASE,
                                      {}, None, 1, 5)
            handlers = build.call_args.args
            self.assertTrue(any(isinstance(item, STORE._NoRedirect) for item in handlers))
            self.assertEqual(next(item for item in handlers if isinstance(item, STORE.ProxyHandler)).proxies, {})
        self.assertIsNone(STORE._NoRedirect().redirect_request(None, None, 302, "", None, "https://other"))

    def test_native_transport_deadline_and_content_length_are_checked(self):
        class NativeResponse:
            status = 200
            headers = {"Content-Length": "100"}
            def __enter__(self):
                return self
            def __exit__(self, *_):
                return False
            def read1(self, _):
                raise AssertionError("oversized body should not be read")
        opener = mock.Mock()
        opener.open.return_value = NativeResponse()
        with mock.patch.object(STORE, "build_opener", return_value=opener):
            with self.assertRaises(STORE.StoreError):
                STORE.https_transport("GET", "https://api.github.com" + BASE,
                                      {}, None, 1, 5)
        NativeResponse.headers = {}
        with mock.patch.object(STORE, "build_opener", return_value=opener), \
             mock.patch.object(STORE.time, "monotonic", side_effect=[10, 12]):
            with self.assertRaises(STORE.StoreError):
                STORE.https_transport("GET", "https://api.github.com" + BASE,
                                      {}, None, 1, 5)

    def test_native_redirect_response_is_returned_once_and_duplicate_headers_fail(self):
        headers = Message()
        headers.add_header("Location", "https://artifacts.example.test/bundle")
        opener = mock.Mock()
        opener.open.side_effect = STORE.HTTPError(
            "https://api.github.com" + BASE, 302, "redirect", headers, io.BytesIO(b""))
        with mock.patch.object(STORE, "build_opener", return_value=opener):
            result = STORE.https_transport("GET", "https://api.github.com" + BASE,
                                          {}, None, 1, 5)
        self.assertEqual(result.status, 302)
        self.assertEqual(opener.open.call_count, 1)
        headers.add_header("Location", "https://other.example.test/bundle")
        opener.open.side_effect = STORE.HTTPError(
            "https://api.github.com" + BASE, 302, "redirect", headers, io.BytesIO(b""))
        with mock.patch.object(STORE, "build_opener", return_value=opener):
            with self.assertRaises(STORE.StoreError):
                STORE.https_transport("GET", "https://api.github.com" + BASE,
                                      {}, None, 1, 5)

    def test_oversized_intercepted_response_and_bad_json_fail_closed(self):
        for body in (b" " * (4 * 1024 * 1024 + 1), b'{"id":41,"id":41}', b"[]", b"{"):
            provider = Provider()
            provider.before = lambda _: STORE.HttpResponse(200, {}, body)
            with self.subTest(size=len(body)), self.assertRaises(STORE.StoreError):
                provider.client().read(SUBJECT)

    def test_ambiguous_headers_wrong_encoding_and_truncated_length_fail_closed(self):
        for headers in (
            {"Location": "https://one.example", "location": "https://two.example"},
            {"Content-Encoding": "gzip"}, {"Content-Length": "99"},
            {"Content-Length": "1" * 4000}, {"Location": "https://example/\r\ninjection"},
            {"header": 12}, None,
        ):
            provider = Provider()
            provider.before = lambda _: STORE.HttpResponse(200, headers, b"{}")
            with self.subTest(headers=headers), self.assertRaises(STORE.StoreError):
                provider.client().read(SUBJECT)

    def test_transport_instrument_failure_keeps_its_class_without_its_message(self):
        def failure(_):
            raise STORE.StoreError("instrument_failure", FAKE_CREDENTIAL)
        provider = Provider()
        provider.before = failure
        with self.assertRaises(STORE.StoreError) as error:
            provider.client().read(SUBJECT)
        self.assertEqual(error.exception.kind, "instrument_failure")
        self.assertNotIn(FAKE_CREDENTIAL, str(error.exception))


def typed_roundtrip(input_root: Path, output_root: Path):
    """Called by Cargo's integration test with Rust-constructed domain records."""
    before = {path.name: path.read_bytes() for path in (input_root / "before").glob("*.json")}
    after = {path.name: path.read_bytes() for path in (input_root / "after").glob("*.json")}
    transfer = json.loads((input_root / "transfer.json").read_bytes())
    producer_bytes = (input_root / "producer.json").read_bytes()
    expected_producer = json.loads(producer_bytes)
    source = json.loads((input_root / "source.json").read_bytes())
    source_body = (input_root / "source.body").read_bytes()
    subject = (input_root / "subject.txt").read_text().strip()
    operation = (input_root / "operation.txt").read_text().strip()
    provider = Provider()
    provider.source_body = source_body
    provider.source["body"] = source_body.decode()
    provider.artifact_files = before
    provider.artifact_zip = archive(before)
    provider.artifact["name"] = transfer["provider_artifact_name"]
    provider.artifact["expires_at"] = date(NOW + 2 * 86400)
    provider.jobs = [{**provider.jobs[0], "started_at": date(NOW - 150),
                      "status": "in_progress", "conclusion": None, "completed_at": None}]
    store = provider.client()
    observed_source = store.read_source(source, approved_actor_id=404,
                                        approved_actor_login="release-operator")
    downloaded = store.read_artifact(transfer, artifact_id=505,
                                     expected_producer=expected_producer)
    initial = store.read(subject)
    first = store.prepare_append(initial, downloaded, operation_digest=operation,
                                 producer_bytes=producer_bytes,
                                 request_boundary="typed-selection",
                                 valid_until=NOW + 60).append().consume(lambda snapshot: snapshot)
    witness = store.prepare_append(first, after, operation_digest=operation,
                                   producer_bytes=producer_bytes,
                                   request_boundary="typed-lease-renewal",
                                   valid_until=NOW + 60).append()
    def retain(snapshot):
        output_root.mkdir(parents=True)
        for name, data in snapshot.files.items():
            (output_root / name).write_bytes(data)
        (output_root / "source.body").write_bytes(observed_source)
        download_dir = output_root / "downloaded"
        download_dir.mkdir()
        for name, data in downloaded.items():
            (download_dir / name).write_bytes(data)
    witness.consume(retain)
    try:
        witness.consume(lambda _: (_ for _ in ()).throw(AssertionError("duplicate callback")))
    except STORE.StoreError:
        pass
    else:
        raise AssertionError("typed witness was reusable")
    if provider.unexpected or len(provider.mutations()) != 2:
        raise AssertionError("typed provider sequence differs")
    print("typed provider roundtrip: exact source/artifact/Git bytes; one create and one nonforced append")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--typed-input", type=Path)
    parser.add_argument("--typed-output", type=Path)
    options, remaining = parser.parse_known_args()
    if options.typed_input is not None or options.typed_output is not None:
        if options.typed_input is None or options.typed_output is None or remaining:
            raise SystemExit("typed roundtrip requires only both fixture paths")
        typed_roundtrip(options.typed_input, options.typed_output)
    else:
        unittest.main(argv=[sys.argv[0], *remaining])
