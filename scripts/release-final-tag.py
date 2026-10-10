#!/usr/bin/env python3
"""Production final-tag I/O driver; the Rust bridge owns release semantics.

Only explicit configuration, selected artifact envelopes and an explicit
credential callback/fd are accepted. There is no ambient credential fallback.
prepare writes checkpoint files; finalize consumes the separately assigned
artifact ID. The single physical push is inside a fresh append witness in the
same process. Restart and reconciliation cannot reconstruct that witness.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass, field
from datetime import datetime, timezone
from email.utils import parsedate_to_datetime
import hashlib
import json
import math
import os
from pathlib import Path
import re
import select
import selectors
import signal
import stat
import subprocess
import sys
import tempfile
import threading
import time
from typing import Any, Callable, Mapping

from release_operation_store import (
    API, REPOSITORY, GitHubReleaseStore, HttpResponse, StoreError,
    _git_oid, _integer, _json, _json_bytes, _object, _require, _sha, _utc, sha256,
)

REMOTE = "https://github.com/EffortlessMetrics/cargo-allow.git"
TAG_REF = "refs/tags/v0.2.0"
MAX_BRIDGE = 64 * 1024 * 1024
MAX_PLAN = 8 * 1024 * 1024
MAX_TAG = 32768
PHASES = ("bootstrap", "lease", "intent", "started", "observation")
_CONTROL_STATE = "state.json"


def _fail(kind: str, detail: str) -> None:
    raise StoreError(kind, detail)


def _date(seconds: int) -> str:
    try:
        return datetime.fromtimestamp(seconds, timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    except (ValueError, OverflowError, OSError):
        _fail("invalid_input", "timestamp is outside the supported range")


def _secret_free(raw: bytes, credential: Callable[[], str]) -> None:
    # The credential is never sent to Rust, retained, logged or placed in argv.
    value = credential()
    _require(isinstance(value, str) and 1 <= len(value) <= 4096
             and all(33 <= ord(c) <= 126 for c in value),
             "invalid_input", "explicit credential callback returned an invalid value")
    _require(value.encode("ascii") not in raw, "instrument_failure",
             "credential material appeared in noncredential transport data")


@dataclass(frozen=True, repr=False)
class ChildResult:
    returncode: int
    stdout: bytes = field(repr=False)
    stderr: bytes = field(repr=False)


def bounded_run(argv: list[str], *, input_bytes: bytes = b"", cwd: Path,
                environment: Mapping[str, str], timeout: float = 60,
                output_limit: int = MAX_BRIDGE) -> ChildResult:
    """One POSIX child, bounded stdin/stdout/stderr and an entire-process deadline."""
    _require(os.name == "posix" and isinstance(argv, list) and argv
             and 0 < timeout <= 60 and 0 < output_limit <= MAX_BRIDGE,
             "invalid_input", "unsupported bounded child invocation")
    process = None
    streams = selectors.DefaultSelector()
    output: dict[str, bytearray] = {"stdout": bytearray(), "stderr": bytearray()}
    sent = 0
    deadline = time.monotonic() + timeout
    try:
        process = subprocess.Popen(argv, cwd=cwd, env=dict(environment), stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   close_fds=True, start_new_session=True)
        for label, pipe in [("stdout", process.stdout), ("stderr", process.stderr)]:
            _require(pipe is not None, "instrument_failure", "child pipe unavailable")
            os.set_blocking(pipe.fileno(), False)
            streams.register(pipe, selectors.EVENT_READ, label)
        _require(process.stdin is not None, "instrument_failure", "child stdin unavailable")
        if input_bytes:
            os.set_blocking(process.stdin.fileno(), False)
            streams.register(process.stdin, selectors.EVENT_WRITE, "stdin")
        else:
            process.stdin.close()
        while streams.get_map():
            remaining = deadline - time.monotonic()
            _require(remaining > 0, "uncertain", "child exceeded its deadline")
            for key, _ in streams.select(min(remaining, 0.25)):
                pipe, label = key.fileobj, key.data
                if label == "stdin":
                    try:
                        sent += os.write(pipe.fileno(), input_bytes[sent:sent + 65536])
                    except BrokenPipeError:
                        sent = len(input_bytes)
                    if sent == len(input_bytes):
                        streams.unregister(pipe)
                        pipe.close()
                else:
                    chunk = os.read(pipe.fileno(), 65536)
                    if chunk:
                        output[label].extend(chunk)
                        _require(sum(map(len, output.values())) <= output_limit,
                                 "instrument_failure", "child output exceeded its bound")
                    else:
                        streams.unregister(pipe)
                        pipe.close()
        remaining = deadline - time.monotonic()
        _require(remaining > 0, "uncertain", "child exceeded its deadline")
        status = process.wait(timeout=remaining)
        return ChildResult(status, bytes(output["stdout"]), bytes(output["stderr"]))
    except StoreError:
        raise
    except (OSError, ValueError, subprocess.SubprocessError):
        _fail("uncertain", "bounded child could not complete")
    finally:
        streams.close()
        if process is not None:
            if process.poll() is None:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait()
            for pipe in (process.stdin, process.stdout, process.stderr):
                if pipe is not None and not pipe.closed:
                    pipe.close()


def _child_environment() -> dict[str, str]:
    return {"PATH": os.defpath, "LC_ALL": "C", "LANG": "C",
            "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_SYSTEM": os.devnull,
            "GIT_CONFIG_GLOBAL": os.devnull, "GIT_TERMINAL_PROMPT": "0",
            "GCM_INTERACTIVE": "Never", "GIT_OPTIONAL_LOCKS": "0"}


def _regular_bytes(path: Path, limit: int) -> bytes:
    try:
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(fd, "rb") as handle:
            metadata = os.fstat(handle.fileno())
            _require(stat.S_ISREG(metadata.st_mode) and metadata.st_size <= limit,
                     "invalid_input", "selected input is not a bounded regular file")
            raw = handle.read(limit + 1)
        _require(len(raw) <= limit, "invalid_input", "selected file grew beyond its bound")
        return raw
    except StoreError:
        raise
    except OSError:
        _fail("instrument_failure", "selected regular file is unavailable")


class RustBridge:
    """The actual compiled CLI, never a Python eligibility substitute."""
    def __init__(self, executable: Path, expected_sha256: str, *, runner=bounded_run):
        _require(executable.is_absolute() and bool(re.fullmatch(r"sha256:[0-9a-f]{64}", expected_sha256)),
                 "invalid_input", "exact compiled bridge path and digest required")
        self._executable, self._digest, self._runner = executable, expected_sha256, runner

    def _verify(self) -> None:
        try:
            fd = os.open(self._executable, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
            with os.fdopen(fd, "rb") as handle:
                metadata = os.fstat(handle.fileno())
                _require(stat.S_ISREG(metadata.st_mode) and metadata.st_size <= 512 * 1024 * 1024,
                         "invalid_input", "bridge is not a bounded regular executable")
                digest = hashlib.sha256()
                while chunk := handle.read(1024 * 1024):
                    digest.update(chunk)
            _require("sha256:" + digest.hexdigest() == self._digest,
                     "mismatch", "compiled bridge bytes changed")
        except StoreError:
            raise
        except OSError:
            _fail("instrument_failure", "compiled bridge is unavailable")

    def call(self, request: Mapping[str, Any]) -> dict[str, Any]:
        raw = _json_bytes(request)
        _require(len(raw) <= MAX_BRIDGE, "invalid_input", "bridge input exceeds its bound")
        self._verify()
        result = self._runner([str(self._executable), "--color", "never", "release-final-tag-bridge"],
                              input_bytes=raw, cwd=self._executable.parent,
                              environment=_child_environment(), output_limit=MAX_BRIDGE)
        self._verify()
        _require(result.returncode == 0, "ineligible", "typed final-tag bridge refused the current inputs")
        response = _object(result.stdout)
        _require(set(response) == {"operation_identity", "operation_digest", "subject_digest", "request_boundary",
                                   "valid_until", "files", "prepared", "gate_open", "push_object_id"}
                 and type(response.get("gate_open")) is bool,
                 "instrument_failure", "typed bridge response shape differs")
        return response


def _byte_map(value: Any) -> dict[str, bytes]:
    _require(isinstance(value, dict) and len(value) <= 64, "instrument_failure", "bounded byte map required")
    result: dict[str, bytes] = {}
    for path, raw in value.items():
        _require(isinstance(path, str) and isinstance(raw, list) and len(raw) <= 2 * 1024 * 1024
                 and all(type(byte) is int and 0 <= byte <= 255 for byte in raw),
                 "instrument_failure", "invalid byte map member")
        result[path] = bytes(raw)
    _require(sum(map(len, result.values())) <= 8 * 1024 * 1024,
             "instrument_failure", "byte map exceeds the provider boundary")
    return result


def _transport_files(files: Mapping[str, bytes]) -> dict[str, list[int]]:
    return {path: list(raw) for path, raw in files.items()}


@dataclass(frozen=True)
class TagReadback:
    observation: Mapping[str, Any]
    observed_at_unix_seconds: int


def _provider_time(response: HttpResponse, before: int, after: int) -> int:
    value = response.headers.get("date")
    _require(isinstance(value, str) and len(value) <= 80,
             "instrument_failure", "provider Date header is missing")
    try:
        parsed = parsedate_to_datetime(value)
        _require(parsed.tzinfo is not None, "instrument_failure", "provider Date lacks timezone")
        timestamp = int(parsed.timestamp())
    except (TypeError, ValueError, OverflowError):
        _fail("instrument_failure", "provider Date header is malformed")
    _require(before <= timestamp <= after and after - before <= 60,
             "stale", "provider observation is outside the measured read window")
    return timestamp


def tag_object_bytes(commit: str, name: str, email: str, at: int, message: str) -> bytes:
    _sha(commit)
    _require(isinstance(name, str) and bool(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9 ._-]{0,99}", name))
             and isinstance(email, str) and bool(re.fullmatch(r"[A-Za-z0-9._+-]+@[A-Za-z0-9.-]+", email))
             and len(email) <= 200 and _integer(at)
             and isinstance(message, str) and 0 < len(message) <= 8192
             and message.endswith("\n") and message.strip()
             and not any(c in message for c in ("\0", "\r")),
             "invalid_input", "bounded annotated tag identity required")
    raw = (f"object {commit}\ntype commit\ntag v0.2.0\ntagger {name} <{email}> {at} +0000\n\n{message}").encode("utf-8")
    _require(len(raw) <= MAX_TAG, "invalid_input", "annotated object exceeds its bound")
    return raw


class TagStore(GitHubReleaseStore):
    """Read-only tag observation through the reviewed authenticated HTTP seam."""
    def _tag_ref(self) -> tuple[HttpResponse, dict[str, Any] | None]:
        response, record = self._api("GET", "/git/ref/tags/v0.2.0", statuses=(200, 404))
        if response.status == 404:
            return response, None
        _require(record.get("ref") == TAG_REF and isinstance(record.get("object"), dict)
                 and record["object"].get("type") in ("commit", "tag"),
                 "mismatch", "remote tag reference shape differs")
        _sha(record["object"].get("sha"))
        return response, record

    def observe_tag(self, *, expected_raw: bytes | None = None) -> TagReadback:
        self._repository()
        before = _now(self._clock)
        _, first = self._tag_ref()
        observation = {"provider_reachable": True, "ref_exists": first is not None,
                       "remote_is_annotated": False, "remote_object_id": "",
                       "remote_peeled_commit": "", "remote_peeled_tree": ""}
        if first is not None:
            oid = first["object"]["sha"]
            observation["remote_object_id"] = oid
            kind = first["object"]["type"]
            if kind == "tag":
                _, tagged = self._api("GET", "/git/tags/" + oid)
                obj, tagger = tagged.get("object"), tagged.get("tagger")
                _require(tagged.get("sha") == oid and tagged.get("tag") == "v0.2.0"
                         and isinstance(obj, dict) and obj.get("type") == "commit"
                         and isinstance(tagger, dict) and isinstance(tagged.get("message"), str),
                         "mismatch", "annotated tag object differs")
                commit = _sha(obj.get("sha"))
                raw = tag_object_bytes(commit, tagger.get("name"), tagger.get("email"),
                                       _utc(tagger.get("date")), tagged["message"])
                _require(_git_oid("tag", raw) == oid, "mismatch", "remote annotated object bytes differ")
                if expected_raw is not None:
                    _require(raw == expected_raw, "conflict", "remote annotated object differs from the owned request")
                observation["remote_is_annotated"] = True
            else:
                commit = oid
            _, committed = self._api("GET", "/git/commits/" + commit)
            _require(committed.get("sha") == commit and isinstance(committed.get("tree"), dict),
                     "mismatch", "peeled commit identity differs")
            tree = _sha(committed["tree"].get("sha"))
            _, tree_record = self._api("GET", "/git/trees/" + tree)
            _require(tree_record.get("sha") == tree and tree_record.get("truncated") is False,
                     "mismatch", "peeled tree identity is unavailable or truncated")
            observation["remote_peeled_commit"], observation["remote_peeled_tree"] = commit, tree
        final_response, final = self._tag_ref()
        _require(final == first, "conflict", "remote tag moved during independent readback")
        after = _now(self._clock)
        observed_at = _provider_time(final_response, before, after)
        return TagReadback(observation, observed_at)


class GitTagRequest:
    """Isolated local object store, one fixed nonforced OID-to-tag request."""
    def __init__(self, git: Path, credential: Callable[[], str], *, runner=bounded_run):
        _require(git.is_absolute(), "invalid_input", "explicit absolute Git executable required")
        self._git, self._credential, self._runner = git, credential, runner
        self._temporary = None
        self._root = None
        self._oid = None
        self._used = False
        self._pid = os.getpid()
        self._thread = threading.get_ident()
        self._lock = threading.Lock()

    def __copy__(self):
        _fail("already_used", "physical request cannot be copied")

    def __deepcopy__(self, _memo):
        _fail("already_used", "physical request cannot be copied")

    def __reduce__(self):
        _fail("already_used", "physical request cannot be serialized")

    def _owner(self) -> None:
        _require(os.getpid() == self._pid and threading.get_ident() == self._thread,
                 "already_used", "physical request belongs to its creating process and thread")

    def __enter__(self):
        self._owner()
        _require(self._temporary is None, "already_used", "isolated request cannot be reopened")
        self._temporary = tempfile.TemporaryDirectory(prefix="cargo-allow-final-tag-")
        self._root = Path(self._temporary.name)
        os.chmod(self._root, 0o700)
        return self

    def __exit__(self, *_):
        self._owner()
        if self._temporary is not None:
            self._temporary.cleanup()
        self._root = None
        self._oid = None

    def _run(self, arguments: list[str], raw: bytes = b"", *, network: bool = False) -> ChildResult:
        self._owner()
        _require(self._root is not None, "already_used", "isolated Git request is closed")
        environment = _child_environment()
        secret_file = self._root / "explicit-credential"
        helper = self._root / "askpass"
        if network:
            credential = self._credential()
            _require(isinstance(credential, str) and 1 <= len(credential) <= 4096
                     and all(33 <= ord(c) <= 126 for c in credential),
                     "invalid_input", "explicit Git credential is malformed")
            _write_exact(secret_file, credential.encode("ascii"), 0o600)
            helper_bytes = ("#!/usr/bin/python3\nimport os,sys\n"
                "prompt=sys.argv[1] if len(sys.argv)==2 else ''\n"
                "if prompt.startswith('Username'):\n print('x-access-token')\n"
                "elif prompt.startswith('Password'):\n"
                " with open(os.environ['CARGO_ALLOW_TAG_CREDENTIAL_FILE'],'rb') as f: value=f.read(4097)\n"
                " if len(value)>4096: sys.exit(1)\n"
                " sys.stdout.buffer.write(value+b'\\n')\n"
                "else: sys.exit(1)\n").encode("ascii")
            _write_exact(helper, helper_bytes, 0o700)
            environment["GIT_ASKPASS"] = str(helper)
            environment["CARGO_ALLOW_TAG_CREDENTIAL_FILE"] = str(secret_file)
        command = [str(self._git), "-c", "core.hooksPath=" + os.devnull,
                   "-c", "credential.helper=", "-c", "http.extraHeader=",
                   "-c", "http.followRedirects=false", "-c", "http.sslVerify=true",
                   "-c", "protocol.allow=never", "-c", "protocol.https.allow=always",
                   "-c", "maintenance.auto=false", "-c", "gc.auto=0",
                   "--git-dir=" + str(self._root / "objects.git"), *arguments]
        try:
            result = self._runner(command, input_bytes=raw, cwd=self._root,
                                  environment=environment, output_limit=1024 * 1024)
            _secret_free(result.stdout + result.stderr, self._credential)
            return result
        finally:
            if network:
                for path in (secret_file, helper):
                    try:
                        path.unlink()
                    except FileNotFoundError:
                        pass

    def prepare(self, raw: bytes, oid: str, commit: str, tree: str) -> None:
        self._owner()
        _require(self._oid is None and not self._used and _git_oid("tag", raw) == _sha(oid),
                 "already_used", "annotated request is already prepared or differs")
        _sha(commit); _sha(tree)
        for args in (["init", "--bare", "--template="],
                     ["fetch", "--depth=1", "--filter=blob:none", "--no-tags", "--no-write-fetch-head",
                      "--no-recurse-submodules", "--no-auto-maintenance", "--", REMOTE, commit]):
            result = self._run(args, network=args[0] == "fetch")
            _require(result.returncode == 0, "provider_unavailable", "frozen Git object preparation failed")
        committed = self._run(["cat-file", "-p", commit])
        _require(committed.returncode == 0 and committed.stdout.startswith(f"tree {tree}\n".encode("ascii")),
                 "mismatch", "fetched frozen commit/tree differs")
        written = self._run(["hash-object", "-t", "tag", "-w", "--stdin"], raw)
        _require(written.returncode == 0 and written.stdout == (oid + "\n").encode("ascii"),
                 "mismatch", "native annotated object identity differs")
        observed = self._run(["cat-file", "-p", oid])
        _require(observed.returncode == 0 and observed.stdout == raw,
                 "mismatch", "native annotated object bytes differ")
        self._oid = oid

    def push_once(self) -> bool:
        self._owner()
        with self._lock:
            _require(self._oid is not None and not self._used, "already_used", "physical tag request cannot be repeated")
            self._used = True
        result = self._run(["push", "--porcelain", "--no-verify", "--no-follow-tags", "--recurse-submodules=no",
                            "--", REMOTE, self._oid + ":" + TAG_REF], network=True)
        return result.returncode == 0


def _write_exact(path: Path, raw: bytes, mode: int = 0o600) -> None:
    try:
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
        with os.fdopen(fd, "wb") as handle:
            handle.write(raw)
            handle.flush()
            os.fsync(handle.fileno())
    except (OSError, ValueError):
        _fail("instrument_failure", "exact file creation or durability failed")


def _fsync_directory(path: Path) -> None:
    try:
        fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)
    except OSError:
        _fail("instrument_failure", "directory durability failed")


def _now(clock: Callable[[], float]) -> int:
    value = clock()
    _require(isinstance(value, (int, float)) and not isinstance(value, bool)
             and math.isfinite(value) and 0 < value < 10**12,
             "instrument_failure", "workflow clock is unavailable")
    return int(value)


def _checkpoint_names(files: Mapping[str, bytes]) -> None:
    required = {"identity.json", "events.json", "head.json"}
    names = set(files)
    _require(names in (required, required | {"request.json", "tag-object"}),
             "instrument_failure", "checkpoint file inventory differs")


def _stored_object(files: Mapping[str, bytes]) -> dict[str, Any] | None:
    if not files:
        return None
    _require(set(files) == {_CONTROL_STATE}, "instrument_failure", "unsupported control store inventory")
    return _object(files[_CONTROL_STATE])


def _owned_raw(state: dict[str, Any] | None) -> bytes | None:
    if state is None or state.get("tag") is None:
        return None
    raw = _byte_map({"tag-object": state.get("tag_object")})["tag-object"]
    _require(0 < len(raw) <= MAX_TAG, "instrument_failure", "owned tag object is missing or oversized")
    tag = state.get("tag")
    _require(isinstance(tag, dict) and isinstance(tag.get("tag"), dict)
             and _git_oid("tag", raw) == tag["tag"].get("tag_object_id"),
             "mismatch", "stored annotated bytes and object ID differ")
    return raw


class FinalTagDriver:
    """Real provider/bridge/atomic-store sequence with explicit injectable I/O."""
    def __init__(self, configuration: Mapping[str, Any], store: TagStore, bridge: RustBridge,
                 credential: Callable[[], str], git_factory: Callable[[], GitTagRequest], *, clock=time.time):
        self.config = dict(configuration)
        required = {"repository_id", "anchor_commit", "anchor_tree", "control_prefix", "download_hosts", "producer",
                    "operation_nonce", "expires_at_unix_seconds", "approved_actor_id", "approved_actor_login",
                    "source", "artifacts", "tagger_name", "tagger_email", "tag_message"}
        _require(set(self.config) == required and isinstance(self.config["producer"], dict)
                 and isinstance(self.config["artifacts"], list) and 0 < len(self.config["artifacts"]) <= 32,
                 "invalid_input", "closed independent driver configuration required")
        _require(isinstance(self.config["operation_nonce"], str)
                 and bool(re.fullmatch(r"[A-Za-z0-9._-]{1,100}", self.config["operation_nonce"]))
                 and _integer(self.config["expires_at_unix_seconds"]),
                 "invalid_input", "bounded operation nonce and expiry required")
        self.store, self.bridge, self.credential, self.git_factory, self.clock = store, bridge, credential, git_factory, clock

    def _selected(self) -> tuple[dict[str, bytes], bytes]:
        selected: dict[str, bytes] = {}
        for item in self.config["artifacts"]:
            _require(isinstance(item, dict) and set(item) == {"artifact_id", "transfer", "producer", "selections"}
                     and isinstance(item["selections"], dict),
                     "invalid_input", "independently selected artifact binding required")
            files = self.store.read_artifact(item["transfer"], artifact_id=item["artifact_id"], expected_producer=item["producer"])
            for alias, path in item["selections"].items():
                _require(isinstance(alias, str) and bool(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,160}", alias))
                         and alias not in selected and path in files,
                         "invalid_input", "duplicate or missing selected artifact file")
                raw = files[path]
                if alias.endswith(".json"):
                    _json(raw)
                _secret_free(raw, self.credential)
                selected[alias] = raw
        _require(len(selected) <= 64 and sum(map(len, selected.values())) <= 8 * 1024 * 1024,
                 "invalid_input", "selected artifact set exceeds its bound")
        decision = _object(selected.get("authorization.json", b""))
        _require(isinstance(decision.get("authority"), dict)
                 and decision["authority"].get("source") == self.config["source"],
                 "mismatch", "decision source differs from the independently selected source")
        source = self.store.read_source(self.config["source"], approved_actor_id=self.config["approved_actor_id"],
                                        approved_actor_login=self.config["approved_actor_login"])
        _secret_free(source, self.credential)
        return selected, source

    def _blank_request(self, selected: Mapping[str, bytes], source: bytes) -> dict[str, Any]:
        now = _now(self.clock)
        return {"action": "inspect", "phase": None, "selected": _transport_files(selected), "source_bytes": list(source),
                "producer": self.config["producer"], "operation_nonce": self.config["operation_nonce"],
                "expires_at_unix_seconds": self.config["expires_at_unix_seconds"], "now_unix_seconds": now, "now_utc": _date(now),
                "stored": {}, "retained_checkpoints": [], "prepared": None, "checkpoint": None, "tag_object": [],
                "tag_object_id": "", "remote": None, "provider_observed_at_unix_seconds": None, "response_observed": None}

    def _call(self, request: Mapping[str, Any]) -> dict[str, Any]:
        _secret_free(_json_bytes(request), self.credential)
        result = self.bridge.call(request)
        _secret_free(_json_bytes(result), self.credential)
        return result

    def _load(self, action: str, phase: str | None = None, *, observe: bool = True):
        selected, source = self._selected()
        request = self._blank_request(selected, source)
        identity = self._call(request)
        snapshot = self.store.read(identity["subject_digest"])
        if snapshot.commit is not None:
            expected_message = (
                "cargo-allow release operation store v1\n"
                "subject " + identity["subject_digest"] + "\n"
                "operation " + identity["operation_digest"] + "\n"
                "producer " + sha256(_json_bytes(self.config["producer"])) + "\n"
                "boundary " + identity["request_boundary"] + "\n"
            )
            _require(snapshot.message.startswith(expected_message),
                     "mismatch", "authenticated control append belongs to another operation or producer")
        state = _stored_object(snapshot.files)
        checkpoints: list[dict[str, Any]] = []
        if state is not None:
            retained = state.get("checkpoints")
            _require(isinstance(retained, list) and 1 <= len(retained) <= 5,
                     "instrument_failure", "bounded retained checkpoint inventory required")
            for transfer in retained:
                _require(isinstance(transfer, dict) and transfer.get("producer") == self.config["producer"],
                         "mismatch", "retained checkpoint producer differs")
                numeric = transfer.get("stable_artifact_id")
                _require(isinstance(numeric, str) and bool(re.fullmatch(r"[1-9][0-9]{0,19}", numeric)),
                         "invalid_input", "retained numeric artifact ID required")
                files = self.store.read_artifact(transfer, artifact_id=int(numeric), expected_producer=self.config["producer"])
                checkpoints.append({"transfer": transfer, "files": _transport_files(files)})
        request.update(action=action, phase=phase, stored=_transport_files(snapshot.files), retained_checkpoints=checkpoints)
        owned = _owned_raw(state)
        if observe and (owned is not None or phase in ("intent", "started", "observation")):
            readback = self.store.observe_tag(expected_raw=owned)
            request["remote"] = dict(readback.observation)
            request["provider_observed_at_unix_seconds"] = readback.observed_at_unix_seconds
        now = _now(self.clock)
        request.update(now_unix_seconds=now, now_utc=_date(now))
        return request, identity, snapshot, state

    def prepare(self, phase: str, out: Path) -> dict[str, Any]:
        _require(phase in PHASES and out.is_absolute(), "invalid_input", "exact phase and absolute output path required")
        request, identity, snapshot, state = self._load("prepare", phase)
        if phase == "intent":
            raw = tag_object_bytes(self.config["anchor_commit"], self.config["tagger_name"], self.config["tagger_email"],
                                   request["now_unix_seconds"], self.config["tag_message"])
            request["tag_object"], request["tag_object_id"] = list(raw), _git_oid("tag", raw)
        response = self._call(request)
        prepared = response.get("prepared")
        _require(isinstance(prepared, dict) and prepared.get("phase") == phase,
                 "instrument_failure", "typed preparation did not return its exact phase")
        files = _byte_map(prepared.get("checkpoint_files"))
        _checkpoint_names(files)
        name = "cargo-allow-" + identity["operation_identity"]["operation_id"] + "-" + phase + "-" + sha256(_json_bytes(prepared))[-16:]
        plan = {"configuration_sha256": sha256(_json_bytes(self.config)), "bridge_sha256": self.bridge._digest,
                "previous_control_commit": snapshot.commit, "operation_digest": response["operation_digest"],
                "subject_digest": response["subject_digest"], "artifact_name": name, "prepared": prepared}
        raw_plan = _json_bytes(plan)
        _secret_free(raw_plan, self.credential)
        _require(len(raw_plan) <= MAX_PLAN, "invalid_input", "prepared phase exceeds its bound")
        try:
            out.mkdir(mode=0o700)
            checkpoint_dir = out / "checkpoint"
            checkpoint_dir.mkdir(mode=0o700)
            for path, raw in files.items():
                _write_exact(checkpoint_dir / path, raw)
            _fsync_directory(checkpoint_dir)
            _write_exact(out / "plan.json", raw_plan)
            _fsync_directory(out)
            _fsync_directory(out.parent)
        except StoreError:
            raise
        except OSError:
            _fail("instrument_failure", "prepared checkpoint directory could not be created")
        return {"phase": phase, "artifact_name": name, "checkpoint_directory": str(out / "checkpoint"),
                "operation_digest": response["operation_digest"], "subject_digest": response["subject_digest"],
                "previous_control_commit": snapshot.commit, "gate_open": False}

    def _read_plan(self, directory: Path) -> dict[str, Any]:
        _require(directory.is_absolute() and not directory.is_symlink() and not (directory / "checkpoint").is_symlink(),
                 "invalid_input", "exact regular prepared directory required")
        plan = _object(_regular_bytes(directory / "plan.json", MAX_PLAN))
        _require(set(plan) == {"configuration_sha256", "bridge_sha256", "previous_control_commit", "operation_digest", "subject_digest", "artifact_name", "prepared"}
                 and plan["configuration_sha256"] == sha256(_json_bytes(self.config))
                 and plan["bridge_sha256"] == self.bridge._digest and isinstance(plan["prepared"], dict),
                 "mismatch", "prepared configuration or compiled bridge changed")
        files = _byte_map(plan["prepared"].get("checkpoint_files"))
        _checkpoint_names(files)
        try:
            actual_paths = {path.name for path in (directory / "checkpoint").iterdir()}
        except OSError:
            _fail("instrument_failure", "prepared checkpoint files are unavailable")
        _require(actual_paths == set(files), "mismatch", "prepared checkpoint inventory moved")
        for path, raw in files.items():
            _require(_regular_bytes(directory / "checkpoint" / path, 2 * 1024 * 1024) == raw,
                     "mismatch", "prepared checkpoint bytes moved")
        return plan

    def _finalized_transfer(self, plan: Mapping[str, Any], artifact_id: int) -> dict[str, Any]:
        _require(_integer(artifact_id), "invalid_input", "assigned positive numeric artifact ID required")
        _, metadata = self.store._api("GET", f"/actions/artifacts/{artifact_id}")
        _require(metadata.get("id") == artifact_id and metadata.get("name") == plan["artifact_name"],
                 "mismatch", "uploaded artifact name or assigned ID differs")
        created = metadata.get("created_at")
        _utc(created)
        files = _byte_map(plan["prepared"]["checkpoint_files"])
        return {"schema_id": "cargo-allow.release-artifact-transfer.v1", "schema_version": 1,
                "transfer_id": "operation-checkpoint-" + str(artifact_id), "role": "release-operation-checkpoint",
                "stable_artifact_id": str(artifact_id), "producer": self.config["producer"],
                "provider_id": "github-actions-artifact", "provider_artifact_name": plan["artifact_name"],
                "files": [{"path": path, "size_bytes": len(raw), "sha256": sha256(raw)} for path, raw in sorted(files.items())],
                "semantic_payload_digest": sha256(files["head.json"]), "trust_class": "ManualDispatch",
                "untrusted_input_posture": "StrictByteMatch", "created_at_utc": created,
                "claim_boundary": ["exact_producer_identity", "file_set_sha256_and_size_binding", "trust_class_enforcement", "no_shell_or_workflow_interpolation"],
                "limitations": ["does_not_prove_provider_availability", "does_not_mutate_remote_storage"]}

    def _append(self, snapshot, response):
        files = _byte_map(response["files"])
        _secret_free(_json_bytes(_transport_files(files)), self.credential)
        return self.store.prepare_append(snapshot, files, operation_digest=response["operation_digest"],
                                         producer_bytes=_json_bytes(self.config["producer"]),
                                         request_boundary=response["request_boundary"], valid_until=response["valid_until"]).append()

    def _record_response(self, observed: bool) -> None:
        request, _, snapshot, _ = self._load("response", observe=False)
        request["response_observed"] = observed
        response = self._call(request)
        _require(response["push_object_id"] is None and not response["gate_open"],
                 "instrument_failure", "response retention cannot open a request boundary")
        self._append(snapshot, response)

    def finalize(self, directory: Path, artifact_id: int) -> dict[str, Any]:
        plan = self._read_plan(directory)
        phase = plan["prepared"].get("phase")
        _require(phase in PHASES, "invalid_input", "prepared phase is unsupported")
        request, identity, snapshot, state = self._load("finalize", phase)
        _require(snapshot.commit == plan["previous_control_commit"]
                 and identity["operation_digest"] == plan["operation_digest"] and identity["subject_digest"] == plan["subject_digest"],
                 "conflict", "prepared control parent or operation changed")
        transfer = self._finalized_transfer(plan, artifact_id)
        downloaded = self.store.read_artifact(transfer, artifact_id=artifact_id, expected_producer=self.config["producer"])
        _require(dict(downloaded) == _byte_map(plan["prepared"]["checkpoint_files"]),
                 "mismatch", "uploaded checkpoint differs from the prepared exact bytes")
        request["prepared"] = plan["prepared"]
        request["checkpoint"] = {"transfer": transfer, "files": _transport_files(downloaded)}
        now = _now(self.clock)
        request.update(now_unix_seconds=now, now_utc=_date(now))
        response = self._call(request)
        if phase == "started":
            _require(isinstance(response["push_object_id"], str) and response["gate_open"] is False,
                     "instrument_failure", "typed start did not name exactly one closed-gate request")
            next_state = _stored_object(_byte_map(response["files"]))
            raw = _owned_raw(next_state)
            _require(raw is not None and _git_oid("tag", raw) == response["push_object_id"],
                     "mismatch", "typed start and immutable annotated object differ")
            with self.git_factory() as git_request:
                git_request.prepare(raw, response["push_object_id"], self.config["anchor_commit"], self.config["anchor_tree"])
                # Object preparation is read-only remotely. Recheck independent
                # inputs after it and before creating the sole Started witness.
                fresh, fresh_identity, fresh_snapshot, _ = self._load("finalize", phase)
                _require(fresh_snapshot.commit == snapshot.commit and fresh_identity["operation_digest"] == identity["operation_digest"],
                         "conflict", "operation moved during native object preparation")
                fresh["prepared"], fresh["checkpoint"] = request["prepared"], request["checkpoint"]
                response = self._call(fresh)
                witness = self._append(fresh_snapshot, response)
                def push_fixed(_snapshot):
                    _require(_now(self.clock) < response["valid_until"], "expired", "operation expired before the fixed request")
                    return git_request.push_once()
                try:
                    observed = witness.consume(push_fixed)
                except StoreError:
                    observed = False
                self._record_response(observed)
            # This read alone opens no gate; exact observation must first be
            # retained in its independently uploaded checkpoint.
            observed_tag = self.store.observe_tag(expected_raw=raw)
            return {"phase": phase, "subject_digest": response["subject_digest"], "gate_open": False,
                    "response_observed": observed, "remote_observation": dict(observed_tag.observation),
                    "provider_observed_at_unix_seconds": observed_tag.observed_at_unix_seconds,
                    "next_phase": "observation"}
        _require(response["push_object_id"] is None, "instrument_failure", "only Started may describe the physical request")
        witness = self._append(snapshot, response)
        # An unused metadata append witness is never a tag permit. No callback
        # is invoked for bootstrap/lease/intent/observation or restarted reads.
        del witness
        if phase == "observation":
            return self.continuation()
        retained = self.store.read(response["subject_digest"])
        return {"phase": phase, "subject_digest": response["subject_digest"], "control_commit": retained.commit,
                "gate_open": False, "checkpoint_transfer": transfer}

    def continuation(self) -> dict[str, Any]:
        request, _, snapshot, _ = self._load("validate")
        response = self._call(request)
        _require(response["gate_open"] is True and response["push_object_id"] is None,
                 "ineligible", "same-run continuation requires retained exact tag observation")
        state = _stored_object(snapshot.files)
        _require(state is not None, "instrument_failure", "validated state is missing")
        return {"gate_open": True, "control_commit": snapshot.commit, "subject_digest": response["subject_digest"],
                "operation_identity": state["identity"], "operation_head": state["heads"][-1],
                "authorization_custody": state["authorization"], "operation_lease": state["lease"],
                "tag_transaction": state["tag"], "checkpoint_transfer": state["checkpoints"][-1],
                "producer": state["producer"]}

    def reconcile(self, subject_digest: str) -> dict[str, Any]:
        """Read-only, even after expiry: no append, new selection or push permit."""
        snapshot = self.store.read(subject_digest)
        state = _stored_object(snapshot.files)
        raw = _owned_raw(state)
        observation = self.store.observe_tag(expected_raw=raw)
        # Existing terminal/Started JSON is only diagnostic here. The guarded
        # prepare/finalize observation path performs complete typed replay.
        return {"gate_open": False, "control_commit": snapshot.commit, "subject_digest": subject_digest,
                "remote_observation": dict(observation.observation),
                "provider_observed_at_unix_seconds": observation.observed_at_unix_seconds,
                "next_action": "retain_exact_observation_in_same_run_or_route_to_recovery"}


def _credential_fd(fd: int, *, timeout: float = 10) -> Callable[[], str]:
    _require(type(fd) is int and fd >= 3, "invalid_input", "explicit nonstandard credential descriptor required")
    _require(0 < timeout <= 10, "invalid_input", "credential read deadline is invalid")
    chunks = bytearray()
    deadline = time.monotonic() + timeout
    try:
        while len(chunks) <= 4096:
            remaining = deadline - time.monotonic()
            _require(remaining > 0 and select.select([fd], [], [], max(0, remaining))[0],
                     "instrument_failure", "explicit credential descriptor exceeded its deadline")
            chunk = os.read(fd, 4097 - len(chunks))
            if not chunk:
                break
            chunks.extend(chunk)
        raw = bytes(chunks)
    except (OSError, ValueError):
        _fail("instrument_failure", "explicit credential descriptor is unavailable")
    _require(0 < len(raw) <= 4096, "invalid_input", "credential descriptor exceeds its bound")
    try:
        value = raw.decode("ascii").rstrip("\n")
    except UnicodeDecodeError:
        _fail("invalid_input", "credential descriptor is malformed")
    _require(value and all(33 <= ord(c) <= 126 for c in value), "invalid_input", "credential descriptor is malformed")
    return lambda: value


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, required=True)
    parser.add_argument("--bridge", type=Path, required=True)
    parser.add_argument("--bridge-sha256", required=True)
    parser.add_argument("--git", type=Path, required=True)
    parser.add_argument("--credential-fd", type=int, required=True)
    commands = parser.add_subparsers(dest="command", required=True)
    prepare_parser = commands.add_parser("prepare")
    prepare_parser.add_argument("--phase", choices=PHASES, required=True)
    prepare_parser.add_argument("--out", type=Path, required=True)
    finalize_parser = commands.add_parser("finalize")
    finalize_parser.add_argument("--prepared", type=Path, required=True)
    finalize_parser.add_argument("--artifact-id", type=int, required=True)
    commands.add_parser("continuation")
    reconcile_parser = commands.add_parser("reconcile")
    reconcile_parser.add_argument("--subject-digest", required=True)
    arguments = parser.parse_args()
    try:
        configuration = _object(_regular_bytes(arguments.config, MAX_PLAN))
        credential = _credential_fd(arguments.credential_fd)
        store = TagStore(repository_id=configuration["repository_id"], anchor_commit=configuration["anchor_commit"],
                         anchor_tree=configuration["anchor_tree"], control_prefix=configuration["control_prefix"],
                         credential=credential, download_hosts=frozenset(configuration["download_hosts"]))
        driver = FinalTagDriver(configuration, store, RustBridge(arguments.bridge, arguments.bridge_sha256), credential,
                                lambda: GitTagRequest(arguments.git, credential))
        if arguments.command == "prepare":
            result = driver.prepare(arguments.phase, arguments.out)
        elif arguments.command == "finalize":
            result = driver.finalize(arguments.prepared, arguments.artifact_id)
        elif arguments.command == "continuation":
            result = driver.continuation()
        else:
            result = driver.reconcile(arguments.subject_digest)
        raw = _json_bytes(result)
        _secret_free(raw, credential)
        sys.stdout.buffer.write(raw + b"\n")
        return 0
    except StoreError as error:
        print(str(error), file=sys.stderr)
        return 2
    except (OSError, KeyError, TypeError, ValueError, OverflowError):
        print("release-final-tag: instrument_failure: malformed or unavailable driver input", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
