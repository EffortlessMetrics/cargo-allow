"""Authenticated release-object reads and a restricted append-once Git store.

This module transports existing domain records. It does not evaluate release
eligibility, mint authorization, create tags, or read ambient credentials.
The caller must run the existing Rust reducers before preparing an append.
A fresh append witness proves provider byte storage, never release authority.
"""

from __future__ import annotations

import base64
from dataclasses import dataclass, field
from datetime import datetime, timezone
import hashlib
import io
import json
import math
import os
import re
import secrets
import ssl
import stat
import threading
import time
from types import MappingProxyType
from typing import Any, Callable, Mapping
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlsplit
from urllib.request import (
    HTTPRedirectHandler, HTTPSHandler, ProxyHandler, Request, build_opener,
)
import zipfile
import zlib


REPOSITORY = "EffortlessMetrics/cargo-allow"
API = "https://api.github.com"
API_VERSION = "2022-11-28"
_COMMITTER_NAME = "Cargo Allow Release Store"
_COMMITTER_EMAIL = "release-store@invalid"
_SEAL = object()
_SHA = re.compile(r"[0-9a-f]{40}\Z")
_DIGEST = re.compile(r"sha256:[0-9a-f]{64}\Z")
_TOKEN = re.compile(r"[A-Za-z0-9._:/#-]{1,200}\Z")
_CONTROL_FILE = re.compile(r"[a-z][a-z0-9.-]{0,118}\.json\Z")


class StoreError(Exception):
    """Sanitized transport failure; never includes provider bodies or headers."""

    def __init__(self, kind: str, detail: str):
        self.kind = kind
        super().__init__(f"release operation store: {kind}: {detail}")


def _require(condition: bool, kind: str, detail: str) -> None:
    if not condition:
        raise StoreError(kind, detail)


def _integer(value: Any, *, positive: bool = True) -> bool:
    return type(value) is int and value >= (1 if positive else 0)


def _same_integer(value: Any, expected: int) -> bool:
    return _integer(value) and value == expected


def _selected_job_id(value: Any) -> int:
    # Match the existing provider JSON-number resource ceiling before parsing.
    _require(isinstance(value, str) and 1 <= len(value) <= 128
             and bool(re.fullmatch(r"[1-9][0-9]*", value)),
             "invalid_input", "selected numeric provider job ID required")
    return int(value)


def _sha(value: Any) -> str:
    _require(isinstance(value, str) and bool(_SHA.fullmatch(value)),
             "invalid_input", "expected a canonical GitHub SHA-1 object identity")
    return value


def _digest(value: Any) -> str:
    _require(isinstance(value, str) and bool(_DIGEST.fullmatch(value)),
             "invalid_input", "expected a canonical SHA-256 digest")
    return value


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def _git_oid(kind: str, data: bytes) -> str:
    header = f"{kind} {len(data)}\0".encode("ascii")
    return hashlib.sha1(header + data, usedforsecurity=False).hexdigest()


def _json_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"),
                      sort_keys=True, allow_nan=False).encode("utf-8")


def _json(data: bytes) -> Any:
    def unique(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            _require(key not in result, "instrument_failure", "duplicate JSON key")
            result[key] = value
        return result

    def no_constant(_: str) -> None:
        raise StoreError("instrument_failure", "non-finite JSON value")

    def bounded_integer(value: str) -> int:
        _require(len(value) <= 128, "instrument_failure", "oversized JSON number")
        return int(value)

    def finite_float(value: str) -> float:
        _require(len(value) <= 128, "instrument_failure", "oversized JSON number")
        parsed = float(value)
        _require(math.isfinite(parsed), "instrument_failure", "non-finite JSON value")
        return parsed

    try:
        value = json.loads(data.decode("utf-8"), object_pairs_hook=unique,
                           parse_constant=no_constant, parse_int=bounded_integer,
                           parse_float=finite_float)
        pending = [value]
        while pending:
            item = pending.pop()
            if isinstance(item, dict):
                pending.extend(item.keys())
                pending.extend(item.values())
            elif isinstance(item, list):
                pending.extend(item)
            elif isinstance(item, str):
                _require(not any(0xD800 <= ord(c) <= 0xDFFF for c in item),
                         "instrument_failure", "invalid JSON Unicode scalar")
        return value
    except (UnicodeDecodeError, ValueError, RecursionError):
        raise StoreError("instrument_failure", "malformed JSON bytes") from None


def _object(data: bytes) -> dict[str, Any]:
    value = _json(data)
    _require(isinstance(value, dict), "instrument_failure", "expected a JSON object")
    return value


def _utc(value: Any) -> int:
    _require(isinstance(value, str) and len(value) <= 40,
             "instrument_failure", "missing provider timestamp")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
        _require(parsed.tzinfo is not None, "instrument_failure",
                 "provider timestamp lacks a timezone")
        return int(parsed.timestamp())
    except (ValueError, OverflowError):
        raise StoreError("instrument_failure", "malformed provider timestamp") from None


@dataclass(frozen=True)
class Limits:
    """Resource bounds; callers may tighten, but cannot enlarge these ceilings."""

    timeout_seconds: float = 20.0
    json_bytes: int = 4 * 1024 * 1024
    file_bytes: int = 2 * 1024 * 1024
    total_file_bytes: int = 8 * 1024 * 1024
    archive_bytes: int = 16 * 1024 * 1024
    files: int = 64
    job_pages: int = 10

    def __post_init__(self) -> None:
        _require(isinstance(self.timeout_seconds, (int, float))
                 and not isinstance(self.timeout_seconds, bool)
                 and math.isfinite(self.timeout_seconds)
                 and 0 < self.timeout_seconds <= 30,
                 "invalid_input", "invalid HTTP deadline")
        for value, ceiling in [
            (self.json_bytes, 4 * 1024 * 1024),
            (self.file_bytes, 2 * 1024 * 1024),
            (self.total_file_bytes, 8 * 1024 * 1024),
            (self.archive_bytes, 16 * 1024 * 1024),
            (self.files, 64),
            (self.job_pages, 10),
        ]:
            _require(_integer(value) and value <= ceiling,
                     "invalid_input", "invalid resource bound")


@dataclass(frozen=True, repr=False)
class HttpResponse:
    """Private transport data, not a retained release authority envelope."""

    status: int
    headers: Mapping[str, str] = field(repr=False)
    body: bytes = field(repr=False)


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req: Any, fp: Any, code: int, msg: str,
                         headers: Any, newurl: str) -> None:
        return None


def _headers(items: Any) -> dict[str, str]:
    checked: dict[str, str] = {}
    size = 0
    for count, (key, value) in enumerate(items, start=1):
        _require(isinstance(key, str) and isinstance(value, str)
                 and bool(re.fullmatch(r"[A-Za-z0-9!#$%&'*+.^_\x60|~-]+", key))
                 and all(32 <= ord(c) <= 126 or c == "\t" for c in value),
                 "instrument_failure", "malformed HTTP headers")
        lower = key.lower()
        size += len(key) + len(value)
        # Bound physical lines and characters before combining only Vary.
        # Routing, framing, encoding and all other duplicates remain errors.
        _require((lower not in checked or lower == "vary")
                 and count <= 128 and size <= 65536,
                 "instrument_failure", "ambiguous or oversized HTTP headers")
        checked[lower] = checked[lower] + ", " + value if lower in checked else value
    return checked


def _content_length(headers: Mapping[str, str], limit: int) -> int | None:
    length = headers.get("content-length")
    if length is None:
        return None
    _require(len(length) <= 10 and length.isascii() and length.isdecimal()
             and int(length) <= limit,
             "instrument_failure", "HTTP body exceeds its bound")
    return int(length)


def https_transport(method: str, url: str, headers: Mapping[str, str],
                    body: bytes | None, timeout: float, limit: int) -> HttpResponse:
    """One bounded HTTPS exchange, with no retries, proxy, or redirects."""
    _require(urlsplit(url).scheme == "https", "invalid_input", "HTTPS required")
    opener = build_opener(ProxyHandler({}), _NoRedirect(),
                          HTTPSHandler(context=ssl.create_default_context()))
    request = Request(url, data=body, headers=dict(headers), method=method)
    deadline = time.monotonic() + timeout
    try:
        try:
            response = opener.open(request, timeout=timeout)
        except HTTPError as error:
            response = error
        with response:
            checked_headers = _headers(response.headers.items())
            length = _content_length(checked_headers, limit)
            chunks: list[bytes] = []
            received = 0
            while True:
                _require(time.monotonic() <= deadline, "provider_unavailable",
                         "HTTP body exceeded its deadline")
                chunk = response.read1(min(65536, limit + 1 - received))
                _require(time.monotonic() <= deadline, "provider_unavailable",
                         "HTTP body exceeded its deadline")
                if not chunk:
                    break
                chunks.append(chunk)
                received += len(chunk)
                _require(received <= limit, "instrument_failure",
                         "HTTP body exceeds its bound")
            data = b"".join(chunks)
            _require(length is None or len(data) == length, "instrument_failure",
                     "HTTP body length differs")
            encoding = checked_headers.get("content-encoding", "identity")
            _require(encoding == "identity", "instrument_failure",
                     "unexpected HTTP content encoding")
            return HttpResponse(response.status, checked_headers, data)
    except StoreError:
        raise
    except (OSError, URLError, ValueError):
        raise StoreError("provider_unavailable", "HTTPS exchange failed") from None


@dataclass(frozen=True, repr=False)
class _Snapshot:
    store: object = field(repr=False)
    ref: str
    commit: str | None
    tree: str | None
    parent: str | None
    files: Mapping[str, bytes] = field(repr=False)
    message: str = field(repr=False)


class _ProcessOnce:
    """No durable or copyable path back to an active attempt or witness."""

    __slots__ = ("_pid", "_lock", "_used", "_deadline", "_clock")

    def __init__(self, seal: object, deadline: float,
                 clock: Callable[[], float]):
        _require(seal is _SEAL, "invalid_input", "private process object")
        self._pid = os.getpid()
        self._lock = threading.Lock()
        self._used = False
        self._deadline = deadline
        self._clock = clock

    def _take(self) -> None:
        # Check before touching the lock: a fork can inherit a locked mutex.
        _require(os.getpid() == self._pid, "already_used",
                 "process identity changed")
        with self._lock:
            _require(not self._used, "already_used", "attempt already consumed")
            self._used = True
            _require(self._clock() <= self._deadline, "expired",
                     "active attempt exceeded its deadline")

    def __copy__(self) -> Any:
        raise StoreError("already_used", "process objects cannot be copied")

    def __deepcopy__(self, memo: Any) -> Any:
        raise StoreError("already_used", "process objects cannot be copied")

    def __reduce_ex__(self, protocol: int) -> Any:
        raise StoreError("already_used", "process objects cannot be serialized")

    def __repr__(self) -> str:
        return "<private release-store process object>"


class _FreshAppend(_ProcessOnce):
    __slots__ = ("_snapshot",)

    def __init__(self, seal: object, snapshot: _Snapshot, deadline: float,
                 clock: Callable[[], float]):
        super().__init__(seal, deadline, clock)
        self._snapshot = snapshot

    def consume(self, callback: Callable[[_Snapshot], Any]) -> Any:
        """Consume before invoking; callback failure cannot restore the witness."""
        self._take()
        return callback(self._snapshot)


class _AppendAttempt(_ProcessOnce):
    __slots__ = ("_store", "_snapshot", "_files", "_message", "_timestamp")

    def __init__(self, seal: object, store: GitHubReleaseStore,
                 snapshot: _Snapshot, files: Mapping[str, bytes], message: str,
                 timestamp: int, deadline: float):
        super().__init__(seal, deadline, store._monotonic)
        self._store = store
        self._snapshot = snapshot
        self._files = files
        self._message = message
        self._timestamp = timestamp

    def append(self) -> _FreshAppend:
        self._take()
        return self._store._append(self)


class GitHubReleaseStore:
    """An explicitly configured provider adapter; construction performs no I/O.

    The selected control prefix, repository ID, anchor and download hosts come
    from the caller's independently checked deployment context. Constructor
    arguments do not prove that live permissions or controls are acceptable.
    """

    def __init__(
        self, *, repository_id: int, anchor_commit: str, anchor_tree: str,
        control_prefix: str, credential: Callable[[], str],
        download_hosts: frozenset[str] = frozenset(),
        transport: Callable[..., HttpResponse] = https_transport,
        limits: Limits = Limits(), clock: Callable[[], float] = time.time,
        monotonic: Callable[[], float] = time.monotonic,
    ):
        _require(_integer(repository_id), "invalid_input", "repository ID required")
        _require(isinstance(control_prefix, str)
                 and bool(re.fullmatch(
                     r"refs/heads/(?:[a-z0-9][a-z0-9_-]*/)+", control_prefix))
                 and len(control_prefix) <= 160,
                 "invalid_input", "a dedicated control branch prefix is required")
        for host in download_hosts:
            _require(isinstance(host, str)
                     and bool(re.fullmatch(r"[a-z0-9]+(?:[.-][a-z0-9]+)*", host))
                     and "." in host and not host.replace(".", "").isdecimal()
                     and host != "api.github.com",
                     "invalid_input", "invalid selected download host")
        self._repository_id = repository_id
        self._anchor_commit = _sha(anchor_commit)
        self._anchor_tree = _sha(anchor_tree)
        self._prefix = control_prefix
        self._credential = credential
        self._download_hosts = frozenset(download_hosts)
        self._transport = transport
        self._limits = limits
        self._clock = clock
        self._monotonic = monotonic
        self._identity = object()
        self._claim_lock = threading.Lock()
        self._claims: set[str] = set()
        self._base = f"/repos/{REPOSITORY}"

    def _exchange(self, method: str, url: str, *, body: bytes | None = None,
                  authenticated: bool = True, limit: int | None = None) -> HttpResponse:
        headers = {"Accept": "application/vnd.github+json",
                   "Accept-Encoding": "identity",
                   "User-Agent": "cargo-allow-release-operation-store/1"}
        if authenticated:
            _require(url.startswith(API + self._base + "/")
                     or url == API + self._base,
                     "invalid_input", "credential destination rejected")
            try:
                token = self._credential()
                _require(isinstance(token, str) and 1 <= len(token) <= 4096
                         and all(32 < ord(c) < 127 for c in token),
                         "invalid_input", "credential provider returned invalid data")
                headers["Authorization"] = "Bearer " + token
            except Exception:
                raise StoreError("invalid_input", "credential provider failed") from None
            headers["X-GitHub-Api-Version"] = API_VERSION
        if body is not None:
            headers["Content-Type"] = "application/json"
        bound = self._limits.json_bytes if limit is None else limit
        try:
            response = self._transport(method, url, headers, body,
                                       self._limits.timeout_seconds, bound)
        except StoreError as error:
            # Preserve the bounded native transport's instrument-failure
            # class without trusting any message supplied by a transport.
            kind = "instrument_failure" if error.kind == "instrument_failure" else "provider_unavailable"
            raise StoreError(kind, "provider exchange failed") from None
        except Exception:
            # Never format an exception originating in a credential-bearing
            # transport: it can contain a URL, request headers or provider body.
            raise StoreError("provider_unavailable", "provider exchange failed") from None
        _require(isinstance(response, HttpResponse)
                 and _integer(response.status) and 100 <= response.status <= 599
                 and isinstance(response.headers, Mapping)
                 and isinstance(response.body, bytes) and len(response.body) <= bound,
                 "instrument_failure", "invalid bounded HTTP response")
        checked_headers = _headers(response.headers.items())
        length = _content_length(checked_headers, bound)
        _require(length is None or len(response.body) == length, "instrument_failure",
                 "HTTP body length differs")
        _require(checked_headers.get("content-encoding", "identity") == "identity",
                 "instrument_failure", "unexpected HTTP content encoding")
        return HttpResponse(response.status, checked_headers, response.body)

    def _api(self, method: str, suffix: str, payload: Any = None,
             statuses: tuple[int, ...] = (200,)) -> tuple[HttpResponse, Any]:
        response = self._exchange(method, API + self._base + suffix,
                                  body=None if payload is None else _json_bytes(payload))
        if response.status not in statuses:
            kind = "conflict" if response.status in (409, 422) else "provider_unavailable"
            raise StoreError(kind, f"provider returned HTTP {response.status}")
        return response, _object(response.body)

    def _repository(self) -> None:
        _, repository = self._api("GET", "")
        _require(_same_integer(repository.get("id"), self._repository_id)
                 and repository.get("full_name") == REPOSITORY,
                 "mismatch", "repository identity differs")
        # A metadata-only repository response cannot establish Contents access.
        # Read a known independently selected commit before accepting ref 404.
        _, anchor = self._api("GET", "/git/commits/" + self._anchor_commit)
        _require(anchor.get("sha") == self._anchor_commit
                 and isinstance(anchor.get("tree"), dict)
                 and anchor["tree"].get("sha") == self._anchor_tree,
                 "mismatch", "independent anchor commit/tree differs")

    def read_source(self, source: Mapping[str, Any], *, approved_actor_id: int,
                    approved_actor_login: str) -> bytes:
        """Read an exact IssueComment source; source authority is caller-owned."""
        _require(isinstance(source, Mapping) and _integer(approved_actor_id)
                 and isinstance(approved_actor_login, str)
                 and bool(re.fullmatch(r"[A-Za-z0-9-]{1,39}", approved_actor_login)),
                 "invalid_input", "independently approved actor required")
        _require(source.get("kind") == "issue_comment"
                 and source.get("repository") == REPOSITORY
                 and source.get("author") == approved_actor_login,
                 "invalid_input", "unsupported or foreign authorization source")
        reference = source.get("reference")
        match = re.fullmatch(r"issue:([1-9][0-9]*)#comment:([1-9][0-9]*)",
                             reference) if isinstance(reference, str) and len(reference) <= 100 else None
        _require(match is not None, "invalid_input", "exact issue comment required")
        expected_digest = _digest(source.get("body_digest"))
        issue, comment = match.groups()
        self._repository()
        _, observed = self._api("GET", "/issues/comments/" + comment)
        user = observed.get("user")
        _require(_same_integer(observed.get("id"), int(comment))
                 and observed.get("issue_url") == API + self._base + "/issues/" + issue
                 and isinstance(user, dict)
                 and _same_integer(user.get("id"), approved_actor_id)
                 and user.get("login") == approved_actor_login,
                 "mismatch", "source object or approved actor differs")
        body = observed.get("body")
        _require(isinstance(body, str), "instrument_failure", "source body is missing")
        raw = body.encode("utf-8")
        _require(len(raw) <= self._limits.file_bytes,
                 "instrument_failure", "source body exceeds its bound")
        _require(sha256(raw) == expected_digest, "mismatch", "source bytes differ")
        return raw

    def _ref(self, ref: str) -> str | None:
        suffix = "/git/ref/" + quote(ref.removeprefix("refs/"), safe="/")
        response = self._exchange("GET", API + self._base + suffix)
        if response.status == 404:
            return None
        _require(response.status == 200, "provider_unavailable",
                 f"ref read returned HTTP {response.status}")
        value = _object(response.body)
        _require(value.get("ref") == ref and isinstance(value.get("object"), dict)
                 and value["object"].get("type") == "commit",
                 "mismatch", "exact control ref identity differs")
        return _sha(value["object"].get("sha"))

    def _files(self, files: Mapping[str, bytes]) -> Mapping[str, bytes]:
        _require(isinstance(files, Mapping) and 0 < len(files) <= self._limits.files,
                 "invalid_input", "bounded nonempty control file set required")
        copied: dict[str, bytes] = {}
        total = 0
        for path, data in files.items():
            _require(isinstance(path, str) and bool(_CONTROL_FILE.fullmatch(path))
                     and isinstance(data, bytes) and 0 < len(data) <= self._limits.file_bytes,
                     "invalid_input", "invalid regular control JSON file")
            _json(data)
            total += len(data)
            copied[path] = data
        _require(total <= self._limits.total_file_bytes,
                 "invalid_input", "control file set exceeds its bound")
        return MappingProxyType(copied)

    def _commit_bytes(self, tree: str, parents: list[str], timestamp: int,
                      message: str) -> bytes:
        identity = f"{_COMMITTER_NAME} <{_COMMITTER_EMAIL}> {timestamp} +0000"
        lines = [f"tree {tree}", *(f"parent {parent}" for parent in parents),
                 f"author {identity}", f"committer {identity}", "", message]
        return "\n".join(lines).encode("utf-8")

    def _read_snapshot(self, ref: str) -> _Snapshot:
        commit = self._ref(ref)
        if commit is None:
            return _Snapshot(self._identity, ref, None, None, None,
                             MappingProxyType({}), "")
        _, record = self._api("GET", "/git/commits/" + commit)
        _require(record.get("sha") == commit and isinstance(record.get("tree"), dict),
                 "mismatch", "control commit identity differs")
        tree = _sha(record["tree"].get("sha"))
        parents = record.get("parents")
        _require(isinstance(parents, list) and len(parents) <= 1
                 and all(isinstance(item, dict) for item in parents),
                 "mismatch", "control commit must have at most one parent")
        parent_ids = [_sha(item.get("sha")) for item in parents]
        author, committer = record.get("author"), record.get("committer")
        _require(isinstance(author, dict) and isinstance(committer, dict)
                 and author == committer
                 and author.get("name") == _COMMITTER_NAME
                 and author.get("email") == _COMMITTER_EMAIL,
                 "mismatch", "control commit author/committer differs")
        timestamp = _utc(author.get("date"))
        message = record.get("message")
        message_prefix = ("cargo-allow release operation store v1\nsubject sha256:"
                          + ref.removeprefix(self._prefix) + "\n")
        _require(isinstance(message, str) and len(message) <= 1500
                 and bool(re.fullmatch(
                     re.escape(message_prefix) + r"operation sha256:[0-9a-f]{64}\n"
                     r"producer sha256:[0-9a-f]{64}\nboundary [A-Za-z0-9._:/#-]{1,200}\n"
                     r"claim [0-9a-f]{64}\n", message)),
                 "mismatch", "control commit message differs")
        _require(_git_oid("commit", self._commit_bytes(
            tree, parent_ids, timestamp, message)) == commit,
            "mismatch", "control commit bytes differ")
        _, tree_record = self._api("GET", "/git/trees/" + tree)
        entries = tree_record.get("tree")
        _require(tree_record.get("sha") == tree
                 and tree_record.get("truncated") is False
                 and isinstance(entries, list) and 0 < len(entries) <= self._limits.files,
                 "instrument_failure", "invalid or truncated control tree")
        files: dict[str, bytes] = {}
        tree_bytes: dict[str, bytes] = {}
        for entry in entries:
            _require(isinstance(entry, dict), "instrument_failure", "invalid tree entry")
            path = entry.get("path")
            _require(isinstance(path, str) and bool(_CONTROL_FILE.fullmatch(path))
                     and path not in files and entry.get("mode") == "100644"
                     and entry.get("type") == "blob",
                     "mismatch", "unexpected control file or mode")
            oid = _sha(entry.get("sha"))
            _, blob = self._api("GET", "/git/blobs/" + oid)
            _require(blob.get("sha") == oid and blob.get("encoding") == "base64"
                     and _integer(blob.get("size"))
                     and blob["size"] <= self._limits.file_bytes
                     and isinstance(blob.get("content"), str),
                     "instrument_failure", "invalid bounded control blob")
            try:
                # GitHub wraps base64 with line feeds; other junk is rejected.
                encoded = blob["content"].replace("\n", "")
                data = base64.b64decode(encoded, validate=True)
            except ValueError:
                raise StoreError("instrument_failure", "invalid blob encoding") from None
            _require(len(data) == blob["size"]
                     and _same_integer(entry.get("size"), len(data))
                     and _git_oid("blob", data) == oid,
                     "mismatch", "control blob bytes differ")
            files[path] = data
            tree_bytes[path] = b"100644 " + path.encode("ascii") + b"\0" + bytes.fromhex(oid)
        actual_tree = b"".join(tree_bytes[path] for path in sorted(tree_bytes))
        _require(_git_oid("tree", actual_tree) == tree, "mismatch", "control tree bytes differ")
        checked = self._files(files)
        _require(self._ref(ref) == commit, "conflict", "control ref moved during readback")
        return _Snapshot(self._identity, ref, commit, tree,
                         next(iter(parent_ids), None), checked, message)

    def read(self, subject_digest: str) -> _Snapshot:
        """Observe only. No read, including a restarted read, returns a witness."""
        ref = self._prefix + _digest(subject_digest).removeprefix("sha256:")
        self._repository()
        return self._read_snapshot(ref)

    def prepare_append(self, observed: _Snapshot, files: Mapping[str, bytes], *,
                       operation_digest: str, producer_bytes: bytes,
                       request_boundary: str, valid_until: int) -> _AppendAttempt:
        """Prepare once after caller-owned semantic checks; performs no HTTP."""
        _require(isinstance(observed, _Snapshot) and observed.store is self._identity,
                 "invalid_input", "snapshot belongs to another store instance")
        checked = self._files(files)
        _require(dict(checked) != dict(observed.files),
                 "already_used", "unchanged file set cannot create a fresh append")
        _digest(operation_digest)
        _require(isinstance(producer_bytes, bytes)
                 and 0 < len(producer_bytes) <= self._limits.file_bytes,
                 "invalid_input", "bounded existing producer record required")
        _object(producer_bytes)
        _require(isinstance(request_boundary, str)
                 and bool(_TOKEN.fullmatch(request_boundary)),
                 "invalid_input", "bounded request boundary required")
        now = self._clock()
        _require(isinstance(now, (int, float)) and math.isfinite(now)
                 and _integer(valid_until) and 0 <= now < valid_until,
                 "expired", "append validity window is closed")
        nonce = secrets.token_hex(32)
        _require(bool(re.fullmatch(r"[0-9a-f]{64}", nonce)),
                 "instrument_failure", "claim nonce generation failed")
        with self._claim_lock:
            _require(nonce not in self._claims and len(self._claims) < 1024,
                     "instrument_failure", "claim nonce reused or session exhausted")
            self._claims.add(nonce)
        subject = observed.ref.removeprefix(self._prefix)
        message = (
            "cargo-allow release operation store v1\n"
            f"subject sha256:{subject}\noperation {operation_digest}\n"
            f"producer {sha256(producer_bytes)}\nboundary {request_boundary}\n"
            f"claim {nonce}\n"
        )
        # Use a monotonic budget once created; wall-clock rollback cannot
        # lengthen an active attempt or resurrect an expired witness.
        deadline = self._monotonic() + min(valid_until - now, 300)
        return _AppendAttempt(_SEAL, self, observed, checked, message, int(now), deadline)

    def _append(self, attempt: _AppendAttempt) -> _FreshAppend:
        observed = attempt._snapshot
        self._repository()
        current = self._read_snapshot(observed.ref)
        _require(current.commit == observed.commit
                 and dict(current.files) == dict(observed.files),
                 "conflict", "observed control parent is stale")
        tree_entries: list[dict[str, str]] = []
        tree_bytes: list[bytes] = []
        for path in sorted(attempt._files):
            _require(self._monotonic() <= attempt._deadline,
                     "expired", "append expired before object creation")
            data = attempt._files[path]
            oid = _git_oid("blob", data)
            _, blob = self._api("POST", "/git/blobs",
                                {"content": base64.b64encode(data).decode("ascii"),
                                 "encoding": "base64"}, (201,))
            _require(blob.get("sha") == oid, "mismatch", "created blob identity differs")
            tree_entries.append({"path": path, "mode": "100644", "type": "blob", "sha": oid})
            tree_bytes.append(b"100644 " + path.encode("ascii") + b"\0" + bytes.fromhex(oid))
        tree = _git_oid("tree", b"".join(tree_bytes))
        _require(self._monotonic() <= attempt._deadline,
                 "expired", "append expired before tree creation")
        _, made_tree = self._api("POST", "/git/trees", {"tree": tree_entries}, (201,))
        _require(made_tree.get("sha") == tree, "mismatch", "created tree identity differs")
        parents = [] if observed.commit is None else [observed.commit]
        date = datetime.fromtimestamp(attempt._timestamp, timezone.utc).isoformat().replace("+00:00", "Z")
        actor = {"name": _COMMITTER_NAME, "email": _COMMITTER_EMAIL, "date": date}
        commit = _git_oid("commit", self._commit_bytes(
            tree, parents, attempt._timestamp, attempt._message))
        _require(commit != observed.commit, "already_used", "unchanged commit rejected")
        _require(self._monotonic() <= attempt._deadline,
                 "expired", "append expired before commit creation")
        _, made_commit = self._api("POST", "/git/commits",
                                  {"message": attempt._message, "tree": tree,
                                   "parents": parents, "author": actor, "committer": actor},
                                  (201,))
        _require(made_commit.get("sha") == commit, "mismatch", "created commit identity differs")
        # Recheck both the independent parent and the process budget before
        # the only ref mutation. No transport automatically retries it.
        _require(self._ref(observed.ref) == observed.commit,
                 "conflict", "control parent changed before append")
        _require(self._monotonic() <= attempt._deadline,
                 "expired", "append expired before ref mutation")
        try:
            if observed.commit is None:
                _, written = self._api("POST", "/git/refs",
                                       {"ref": observed.ref, "sha": commit}, (201,))
            else:
                suffix = "/git/refs/" + quote(observed.ref.removeprefix("refs/"), safe="/")
                _, written = self._api("PATCH", suffix, {"sha": commit, "force": False})
            _require(written.get("ref") == observed.ref
                     and isinstance(written.get("object"), dict)
                     and written["object"].get("type") == "commit"
                     and written["object"].get("sha") == commit,
                     "mismatch", "ref mutation response differs")
            readback = self._read_snapshot(observed.ref)
            _require(readback.commit == commit and readback.tree == tree
                     and readback.parent == observed.commit
                     and readback.message == attempt._message
                     and dict(readback.files) == dict(attempt._files),
                     "mismatch", "independent append readback differs")
            _require(self._monotonic() <= attempt._deadline,
                     "expired", "append expired during readback")
        except Exception:
            # Even an exact later read cannot recover a permit from this
            # uncertainty. Reconciliation is the ordinary read-only API.
            raise StoreError("uncertain", "ref append has no fresh usable witness") from None
        return _FreshAppend(_SEAL, readback, attempt._deadline, self._monotonic)

    def _inventory(self, transfer: Mapping[str, Any]) -> dict[str, tuple[int, str]]:
        rows = transfer.get("files")
        _require(isinstance(rows, list) and 0 < len(rows) <= self._limits.files,
                 "invalid_input", "bounded artifact inventory required")
        result: dict[str, tuple[int, str]] = {}
        total = 0
        for row in rows:
            _require(isinstance(row, dict), "invalid_input", "invalid artifact file")
            path = row.get("path")
            self._archive_path(path)
            size = row.get("size_bytes")
            _require(path not in result and _integer(size, positive=False)
                     and size <= self._limits.file_bytes,
                     "invalid_input", "duplicate or oversized artifact file")
            result[path] = (size, _digest(row.get("sha256")))
            total += size
        _require(total <= self._limits.total_file_bytes,
                 "invalid_input", "artifact inventory exceeds its bound")
        return result

    @staticmethod
    def _archive_path(path: Any) -> None:
        _require(isinstance(path, str) and 0 < len(path) <= 240
                 and bool(re.fullmatch(r"[A-Za-z0-9._/-]+", path))
                 and all(part not in ("", ".", "..") for part in path.split("/"))
                 and not path.endswith("/")
                 and all(not part.endswith((".", " ")) for part in path.split("/")),
                 "invalid_input", "unsafe artifact path")

    def _unzip(self, raw: bytes, inventory: Mapping[str, tuple[int, str]]) -> Mapping[str, bytes]:
        files: dict[str, bytes] = {}
        folded: set[str] = set()
        try:
            with zipfile.ZipFile(io.BytesIO(raw)) as archive:
                entries = archive.infolist()
                _require(len(entries) == len(inventory), "mismatch",
                         "artifact file count differs")
                total = 0
                for entry in entries:
                    self._archive_path(entry.filename)
                    mode = entry.external_attr >> 16
                    _require(entry.orig_filename == entry.filename
                             and not entry.is_dir() and not (entry.flag_bits & 1)
                             and (stat.S_IFMT(mode) in (0, stat.S_IFREG))
                             and entry.compress_type in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED)
                             and entry.filename in inventory
                             and entry.filename not in files
                             and entry.filename.casefold() not in folded,
                             "mismatch", "unexpected, duplicate or nonregular artifact file")
                    size, digest = inventory[entry.filename]
                    _require(entry.file_size == size and entry.file_size <= self._limits.file_bytes,
                             "mismatch", "artifact member size differs")
                    total += entry.file_size
                    _require(total <= self._limits.total_file_bytes,
                             "instrument_failure", "expanded artifact exceeds its bound")
                    with archive.open(entry) as member:
                        data = member.read(size + 1)
                    _require(len(data) == size and sha256(data) == digest,
                             "mismatch", "artifact member bytes differ")
                    files[entry.filename] = data
                    folded.add(entry.filename.casefold())
        except StoreError:
            raise
        except (OSError, EOFError, ValueError, zipfile.BadZipFile, zlib.error,
                RuntimeError, NotImplementedError):
            raise StoreError("instrument_failure", "malformed artifact archive") from None
        return MappingProxyType(files)

    def _attempt_job(self, producer: Mapping[str, Any], job_id: int) -> dict[str, Any]:
        run_id, attempt = producer["run_id"], producer["run_attempt"]
        found: dict[str, Any] | None = None
        seen: set[int] = set()
        expected_total: int | None = None
        for page in range(1, self._limits.job_pages + 1):
            _, listing = self._api(
                "GET", f"/actions/runs/{run_id}/attempts/{attempt}/jobs?per_page=100&page={page}")
            count, jobs = listing.get("total_count"), listing.get("jobs")
            _require(_integer(count, positive=False)
                     and count <= self._limits.job_pages * 100
                     and isinstance(jobs, list) and len(jobs) <= 100
                     and (expected_total is None or count == expected_total),
                     "instrument_failure", "incomplete or moving job inventory")
            expected_total = count
            for job in jobs:
                _require(isinstance(job, dict) and _integer(job.get("id"))
                         and job["id"] not in seen,
                         "instrument_failure", "duplicate or invalid provider job")
                seen.add(job["id"])
                if job["id"] == job_id:
                    found = job
            if len(seen) == count:
                break
            _require(len(jobs) == 100, "instrument_failure", "truncated job inventory")
        _require(len(seen) == expected_total and found is not None,
                 "mismatch", "selected job is absent from the selected attempt")
        _require(_same_integer(found.get("run_id"), run_id)
                 and found.get("head_sha") == producer["commit_sha"],
                 "mismatch", "selected job provenance differs")
        return found

    def _artifact_metadata(self, artifact_id: int, transfer: Mapping[str, Any],
                           producer: Mapping[str, Any]) -> dict[str, Any]:
        _, metadata = self._api("GET", f"/actions/artifacts/{artifact_id}")
        run = metadata.get("workflow_run")
        now = self._clock()
        _require(_same_integer(metadata.get("id"), artifact_id)
                 and metadata.get("name") == transfer.get("provider_artifact_name")
                 and metadata.get("expired") is False
                 and _utc(metadata.get("expires_at")) > now
                 and _utc(metadata.get("created_at")) <= now
                 and isinstance(run, dict)
                 and _same_integer(run.get("id"), producer["run_id"])
                 and _same_integer(run.get("repository_id"), self._repository_id)
                 and _same_integer(run.get("head_repository_id"), self._repository_id)
                 and run.get("head_sha") == producer["commit_sha"],
                 "mismatch", "artifact identity, provenance or expiry differs")
        return metadata

    def read_artifact(self, transfer: Mapping[str, Any], *,
                      artifact_id: int, expected_producer: Mapping[str, Any]) -> Mapping[str, bytes]:
        """Download exact selected files, preserving the existing envelope.

        The expected envelope/producer binding must originate in the selected
        independent producer receipt. GitHub's artifact metadata binds the run,
        not an individual job; attempt/job reads corroborate that receipt and
        cannot replace it. This method does not assert maintainer approval.
        """
        _require(_integer(artifact_id)
                 and isinstance(transfer, Mapping)
                 and isinstance(expected_producer, Mapping)
                 and transfer.get("schema_id") == "cargo-allow.release-artifact-transfer.v1"
                 and _same_integer(transfer.get("schema_version"), 1)
                 and transfer.get("provider_id") == "github-actions-artifact"
                 and transfer.get("stable_artifact_id") == str(artifact_id)
                 and isinstance(transfer.get("producer"), Mapping)
                 and transfer.get("producer") == expected_producer
                 and all(type(transfer["producer"].get(key)) is type(value)
                         for key, value in expected_producer.items())
                 and transfer.get("trust_class") in ("ManualDispatch", "TagWorkflow", "CleanRelease")
                 and transfer.get("untrusted_input_posture") == "StrictByteMatch",
                 "invalid_input", "exact trusted artifact transfer binding required")
        producer = dict(expected_producer)
        _require(producer.get("repository") == REPOSITORY
                 and _integer(producer.get("run_id")) and _integer(producer.get("run_attempt"))
                 and producer.get("release_version") == "0.2.0"
                 and isinstance(producer.get("workflow_path"), str)
                 and bool(re.fullmatch(r"\.github/workflows/[A-Za-z0-9_-]+\.ya?ml",
                                      producer["workflow_path"]))
                 and isinstance(producer.get("git_ref"), str)
                 and len(producer["git_ref"]) <= 240
                 and bool(re.fullmatch(r"refs/(heads|tags)/[A-Za-z0-9._/-]+",
                                      producer["git_ref"]))
                 and ".." not in producer["git_ref"]
                 and all(part and not part.startswith(".")
                         and not part.endswith((".", ".lock"))
                         for part in producer["git_ref"].split("/")),
                 "invalid_input", "unsupported selected producer identity")
        _sha(producer.get("commit_sha"))
        _sha(producer.get("tree_sha"))
        job_id = _selected_job_id(producer.get("job_id"))
        inventory = self._inventory(transfer)
        self._repository()
        metadata = self._artifact_metadata(artifact_id, transfer, producer)
        run_id, attempt = producer["run_id"], producer["run_attempt"]
        _, run = self._api("GET", f"/actions/runs/{run_id}/attempts/{attempt}")
        path = run.get("path")
        expected_path = producer["workflow_path"]
        expected_short_ref = producer["git_ref"].split("/", 2)[-1]
        event = run.get("event")
        tag_push = event == "push" and producer["git_ref"].startswith("refs/tags/")
        supported_event = event == "workflow_dispatch" or tag_push
        _require(_same_integer(run.get("id"), run_id)
                 and _same_integer(run.get("run_attempt"), attempt)
                 and run.get("head_sha") == producer["commit_sha"]
                 and isinstance(run.get("repository"), dict)
                 and _same_integer(run["repository"].get("id"), self._repository_id)
                 and isinstance(run.get("head_repository"), dict)
                 and _same_integer(run["head_repository"].get("id"), self._repository_id)
                 and path in (expected_path, expected_path + "@" + producer["git_ref"],
                              expected_path + "@" + expected_short_ref)
                 and supported_event
                 and (transfer["trust_class"] != "ManualDispatch" or event == "workflow_dispatch")
                 and (transfer["trust_class"] != "TagWorkflow" or tag_push)
                 and run.get("head_branch") == expected_short_ref,
                 "mismatch", "workflow attempt provenance differs")
        _, commit = self._api("GET", "/git/commits/" + producer["commit_sha"])
        _require(commit.get("sha") == producer["commit_sha"]
                 and isinstance(commit.get("tree"), dict)
                 and commit["tree"].get("sha") == producer["tree_sha"],
                 "mismatch", "producer commit/tree differs")
        job = self._attempt_job(producer, job_id)
        created = _utc(metadata.get("created_at"))
        completed = job.get("completed_at")
        _require(_utc(job.get("started_at")) <= created
                 and (completed is None or created <= _utc(completed))
                 and job.get("status") in ("in_progress", "completed")
                 and (job.get("status") != "completed" or job.get("conclusion") == "success"),
                 "mismatch", "artifact creation is outside the selected job interval")
        redirect = self._exchange("GET", API + self._base + f"/actions/artifacts/{artifact_id}/zip")
        _require(redirect.status == 302, "provider_unavailable",
                 "artifact download did not return its selected redirect")
        locations = [value for key, value in redirect.headers.items()
                     if key.lower() == "location"]
        _require(len(locations) == 1 and isinstance(locations[0], str),
                 "instrument_failure", "artifact redirect is missing or ambiguous")
        location = locations[0]
        _require(len(location) <= 8192 and all(ord(c) > 32 and ord(c) < 127 for c in location),
                 "instrument_failure", "invalid signed artifact redirect")
        try:
            parsed = urlsplit(location)
            port = parsed.port
        except ValueError:
            raise StoreError("instrument_failure", "invalid signed artifact redirect") from None
        _require(parsed.scheme == "https" and parsed.hostname in self._download_hosts
                 and parsed.username is None and parsed.password is None
                 and port in (None, 443) and not parsed.fragment,
                 "mismatch", "artifact download destination is not selected")
        archive = self._exchange("GET", location, authenticated=False,
                                 limit=self._limits.archive_bytes)
        _require(archive.status == 200, "provider_unavailable",
                 "artifact download was not delivered")
        # Artifact size_in_bytes can describe provider storage rather than the
        # transport ZIP, so the selected file inventory is the byte authority.
        files = self._unzip(archive.body, inventory)
        after = self._artifact_metadata(artifact_id, transfer, producer)
        for key in ("id", "name", "created_at", "expires_at", "workflow_run", "digest"):
            _require(after.get(key) == metadata.get(key),
                     "mismatch", "artifact metadata changed during download")
        return files
