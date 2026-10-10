#!/usr/bin/env python3
"""Intercept production final-tag I/O; full lifecycle requires the real Rust CLI.

The fixture provider models HTTP and Git exchanges only. It never decides
release eligibility. --io-only runs the transport controls available without
Rust. --bridge and --fixture run the actual compiled typed consumer as well.
No credentials, external refs, tags, uploads or publication are accessed.
"""
from __future__ import annotations

import argparse
import copy
from datetime import datetime, timezone
from email.utils import formatdate
import importlib.util
import json
import math
import os
from pathlib import Path
import pickle
import shutil
import stat
import sys
import tempfile
import threading
import unittest
from unittest import mock
from urllib.parse import urlsplit

sys.dont_write_bytecode = True


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise SystemExit("could not load the selected production interface")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


HERE = Path(__file__).resolve().parent
PROVIDER = load("final_tag_provider_contract", HERE / "test-release-operation-store.py")
DRIVER = load("release_final_tag", HERE / "release-final-tag.py")
STORE = PROVIDER.STORE
NOW = PROVIDER.NOW
SECRET = PROVIDER.FAKE_CREDENTIAL
ANCHOR = PROVIDER.ANCHOR
TREE = PROVIDER.ANCHOR_TREE
BASE = PROVIDER.BASE
GIT = Path(shutil.which("git") or "/unavailable-native-git")
FULL_BRIDGE = None
FULL_FIXTURE = None


def producer():
    result = PROVIDER.producer()
    result["schema_id"] = "cargo-allow.release-operation-head.v1"
    return result


def raw_tag(at=NOW, message="Synthetic intercepted final-tag fixture.\n"):
    return DRIVER.tag_object_bytes(ANCHOR, "Release Fixture", "release@example.invalid", at, message)


def tag_record(raw):
    header, message = raw.decode().split("\n\n", 1)
    lines = header.splitlines()
    identity, stamp, offset = lines[3].removeprefix("tagger ").rsplit(" ", 2)
    name, email = identity.rsplit(" <", 1)
    if offset != "+0000":
        raise AssertionError("fixture tag must use the selected UTC encoding")
    return {"sha": STORE._git_oid("tag", raw), "tag": "v0.2.0", "message": message,
            "tagger": {"name": name, "email": email.removesuffix(">"), "date": PROVIDER.date(int(stamp))},
            "object": {"type": "commit", "sha": lines[0].removeprefix("object ")}}


class DriverProvider(PROVIDER.Provider):
    """Intercept the actual HTTP protocol; no semantic authorization model."""
    def __init__(self):
        super().__init__()
        self.now = NOW
        self.tag = None
        self.objects = {}
        self.artifacts = {}
        self.date_header = True
        self.next_artifact = 600
        self.trees[TREE] = []
        self.jobs[0].update(status="in_progress", conclusion=None,
                            started_at=PROVIDER.date(NOW - 3600), completed_at=None)

    def client(self, **kwargs):
        return DRIVER.TagStore(repository_id=41, anchor_commit=ANCHOR, anchor_tree=TREE,
            control_prefix=PROVIDER.PREFIX, credential=lambda: SECRET,
            download_hosts=frozenset({"artifacts.example.test"}), transport=self,
            clock=lambda: self.now, monotonic=lambda: 100.0, **kwargs)

    def __call__(self, *arguments):
        result = super().__call__(*arguments)
        if self.date_header:
            headers = dict(result.headers)
            headers["Date"] = formatdate(self.now, usegmt=True)
            return STORE.HttpResponse(result.status, headers, result.body)
        return result

    def set_tag(self, raw):
        oid = STORE._git_oid("tag", raw)
        self.objects[oid] = tag_record(raw)
        self.tag = {"ref": DRIVER.TAG_REF, "object": {"type": "tag", "sha": oid}}

    def retain(self, files, name, *, artifact_id=None):
        """Intercept the pinned action's checkpoint upload and assign its ID."""
        if artifact_id is None:
            artifact_id = self.next_artifact
            self.next_artifact += 1
        metadata = {"id": artifact_id, "name": name, "expired": False,
                    "created_at": PROVIDER.date(self.now), "expires_at": PROVIDER.date(self.now + 7200),
                    "workflow_run": {"id": 101, "repository_id": 41, "head_repository_id": 41, "head_sha": ANCHOR}}
        zipped = PROVIDER.archive(files)
        metadata["digest"] = STORE.sha256(zipped)
        self.artifacts[artifact_id] = (metadata, dict(files), zipped)
        return artifact_id

    def transfer(self, artifact_id):
        metadata, files, _ = self.artifacts[artifact_id]
        return {"schema_id": "cargo-allow.release-artifact-transfer.v1", "schema_version": 1,
                "transfer_id": "selected-inputs-" + str(artifact_id), "role": "final-tag-selected-inputs",
                "stable_artifact_id": str(artifact_id), "producer": producer(),
                "provider_id": "github-actions-artifact", "provider_artifact_name": metadata["name"],
                "files": [{"path": path, "size_bytes": len(raw), "sha256": STORE.sha256(raw)}
                          for path, raw in sorted(files.items())],
                "semantic_payload_digest": PROVIDER.OPERATION, "trust_class": "ManualDispatch",
                "untrusted_input_posture": "StrictByteMatch", "created_at_utc": metadata["created_at"],
                "claim_boundary": ["exact_producer_identity"], "limitations": []}

    def route(self, request):
        method, url, headers, _, _, _ = request
        parsed = urlsplit(url)
        path = parsed.path
        if parsed.hostname == "artifacts.example.test" and path.startswith("/selected-"):
            artifact_id = int(path.removeprefix("/selected-").removesuffix(".zip"))
            if method != "GET" or "Authorization" in headers:
                raise AssertionError("credential-bearing or unexpected signed download")
            return STORE.HttpResponse(200, {}, self.artifacts[artifact_id][2])
        if parsed.hostname == "api.github.com" and method == "GET":
            if headers.get("Authorization") != "Bearer " + SECRET:
                raise AssertionError("fixture authentication is missing")
            if path == BASE + "/git/ref/tags/v0.2.0":
                return PROVIDER.response(404, {}) if self.tag is None else PROVIDER.response(200, self.tag)
            if path.startswith(BASE + "/git/tags/"):
                return PROVIDER.response(200, self.objects[path.rsplit("/", 1)[-1]])
            if path.startswith(BASE + "/actions/artifacts/"):
                suffix = path.removeprefix(BASE + "/actions/artifacts/")
                artifact_id = int(suffix.split("/")[0])
                if artifact_id in self.artifacts:
                    if suffix.endswith("/zip"):
                        return PROVIDER.response(302, {}, {"Location": f"https://artifacts.example.test/selected-{artifact_id}.zip?sig=fixture"})
                    return PROVIDER.response(200, self.artifacts[artifact_id][0])
        return super().route(request)


class GitRunner:
    """Physical Git process interception; native Git independently hashes bytes."""
    def __init__(self, provider):
        self.provider = provider
        self.raw = None
        self.oid = None
        self.calls = []
        self.secret_paths = []
        self.pushes = 0
        self.outcome = "exact"
        self.tamper_native = False

    def __call__(self, argv, *, input_bytes, cwd, environment, output_limit):
        index = next(i for i, value in enumerate(argv) if value.startswith("--git-dir="))
        arguments = argv[index + 1:]
        self.calls.append((list(argv), bytes(input_bytes), dict(environment)))
        if SECRET in repr(argv) or SECRET in repr(environment):
            raise AssertionError("credential leaked through child arguments/environment")
        if any(key.startswith("GITHUB_") or key.startswith("GH_") for key in environment):
            raise AssertionError("ambient provider credentials reached Git")
        if arguments[0] in ("fetch", "push"):
            secret = Path(environment["CARGO_ALLOW_TAG_CREDENTIAL_FILE"])
            helper = Path(environment["GIT_ASKPASS"])
            if secret.read_bytes() != SECRET.encode() or stat.S_IMODE(secret.stat().st_mode) != 0o600:
                raise AssertionError("explicit credential file is not private")
            if stat.S_IMODE(helper.stat().st_mode) != 0o700 or stat.S_IMODE(cwd.stat().st_mode) != 0o700:
                raise AssertionError("isolated helper/worktree is not private")
            self.secret_paths.extend((secret, helper))
        if arguments[0] == "init":
            if arguments != ["init", "--bare", "--template="]:
                raise AssertionError("unexpected native initialization")
            return DRIVER.ChildResult(0, b"", b"")
        if arguments[0] == "fetch":
            expected = ["fetch", "--depth=1", "--filter=blob:none", "--no-tags", "--no-write-fetch-head",
                        "--no-recurse-submodules", "--no-auto-maintenance", "--", DRIVER.REMOTE, ANCHOR]
            if arguments != expected:
                raise AssertionError("fetch did not pin the frozen object")
            return DRIVER.ChildResult(0, b"", b"")
        if arguments == ["cat-file", "-p", ANCHOR]:
            return DRIVER.ChildResult(0, f"tree {TREE}\n\nfixture commit\n".encode(), b"")
        if arguments == ["hash-object", "-t", "tag", "-w", "--stdin"]:
            oracle = DRIVER.bounded_run([str(GIT), "hash-object", "-t", "tag", "--stdin"], input_bytes=input_bytes,
                                        cwd=cwd, environment=DRIVER._child_environment(), output_limit=1000)
            if oracle.returncode != 0:
                raise AssertionError("native Git hashing oracle failed")
            self.raw = bytes(input_bytes)
            self.oid = oracle.stdout.decode().strip()
            return oracle
        if arguments == ["cat-file", "-p", self.oid]:
            return DRIVER.ChildResult(0, b"tampered" if self.tamper_native else self.raw, b"")
        if arguments[0] == "push":
            expected = ["push", "--porcelain", "--no-verify", "--no-follow-tags", "--recurse-submodules=no",
                        "--", DRIVER.REMOTE, self.oid + ":" + DRIVER.TAG_REF]
            if arguments != expected or any("force" in value or value.startswith("+") for value in arguments):
                raise AssertionError("physical push boundary changed")
            self.pushes += 1
            if self.pushes != 1:
                raise AssertionError("physical push repeated")
            if self.outcome in ("exact", "lost_exact", "raise_exact"):
                self.provider.set_tag(self.raw)
            if self.outcome == "raise_exact":
                raise STORE.StoreError("uncertain", "intercepted lost process response")
            return DRIVER.ChildResult(0 if self.outcome in ("exact", "known_absent") else 1, b"", b"")
        raise AssertionError("unexpected native Git command")


class IoContracts(unittest.TestCase):
    def setUp(self):
        self.provider = DriverProvider()
        self.client = self.provider.client()

    def tearDown(self):
        self.assertEqual(self.provider.unexpected, [])
        self.assertEqual(self.provider.mutations(), [])

    def expect_error(self, callback, kind=None):
        with self.assertRaises(STORE.StoreError) as caught:
            callback()
        if kind is not None:
            self.assertEqual(caught.exception.kind, kind)
        self.assertNotIn(SECRET, str(caught.exception))

    def test_absent_ref_requires_two_authenticated_reads_and_provider_date(self):
        result = self.client.observe_tag()
        self.assertFalse(result.observation["ref_exists"])
        self.assertEqual(result.observed_at_unix_seconds, NOW)
        refs = [call for call in self.provider.calls if urlsplit(call[1]).path.endswith("/git/ref/tags/v0.2.0")]
        self.assertEqual(len(refs), 2)

    def test_exact_annotated_raw_object_commit_tree_and_ref_are_read(self):
        raw = raw_tag()
        self.provider.set_tag(raw)
        result = self.client.observe_tag(expected_raw=raw)
        self.assertEqual(dict(result.observation), {"provider_reachable": True, "ref_exists": True,
            "remote_is_annotated": True, "remote_object_id": STORE._git_oid("tag", raw),
            "remote_peeled_commit": ANCHOR, "remote_peeled_tree": TREE})
        paths = [urlsplit(call[1]).path for call in self.provider.calls]
        self.assertIn(BASE + "/git/tags/" + STORE._git_oid("tag", raw), paths)
        self.assertIn(BASE + "/git/trees/" + TREE, paths)

    def test_lightweight_ref_is_observed_without_claiming_annotation(self):
        self.provider.tag = {"ref": DRIVER.TAG_REF, "object": {"type": "commit", "sha": ANCHOR}}
        observed = self.client.observe_tag()
        self.assertTrue(observed.observation["ref_exists"])
        self.assertFalse(observed.observation["remote_is_annotated"])

    def test_owned_tag_bytes_cannot_be_replaced_by_same_target(self):
        self.provider.set_tag(raw_tag(message="Different immutable tag message.\n"))
        self.expect_error(lambda: self.client.observe_tag(expected_raw=raw_tag()), "conflict")

    def test_provider_object_metadata_must_reconstruct_its_exact_git_oid(self):
        self.provider.set_tag(raw_tag())
        self.provider.objects[self.provider.tag["object"]["sha"]]["message"] = "Moved bytes.\n"
        self.expect_error(self.client.observe_tag, "mismatch")

    def test_tag_moving_during_readback_fails_closed(self):
        self.provider.set_tag(raw_tag())
        count = 0
        def after(request, response):
            nonlocal count
            if urlsplit(request[1]).path.endswith("/git/ref/tags/v0.2.0"):
                count += 1
                if count == 1:
                    self.provider.tag = None
            return response
        self.provider.after = after
        self.expect_error(self.client.observe_tag, "conflict")

    def test_missing_malformed_and_unmeasured_provider_times_fail_closed(self):
        self.provider.date_header = False
        self.expect_error(self.client.observe_tag, "instrument_failure")
        for value, expected in [("not-a-date", "instrument_failure"),
                                (formatdate(NOW - 1, usegmt=True), "stale"),
                                (formatdate(NOW + 1, usegmt=True), "stale")]:
            with self.subTest(value=value):
                self.provider.after = lambda _request, result: STORE.HttpResponse(result.status, {"Date": value}, result.body)
                self.expect_error(self.client.observe_tag, expected)

    def test_bad_clock_is_a_sanitized_instrument_failure(self):
        for value in (float("nan"), float("inf"), True, 0, -1):
            with self.subTest(value=str(value)):
                self.client._clock = lambda: value
                self.expect_error(self.client.observe_tag, "instrument_failure")

    def test_isolated_exact_git_request_uses_native_oid_and_pushes_once(self):
        runner = GitRunner(self.provider)
        with mock.patch.dict(os.environ, {"GIT_CONFIG_COUNT": "99", "GITHUB_TOKEN": "ambient-must-not-propagate"}):
            with DRIVER.GitTagRequest(GIT, lambda: SECRET, runner=runner) as request:
                request.prepare(raw_tag(), STORE._git_oid("tag", raw_tag()), ANCHOR, TREE)
                self.assertTrue(request.push_once())
                self.expect_error(request.push_once, "already_used")
        self.assertEqual(runner.pushes, 1)
        self.assertTrue(all(not path.exists() for path in runner.secret_paths))
        self.assertTrue(self.client.observe_tag(expected_raw=raw_tag()).observation["remote_is_annotated"])

    def test_unknown_physical_response_still_consumes_the_single_call(self):
        runner = GitRunner(self.provider)
        runner.outcome = "raise_exact"
        with DRIVER.GitTagRequest(GIT, lambda: SECRET, runner=runner) as request:
            request.prepare(raw_tag(), STORE._git_oid("tag", raw_tag()), ANCHOR, TREE)
            self.expect_error(request.push_once, "uncertain")
            self.expect_error(request.push_once, "already_used")
        self.assertEqual(runner.pushes, 1)
        self.assertTrue(all(not path.exists() for path in runner.secret_paths))

    def test_copy_pickle_and_other_thread_cannot_duplicate_physical_request(self):
        runner = GitRunner(self.provider)
        with DRIVER.GitTagRequest(GIT, lambda: SECRET, runner=runner) as request:
            request.prepare(raw_tag(), STORE._git_oid("tag", raw_tag()), ANCHOR, TREE)
            for callback in (lambda: copy.copy(request), lambda: copy.deepcopy(request), lambda: pickle.dumps(request)):
                self.expect_error(callback, "already_used")
            outcomes = []
            def foreign():
                try:
                    request.push_once()
                except STORE.StoreError as error:
                    outcomes.append(error.kind)
            thread = threading.Thread(target=foreign)
            thread.start()
            thread.join(timeout=5)
            self.assertEqual(outcomes, ["already_used"])
            self.assertTrue(request.push_once())

    def test_native_object_readback_tampering_prevents_push(self):
        runner = GitRunner(self.provider)
        runner.tamper_native = True
        with DRIVER.GitTagRequest(GIT, lambda: SECRET, runner=runner) as request:
            self.expect_error(lambda: request.prepare(raw_tag(), STORE._git_oid("tag", raw_tag()), ANCHOR, TREE), "mismatch")
            self.expect_error(request.push_once, "already_used")
        self.assertEqual(runner.pushes, 0)

    def test_forked_process_cannot_inherit_the_physical_request(self):
        runner = GitRunner(self.provider)
        with DRIVER.GitTagRequest(GIT, lambda: SECRET, runner=runner) as request:
            request.prepare(raw_tag(), STORE._git_oid("tag", raw_tag()), ANCHOR, TREE)
            read_fd, write_fd = os.pipe()
            pid = os.fork()
            if pid == 0:
                os.close(read_fd)
                try:
                    request.push_once()
                    os.write(write_fd, b"unexpected-push")
                except STORE.StoreError as error:
                    os.write(write_fd, error.kind.encode())
                finally:
                    os.close(write_fd)
                    os._exit(0)
            os.close(write_fd)
            with os.fdopen(read_fd, "rb") as handle:
                outcome = handle.read(100)
            _, status = os.waitpid(pid, 0)
            self.assertEqual(status, 0)
            self.assertEqual(outcome, b"already_used")
            self.assertTrue(request.push_once())
        self.assertEqual(runner.pushes, 1)

    def test_bounded_child_reads_real_pipes_and_does_not_echo_stderr(self):
        with tempfile.TemporaryDirectory(prefix="final-tag-child-") as directory:
            result = DRIVER.bounded_run([sys.executable, "-c", "import sys;sys.stdout.buffer.write(sys.stdin.buffer.read());sys.stderr.write('private')"],
                input_bytes=b"exact child bytes", cwd=Path(directory), environment=DRIVER._child_environment(), output_limit=1000)
            self.assertEqual(result.stdout, b"exact child bytes")
            self.assertNotIn("private", repr(result))
            self.expect_error(lambda: DRIVER.bounded_run([sys.executable, "-c", "print('x'*10000)"],
                cwd=Path(directory), environment=DRIVER._child_environment(), output_limit=100), "instrument_failure")
            self.expect_error(lambda: DRIVER.bounded_run([sys.executable, "-c", "import time;time.sleep(1)"],
                cwd=Path(directory), environment=DRIVER._child_environment(), timeout=0.05), "uncertain")

    def test_no_follow_input_and_exact_file_creation_controls(self):
        with tempfile.TemporaryDirectory(prefix="final-tag-files-") as directory:
            root = Path(directory)
            DRIVER._write_exact(root / "selected", b"exact")
            (root / "linked").symlink_to(root / "selected")
            self.assertEqual(DRIVER._regular_bytes(root / "selected", 5), b"exact")
            self.expect_error(lambda: DRIVER._regular_bytes(root / "linked", 5), "instrument_failure")
            self.expect_error(lambda: DRIVER._regular_bytes(root / "selected", 4), "invalid_input")
            self.expect_error(lambda: DRIVER._write_exact(root / "selected", b"overwritten"), "instrument_failure")

    def test_actual_bridge_digest_and_refusal_boundary_has_no_eligibility_fallback(self):
        with tempfile.TemporaryDirectory(prefix="final-tag-bridge-") as directory:
            executable = Path(directory) / "bridge"
            executable.write_bytes(b"selected executable bytes")
            bridge = DRIVER.RustBridge(executable, STORE.sha256(executable.read_bytes()),
                runner=lambda *_args, **_kwargs: DRIVER.ChildResult(2, b"", SECRET.encode()))
            self.expect_error(lambda: bridge.call({}), "ineligible")
            executable.write_bytes(b"moved executable bytes")
            self.expect_error(lambda: bridge.call({}), "mismatch")

    def test_explicit_credential_descriptor_is_bounded_and_has_no_ambient_fallback(self):
        with tempfile.TemporaryFile() as handle:
            handle.write(SECRET.encode())
            handle.seek(0)
            getter = DRIVER._credential_fd(handle.fileno())
            self.assertEqual(getter(), SECRET)
        self.expect_error(lambda: DRIVER._credential_fd(0), "invalid_input")
        with tempfile.TemporaryFile() as handle:
            handle.write(b"x" * 4097)
            handle.seek(0)
            self.expect_error(lambda: DRIVER._credential_fd(handle.fileno()), "invalid_input")
        read_fd, write_fd = os.pipe()
        try:
            self.expect_error(lambda: DRIVER._credential_fd(read_fd, timeout=0.02), "instrument_failure")
        finally:
            os.close(read_fd)
            os.close(write_fd)


class FullContracts(unittest.TestCase):
    """Every semantic result comes from the actual compiled Rust consumer."""
    def setUp(self):
        self.provider = DriverProvider()
        self.provider.now = FULL_FIXTURE["now_unix_seconds"]
        self.selected = DRIVER._byte_map(FULL_FIXTURE["selected"])
        self.configuration = copy.deepcopy(FULL_FIXTURE["configuration"])
        source = bytes(FULL_FIXTURE["source_bytes"])
        self.provider.source_body = source
        self.provider.source["body"] = source.decode()
        self.input_id = self.provider.retain(self.selected, "selected-existing-typed-final-tag-inputs", artifact_id=505)
        self.configuration["artifacts"] = [{"artifact_id": self.input_id,
            "transfer": self.provider.transfer(self.input_id), "producer": producer(),
            "selections": {path: path for path in self.selected}}]
        self.client = self.provider.client()
        self.git_runner = GitRunner(self.provider)
        self.last_bridge_stderr = b""
        def compiled_child(*args, **kwargs):
            result = DRIVER.bounded_run(*args, **kwargs)
            self.last_bridge_stderr = result.stderr
            return result
        self.bridge = DRIVER.RustBridge(FULL_BRIDGE, STORE.sha256(FULL_BRIDGE.read_bytes()), runner=compiled_child)
        self.driver = DRIVER.FinalTagDriver(self.configuration, self.client, self.bridge, lambda: SECRET,
            lambda: DRIVER.GitTagRequest(GIT, lambda: SECRET, runner=self.git_runner), clock=lambda: self.provider.now)
        self.temporary = tempfile.TemporaryDirectory(prefix="final-tag-intercepted-")
        self.root = Path(self.temporary.name)
        self.counter = 0
        self.subject = None

    def tearDown(self):
        self.temporary.cleanup()
        self.assertEqual(self.provider.unexpected, [])
        self.assertTrue(all(not path.exists() for path in self.git_runner.secret_paths))
        for raw in self.provider.blobs.values():
            self.assertNotIn(SECRET.encode(), raw)

    def call(self, callback):
        try:
            return callback()
        except STORE.StoreError as error:
            # The actual Rust bridge emits sanitized static artifact errors.
            # This diagnostic is test-only and contains no provider credential.
            self.assertNotIn(SECRET.encode(), self.last_bridge_stderr)
            self.fail(str(error) + ": " + self.last_bridge_stderr.decode(errors="replace"))

    def prepare_upload(self, phase):
        self.provider.now += 1
        self.counter += 1
        directory = self.root / f"phase-{self.counter}-{phase}"
        prepared = self.call(lambda: self.driver.prepare(phase, directory))
        self.subject = prepared["subject_digest"]
        self.assertFalse(prepared["gate_open"])
        files = {path.name: path.read_bytes() for path in (directory / "checkpoint").iterdir()}
        self.assertNotIn("plan.json", files)
        artifact_id = self.provider.retain(files, prepared["artifact_name"])
        self.provider.now += 1
        return directory, artifact_id

    def phase(self, phase):
        directory, artifact_id = self.prepare_upload(phase)
        return self.call(lambda: self.driver.finalize(directory, artifact_id))

    def through(self, last):
        result = None
        for phase in DRIVER.PHASES:
            result = self.phase(phase)
            if phase == last:
                return result
        self.fail("unknown fixture phase")

    def state(self):
        snapshot = self.client.read(self.subject)
        return snapshot, json.loads(snapshot.files["state.json"])

    def expect_error(self, callback, kind=None):
        with self.assertRaises(STORE.StoreError) as caught:
            callback()
        if kind is not None:
            self.assertEqual(caught.exception.kind, kind)
        self.assertNotIn(SECRET, str(caught.exception))

    def test_complete_actual_typed_lifecycle_retains_acyclic_heads_before_one_push(self):
        started = self.through("started")
        self.assertFalse(started["gate_open"])
        self.assertEqual(self.git_runner.pushes, 1)
        _, state = self.state()
        self.assertEqual(len(state["events"]), 5)
        self.assertEqual(state["authorization"]["state"], "irreversible_operation_started")
        self.assertEqual(state["lease"]["state"], "held_irreversible")
        self.assertEqual(state["tag"]["state"], "push_response_observed")
        for transfer in state["checkpoints"]:
            files = self.provider.artifacts[int(transfer["stable_artifact_id"])][1]
            self.assertNotIn("state.json", files)
            self.assertNotIn("transfer.json", files)
            self.assertEqual(transfer["semantic_payload_digest"], STORE.sha256(files["head.json"]))
        self.expect_error(self.driver.continuation, "ineligible")
        result = self.phase("observation")
        self.assertTrue(result["gate_open"])
        self.assertEqual(result["operation_head"]["sequence"], 6)
        _, exact = self.state()
        self.assertEqual(exact["events"][-1]["timestamp_source"], "provider_metadata")
        self.assertEqual(exact["events"][-1]["observed_at_unix_seconds"], self.provider.now - 1)
        self.assertEqual(exact["lease"]["journal_head_digest"], STORE.sha256(self.provider.artifacts[int(exact["checkpoints"][-1]["stable_artifact_id"])][1]["head.json"]))
        self.assertEqual(len(exact["authorization"]["consumed_nonces"]), 1)
        self.assertEqual(self.git_runner.pushes, 1)
        self.assertTrue(self.call(self.driver.continuation)["gate_open"])

    def test_provider_lost_exact_response_reconciles_without_a_second_push(self):
        self.git_runner.outcome = "lost_exact"
        started = self.through("started")
        self.assertFalse(started["response_observed"])
        _, state = self.state()
        self.assertEqual(state["tag"]["state"], "push_response_unknown")
        mutations = len(self.provider.mutations())
        observed = self.driver.reconcile(self.subject)
        self.assertFalse(observed["gate_open"])
        self.assertTrue(observed["remote_observation"]["ref_exists"])
        self.assertEqual(len(self.provider.mutations()), mutations)
        self.assertTrue(self.phase("observation")["gate_open"])
        self.assertEqual(self.git_runner.pushes, 1)

    def test_absent_after_unknown_or_success_response_never_opens_retry(self):
        # The process reports success but independent observation remains
        # absent. A successful response cannot stand in for remote proof.
        self.git_runner.outcome = "known_absent"
        self.through("started")
        mutations = len(self.provider.mutations())
        observed = self.driver.reconcile(self.subject)
        self.assertFalse(observed["gate_open"])
        self.assertFalse(observed["remote_observation"]["ref_exists"])
        self.expect_error(lambda: self.driver.prepare("observation", self.root / "absent"), "ineligible")
        self.expect_error(lambda: self.driver.prepare("started", self.root / "retry"), "ineligible")
        self.assertEqual(len(self.provider.mutations()), mutations)
        self.assertEqual(self.git_runner.pushes, 1)

    def test_unknown_absent_response_remains_unresolved_with_no_second_push(self):
        self.git_runner.outcome = "lost_absent"
        self.through("started")
        _, state = self.state()
        self.assertEqual(state["tag"]["state"], "push_response_unknown")
        mutations = len(self.provider.mutations())
        self.assertFalse(self.driver.reconcile(self.subject)["remote_observation"]["ref_exists"])
        self.expect_error(lambda: self.driver.prepare("started", self.root / "duplicate"), "ineligible")
        self.expect_error(lambda: self.driver.prepare("observation", self.root / "absent"), "ineligible")
        self.assertEqual(len(self.provider.mutations()), mutations)
        self.assertEqual(self.git_runner.pushes, 1)

    def test_ambiguous_started_ref_write_retains_started_without_any_push_permit(self):
        self.through("intent")
        directory, artifact_id = self.prepare_upload("started")
        fired = False
        def after(request, response):
            nonlocal fired
            if request[0] == "PATCH" and "/git/refs/" in urlsplit(request[1]).path and not fired:
                fired = True
                raise STORE.StoreError("uncertain", "intercepted lost atomic-ref response")
            return response
        self.provider.after = after
        self.expect_error(lambda: self.driver.finalize(directory, artifact_id), "uncertain")
        self.provider.after = None
        self.assertTrue(fired)
        self.assertEqual(self.git_runner.pushes, 0)
        _, state = self.state()
        self.assertEqual(state["tag"]["state"], "push_started")
        self.assertEqual(len(state["events"]), 5)
        mutations = len(self.provider.mutations())
        self.expect_error(lambda: self.driver.finalize(directory, artifact_id), "conflict")
        self.expect_error(lambda: self.driver.prepare("started", self.root / "restart"), "ineligible")
        self.assertFalse(self.driver.reconcile(self.subject)["gate_open"])
        self.assertEqual(len(self.provider.mutations()), mutations)
        self.assertEqual(self.git_runner.pushes, 0)

    def test_expired_started_reconciliation_is_read_only_and_cannot_continue(self):
        self.git_runner.outcome = "lost_exact"
        self.through("started")
        self.provider.now = self.configuration["expires_at_unix_seconds"] + 1
        mutations = len(self.provider.mutations())
        self.assertFalse(self.driver.reconcile(self.subject)["gate_open"])
        self.expect_error(self.driver.continuation, "ineligible")
        self.assertEqual(len(self.provider.mutations()), mutations)
        self.assertEqual(self.git_runner.pushes, 1)

    def test_response_retention_failure_uses_existing_unknown_reducer_before_exact_gate(self):
        self.through("intent")
        directory, artifact_id = self.prepare_upload("started")
        patches = 0
        def before(request):
            nonlocal patches
            if request[0] == "PATCH" and "/git/refs/" in urlsplit(request[1]).path:
                patches += 1
                if patches == 2:
                    raise STORE.StoreError("uncertain", "intercepted response-retention outage")
            return None
        self.provider.before = before
        self.expect_error(lambda: self.driver.finalize(directory, artifact_id), "uncertain")
        self.provider.before = None
        _, state = self.state()
        self.assertEqual(state["tag"]["state"], "push_started")
        self.assertEqual(self.git_runner.pushes, 1)
        self.assertFalse(self.driver.reconcile(self.subject)["gate_open"])
        self.assertTrue(self.phase("observation")["gate_open"])
        _, state = self.state()
        self.assertEqual(state["tag"]["transitions"][2]["to"], "push_response_unknown")
        self.assertEqual(state["tag"]["transitions"][3]["to"], "remote_observed_exact")
        self.assertEqual(self.git_runner.pushes, 1)

    def test_no_durable_append_before_exact_upload_and_local_plan_readback(self):
        directory, artifact_id = self.prepare_upload("bootstrap")
        plan = json.loads((directory / "plan.json").read_bytes())
        self.assertEqual(self.provider.mutations(), [])
        self.assertIsNone(plan["previous_control_commit"])
        original = (directory / "checkpoint" / "head.json").read_bytes()
        (directory / "checkpoint" / "head.json").write_bytes(original + b" ")
        self.expect_error(lambda: self.driver.finalize(directory, artifact_id), "mismatch")
        (directory / "checkpoint" / "head.json").write_bytes(original)
        metadata, files, _ = self.provider.artifacts[artifact_id]
        files = dict(files)
        files["head.json"] += b" "
        self.provider.artifacts[artifact_id] = (metadata, files, PROVIDER.archive(files))
        self.expect_error(lambda: self.driver.finalize(directory, artifact_id), "mismatch")
        self.assertEqual(self.provider.mutations(), [])
        self.assertEqual(self.git_runner.pushes, 0)

    def test_independent_subject_context_complete_replay_and_actor_controls(self):
        original = copy.deepcopy(self.configuration["artifacts"])
        original_files = dict(self.selected)
        changes = [
            ("expected-context.json", lambda record: record["freeze"].update(tree="c" * 40)),
            ("freeze-replay.json", lambda record: record.update(selected_upload_rows=9)),
            ("freeze-replay.json", lambda record: record.update(retained_bytes_verified=False)),
            ("freeze-replay.json", lambda record: record["observation_readings"][0].update(freshness="stale")),
            ("preflight-inputs.json", lambda record: record["candidate"].update(repository_tree="c" * 40)),
            ("authorization-custody.json", lambda record: record["freeze"].update(tree="c" * 40)),
            ("authorization.json", lambda record: record["authority"].update(nonce="foreign-nonce")),
        ]
        for index, (path, mutate) in enumerate(changes):
            with self.subTest(path=path, index=index):
                files = dict(original_files)
                record = json.loads(files[path])
                mutate(record)
                files[path] = json.dumps(record, separators=(",", ":"), ensure_ascii=False).encode()
                if path == "freeze-replay.json":
                    # Keep the byte-digest chain self-consistent so only the
                    # actual owner's replay, not an old mint hash, catches a
                    # forged Complete field/count/freshness result.
                    birth = json.loads(files["authorization-custody.json"])
                    birth["mint"]["replay_digest"] = STORE.sha256(files[path])
                    files["authorization-custody.json"] = json.dumps(birth, separators=(",", ":"), ensure_ascii=False).encode()
                artifact_id = self.provider.retain(files, f"synthetic-negative-inputs-{index}")
                self.driver.config["artifacts"] = [{"artifact_id": artifact_id,
                    "transfer": self.provider.transfer(artifact_id), "producer": producer(),
                    "selections": {name: name for name in files}}]
                self.expect_error(lambda: self.driver.prepare("bootstrap", self.root / f"wrong-{index}"), "ineligible")
        self.driver.config["artifacts"] = original
        self.provider.source["user"]["id"] = 405
        self.expect_error(lambda: self.driver.prepare("bootstrap", self.root / "actor"), "mismatch")
        self.assertEqual(self.provider.mutations(), [])
        self.assertEqual(self.git_runner.pushes, 0)

    def test_missing_or_oversized_frozen_asset_cannot_bootstrap(self):
        aliases = self.driver.config["artifacts"][0]["selections"]
        asset = next(name for name in aliases if name.endswith(".tar.gz"))
        removed = aliases.pop(asset)
        self.expect_error(lambda: self.driver.prepare("bootstrap", self.root / "missing"), "ineligible")
        aliases[asset] = removed
        transfer = self.driver.config["artifacts"][0]["transfer"]
        member = next(item for item in transfer["files"] if item["path"] == asset)
        member["size_bytes"] = 2 * 1024 * 1024 + 1
        self.expect_error(lambda: self.driver.prepare("bootstrap", self.root / "oversized"))
        self.assertEqual(self.provider.mutations(), [])
        self.assertEqual(self.git_runner.pushes, 0)


def main():
    global FULL_BRIDGE, FULL_FIXTURE
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--io-only", action="store_true")
    parser.add_argument("--bridge", type=Path)
    parser.add_argument("--fixture", type=Path)
    arguments = parser.parse_args()
    if arguments.io_only and (arguments.bridge or arguments.fixture):
        parser.error("choose either the explicit I/O-only scope or an actual compiled bridge and fixture")
    if not arguments.io_only and (arguments.bridge is None or arguments.fixture is None):
        parser.error("full lifecycle is not run without --bridge and --fixture; --io-only is an explicit narrower scope")
    if not GIT.is_file():
        parser.error("native Git hashing oracle is required")
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(IoContracts)
    if not arguments.io_only:
        FULL_BRIDGE = arguments.bridge.resolve()
        FULL_FIXTURE = json.loads(arguments.fixture.read_bytes())
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(FullContracts))
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    print("Scope: intercepted production I/O only; native typed lifecycle " + ("not run" if arguments.io_only else "included"))
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
