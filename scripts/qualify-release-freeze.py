#!/usr/bin/env python3
"""Prepare and qualify a frozen candidate through authenticated, read-only I/O.

The selection file is independently supplied deployment configuration, not a
release authority schema. It selects existing qualification/transfer records,
the exact native review and producer identities, and an authorization window.
The Rust consumer owns typed qualification, custody, readiness and replay.
This driver cannot retain/upload objects, mint, tag, publish or change controls.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
from email.utils import parsedate_to_datetime
import json
import importlib.util
import math
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import time
from typing import Any, Callable, Mapping

_spec = importlib.util.spec_from_file_location(
    "release_operation_store", Path(__file__).with_name("release_operation_store.py"))
if _spec is None or _spec.loader is None:
    raise RuntimeError("selected provider module unavailable")
store = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = store
_spec.loader.exec_module(store)


def require(condition: bool, detail: str, kind: str = "invalid_input") -> None:
    store._require(condition, kind, detail)


def obj(value: Any, keys: set[str], detail: str) -> dict[str, Any]:
    require(isinstance(value, dict) and set(value) == keys, detail)
    return value


def canonical_utc(value: Any) -> str:
    require(isinstance(value, str) and bool(re.fullmatch(
        r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", value)),
        "canonical UTC timestamp required")
    stamp = store._utc(value)
    require(datetime.fromtimestamp(stamp, timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ") == value,
            "canonical UTC timestamp required")
    return value


def date_at(now: float) -> str:
    require(type(now) in (int, float) and math.isfinite(now)
            and 0 <= now <= 253_402_300_799, "evaluation clock unavailable")
    return datetime.fromtimestamp(now, timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def read_json(path: Path) -> tuple[bytes, dict[str, Any]]:
    try:
        with path.open("rb") as handle:
            raw = handle.read(store.Limits().json_bytes + 1)
    except OSError:
        raise store.StoreError("invalid_input", "selected input could not be read") from None
    require(len(raw) <= store.Limits().json_bytes, "selected input exceeds its bound")
    return raw, store._object(raw)


class _ObservedReadStore(store.GitHubReleaseStore):
    """Apply this invocation's observation checks to every authenticated read."""

    def __init__(self, *, observe: Callable[[store.HttpResponse], None], **kwargs):
        super().__init__(**kwargs)
        self._observe = observe

    def _exchange(self, method: str, url: str, *, body: bytes | None = None,
                  authenticated: bool = True, limit: int | None = None) -> store.HttpResponse:
        require(method == "GET" and body is None,
                "qualification permits read-only provider requests")
        response = super()._exchange(method, url, body=body,
                                     authenticated=authenticated, limit=limit)
        if authenticated:
            self._observe(response)
        return response


class Qualifier:
    """One fresh read sequence. No retained flag can skip its provider reads."""

    def __init__(self, selection: Mapping[str, Any], *, selection_dir: Path,
                 credential: Callable[[], str], transport=store.https_transport,
                 clock: Callable[[], float] = time.time,
                 monotonic: Callable[[], float] = time.monotonic):
        self.selection = store._object(store._json_bytes(dict(selection)))
        self.directory = selection_dir.resolve()
        self.clock = clock
        self.monotonic = monotonic
        self.started: float | None = None
        self._started_tick: float | None = None
        self._last_wall: float | None = None
        self._last_tick: float | None = None
        selected = obj(self.selection, {
            "repository_id", "anchor_commit", "anchor_tree", "download_hosts",
            "qualification_source", "qualification_actor", "native_review",
            "reviewed_graph", "artifacts", "authorization_window_end_utc",
            "maximum_observation_age_seconds",
        }, "closed independent qualifier selection required")
        age = selected["maximum_observation_age_seconds"]
        require(store._integer(age) and age <= 2**63 - 1,
                "independently selected observation age is required")
        canonical_utc(selected["authorization_window_end_utc"])
        require(isinstance(selected["download_hosts"], list)
                and all(isinstance(host, str) for host in selected["download_hosts"])
                and len(selected["download_hosts"]) == len(set(selected["download_hosts"])),
                "selected download hosts required")
        obj(selected["qualification_actor"], {"id", "login"}, "qualification source actor required")
        review = obj(selected["native_review"], {"pr_number", "review_id", "actor_id", "actor_login", "body_digest"},
                     "exact independent native review selection required")
        require(all(store._integer(review[key]) for key in ("pr_number", "review_id", "actor_id")),
                "numeric native review identity required")
        store._digest(review["body_digest"])
        require(isinstance(review["actor_login"], str)
                and bool(re.fullmatch(r"[A-Za-z0-9_-]{1,100}", review["actor_login"])),
                "selected review actor required")
        require(isinstance(selected["artifacts"], list) and 1 <= len(selected["artifacts"]) <= 64,
                "bounded original artifact selection required")
        self.client = _ObservedReadStore(
            observe=self.observe,
            repository_id=selected["repository_id"], anchor_commit=selected["anchor_commit"],
            anchor_tree=selected["anchor_tree"], control_prefix="refs/heads/release-control/",
            credential=credential, download_hosts=frozenset(selected["download_hosts"]),
            transport=transport, clock=self.sample_time, monotonic=monotonic)
        self.retained_controls: dict[str, bytes] = {}

    def sample_time(self) -> float:
        now = self.clock()
        date_at(now)
        tick = self.monotonic()
        require(type(tick) in (int, float) and math.isfinite(tick),
                "monotonic observation clock unavailable", "instrument_failure")
        require((self._last_wall is None or now >= self._last_wall)
                and (self._last_tick is None or tick >= self._last_tick),
                "qualification observation clock moved backward", "provider_unavailable")
        if self._started_tick is None:
            self._started_tick = tick
        age = self.selection["maximum_observation_age_seconds"]
        require(tick - self._started_tick <= age
                and (self.started is None or now - self.started <= age)
                and date_at(now) < self.selection["authorization_window_end_utc"],
                "qualification observation window elapsed", "provider_unavailable")
        self._last_wall, self._last_tick = now, tick
        return now

    def observe(self, response: store.HttpResponse) -> None:
        now = self.sample_time()
        try:
            observed = parsedate_to_datetime(response.headers["date"])
            stamp = observed.timestamp()
        except (KeyError, TypeError, ValueError, OverflowError):
            raise store.StoreError("instrument_failure", "provider Date unavailable or malformed") from None
        require(observed.tzinfo is not None and math.isfinite(stamp)
                and 0 <= now - stamp <= self.selection["maximum_observation_age_seconds"]
                and response.headers.get("age", "0") == "0",
                "current provider observation is stale, future or cached", "provider_unavailable")

    def get(self, suffix: str) -> Any:
        _, body = self.client._api("GET", suffix)
        return body

    def current_main(self) -> None:
        result = self.get("/git/ref/heads/main")
        require(isinstance(result, dict) and result.get("ref") == "refs/heads/main"
                and isinstance(result.get("object"), dict)
                and result["object"].get("type") == "commit"
                and result["object"].get("sha") == self.selection["anchor_commit"],
                "selected merged main moved", "mismatch")

    def commit(self, sha: str, expected_tree: str | None = None) -> dict[str, Any]:
        store._sha(sha)
        result = self.get("/git/commits/" + sha)
        require(isinstance(result, dict) and result.get("sha") == sha
                and isinstance(result.get("tree"), dict), "commit identity differs", "mismatch")
        store._sha(result["tree"].get("sha"))
        if expected_tree is not None:
            require(result["tree"]["sha"] == expected_tree, "commit tree differs", "mismatch")
        return result

    def qualification(self) -> dict[str, Any]:
        self.current_main()
        actor = self.selection["qualification_actor"]
        raw = self.client.read_source(self.selection["qualification_source"],
                                      approved_actor_id=actor["id"], approved_actor_login=actor["login"])
        record = store._object(raw)
        obj(record, {"schema_id", "schema_version", "qualification_id", "reviewed", "merged",
                     "changed_files", "semantic_owners", "premerge_evidence_digest",
                     "preserved_evidence_nodes", "invalidated_evidence_nodes", "required_rerun_set",
                     "created_at_utc", "claim_boundary", "limitations"},
            "existing closed CargoAllowPostMergeQualificationV1 record required")
        self.retained_controls["final-freeze.qualification-source.json"] = raw
        reviewed = obj(record.get("reviewed"), {"base_sha", "head_sha", "merge_base_sha", "tree_sha"},
                       "existing ReviewedContextV1 required")
        merged = obj(record.get("merged"), {"pr_number", "merge_commit_sha", "merge_tree_sha",
            "current_main_commit_sha", "current_main_tree_sha", "merge_method", "merge_parents"},
            "existing MergedStateV1 required")
        selected_review = self.selection["native_review"]
        require(merged["pr_number"] == selected_review["pr_number"]
                and type(merged["pr_number"]) is int
                and merged["merge_commit_sha"] == self.selection["anchor_commit"]
                and merged["merge_tree_sha"] == self.selection["anchor_tree"]
                and merged["current_main_commit_sha"] == self.selection["anchor_commit"]
                and merged["current_main_tree_sha"] == self.selection["anchor_tree"],
                "qualification merged subject differs from independent selection", "mismatch")
        self.commit(reviewed["base_sha"])
        self.commit(reviewed["head_sha"], reviewed["tree_sha"])
        self.commit(reviewed["merge_base_sha"])
        actual = self.commit(merged["merge_commit_sha"], merged["merge_tree_sha"])
        parents = actual.get("parents")
        require(isinstance(parents, list) and parents
                and all(isinstance(parent, dict) for parent in parents)
                and [parent.get("sha") for parent in parents] == merged["merge_parents"],
                "merged commit parents differ", "mismatch")
        for parent in merged["merge_parents"]:
            store._sha(parent)
        if merged["merge_method"] == "MergeCommit":
            require(merged["merge_parents"] == [reviewed["base_sha"], reviewed["head_sha"]],
                    "selected merge-commit lineage differs", "mismatch")
        elif merged["merge_method"] == "Squash":
            require(merged["merge_parents"] == [reviewed["base_sha"]],
                    "selected squash lineage differs", "mismatch")
        else:
            raise store.StoreError("invalid_input", "this qualifier requires an exact merge-commit or squash lineage")
        comparison = self.get("/compare/" + reviewed["base_sha"] + "..." + reviewed["head_sha"])
        require(isinstance(comparison, dict) and isinstance(comparison.get("merge_base_commit"), dict)
                and comparison["merge_base_commit"].get("sha") == reviewed["merge_base_sha"],
                "reviewed merge base differs", "mismatch")
        pr = self.get(f'/pulls/{selected_review["pr_number"]}')
        require(isinstance(pr, dict) and store._same_integer(pr.get("number"), selected_review["pr_number"])
                and pr.get("merged") is True and pr.get("merge_commit_sha") == merged["merge_commit_sha"]
                and isinstance(pr.get("head"), dict) and pr["head"].get("sha") == reviewed["head_sha"]
                and isinstance(pr.get("base"), dict) and pr["base"].get("ref") == "main"
                and isinstance(pr["base"].get("repo"), dict)
                and store._same_integer(pr["base"]["repo"].get("id"), self.selection["repository_id"]),
                "selected PR merge/readback differs", "mismatch")
        review = self.get(f'/pulls/{selected_review["pr_number"]}/reviews/{selected_review["review_id"]}')
        require(isinstance(review, dict) and store._same_integer(review.get("id"), selected_review["review_id"])
                and review.get("commit_id") == reviewed["head_sha"]
                and review.get("state") in ("APPROVED", "COMMENTED")
                and isinstance(review.get("user"), dict)
                and store._same_integer(review["user"].get("id"), selected_review["actor_id"])
                and review["user"].get("login") == selected_review["actor_login"]
                and isinstance(review.get("body"), str), "selected native review differs", "mismatch")
        try:
            review_bytes = review["body"].encode("utf-8")
        except UnicodeError:
            raise store.StoreError("instrument_failure", "invalid native review body") from None
        require(store.sha256(review_bytes) == selected_review["body_digest"],
                "selected native review body changed", "mismatch")
        canonical_utc(review.get("submitted_at"))
        require(review["submitted_at"] <= canonical_utc(record.get("created_at_utc")) <= date_at(self.sample_time()),
                "qualification/review chronology differs", "mismatch")
        self.retained_controls["final-freeze.native-review.json"] = store._json_bytes(review)
        self.current_main()
        return record

    def artifact(self, selected: Any, *, reviewed: bool = False) -> tuple[dict[str, Any], store.ArtifactReadback]:
        selected = obj(selected, {"artifact_id", "transfer_path", "expected_producer", "members"},
                       "closed artifact/member selection required")
        require(store._integer(selected["artifact_id"]) and isinstance(selected["transfer_path"], str),
                "selected numeric artifact and envelope path required")
        transfer_path = Path(selected["transfer_path"])
        if not transfer_path.is_absolute():
            transfer_path = self.directory / transfer_path
        raw, transfer = read_json(transfer_path)
        expected = selected["expected_producer"]
        require(isinstance(expected, dict), "independent producer required")
        if not reviewed:
            require(expected.get("commit_sha") == self.selection["anchor_commit"]
                    and expected.get("tree_sha") == self.selection["anchor_tree"],
                    "post-merge regeneration required; original producers cannot be relabeled", "mismatch")
        require(isinstance(selected["members"], list) and selected["members"], "selected logical members required")
        members: dict[str, tuple[str, str]] = {}
        for member in selected["members"]:
            member = obj(member, {"logical_id", "role", "path"}, "closed member selection required")
            require(isinstance(member["logical_id"], str) and member["logical_id"] not in members,
                    "duplicate logical member selection")
            members[member["logical_id"]] = (member["role"], member["path"])
        result = self.client.read_artifact_members(transfer, artifact_id=selected["artifact_id"],
                                                  expected_producer=expected, members=members)
        require(canonical_utc(result.created_at_utc) <= date_at(self.sample_time())
                and canonical_utc(result.retention_expiry_utc) > date_at(self.sample_time()),
                "artifact readback interval is not current", "mismatch")
        # A prepared payload may be retained/read in its still-running job.
        # Original semantic evidence and reviewed evidence require completed
        # producer jobs. This does not turn a job result into a receipt result.
        prepared_ids = {"final-freeze-receipt", "final-freeze-evidence-graph"}
        if reviewed or set(members) != prepared_ids:
            job = self.client._attempt_job(expected, store._selected_job_id(expected.get("job_id")))
            require(job.get("status") == "completed" and job.get("conclusion") == "success"
                    and canonical_utc(job.get("completed_at")) <= date_at(self.sample_time()),
                    "original evidence producer job is not complete", "mismatch")
        self.retained_controls[f'original-transfer-{selected["artifact_id"]}.json'] = raw
        return transfer, result

    def run(self, phase: str, out: Path, bridge: Callable[[str, dict[str, Any], list[str], Path], dict[str, Any]]) -> dict[str, Any]:
        require(phase in ("prepare", "qualify"), "unsupported qualifier phase")
        # Refuse reuse before any provider read. A failed later invocation
        # must never leave an earlier successful observation at its output path.
        try:
            out.mkdir(parents=True, exist_ok=False)
        except OSError:
            raise store.StoreError("invalid_input", "a fresh writable output directory is required") from None
        self.retained_controls = {}
        self.started = self._started_tick = self._last_wall = self._last_tick = None
        self.started = self.sample_time()
        now = date_at(self.started)
        require(self.selection["authorization_window_end_utc"] > now,
                "selected authorization window has ended", "mismatch")
        qualification = self.qualification()
        reviewed_transfer, reviewed = self.artifact(self.selection["reviewed_graph"], reviewed=True)
        expected = self.selection["reviewed_graph"]["expected_producer"]
        require(expected.get("commit_sha") == qualification["reviewed"]["head_sha"]
                and expected.get("tree_sha") == qualification["reviewed"]["tree_sha"]
                and len(reviewed.members) == 1 and reviewed.members[0].role == "EvidenceGraph",
                "selected reviewed graph provenance differs", "mismatch")
        reviewed_graph = store._object(reviewed.members[0].data)
        self.retained_controls["final-freeze.reviewed-graph-source.json"] = reviewed.members[0].data
        self.retained_controls["final-freeze.reviewed-graph-transfer.json"] = store._json_bytes(reviewed_transfer)
        objects: list[dict[str, Any]] = []
        object_ids: set[int] = set()
        ids: set[str] = set()
        total = 0
        with tempfile.TemporaryDirectory(prefix="cargo-allow-freeze-readback-") as directory:
            staging = Path(directory)
            evidence: list[str] = []
            for selected in self.selection["artifacts"]:
                transfer, readback = self.artifact(selected)
                require(readback.artifact_id not in object_ids, "duplicate selected provider object")
                object_ids.add(readback.artifact_id)
                base = staging / str(readback.artifact_id)
                members = []
                for member in readback.members:
                    require(member.logical_id not in ids, "duplicate selected logical identity")
                    ids.add(member.logical_id)
                    total += len(member.data)
                    require(total <= store.Limits().total_file_bytes and len(ids) <= store.Limits().files,
                            "selected freeze set exceeds provider resource bounds")
                    path = base / member.path
                    path.parent.mkdir(parents=True, exist_ok=True)
                    with path.open("xb") as handle:
                        handle.write(member.data)
                    label = None
                    if member.role.startswith("Evidence:"):
                        label = member.role.removeprefix("Evidence:")
                        require(member.logical_id == "evidence:" + label, "evidence logical role differs")
                    elif member.role == "ReleaseManifest":
                        require(member.logical_id == "release-manifest-v2", "manifest logical role differs")
                        label = "release-manifest"
                    if label is not None:
                        evidence.append(label + "=" + str(path))
                    members.append({"logical_id":member.logical_id, "role":member.role,
                                    "path":member.path, "bytes":list(member.data)})
                objects.append({"transfer":transfer, "expected_producer":selected["expected_producer"],
                                "created_at_utc":readback.created_at_utc,
                                "retention_expiry_utc":readback.retention_expiry_utc, "members":members})
            prepared_ids = {"final-freeze-receipt", "final-freeze-evidence-graph"}
            require((phase == "prepare" and not ids.intersection(prepared_ids))
                    or (phase == "qualify" and prepared_ids.issubset(ids)),
                    "phase does not select its exact prepared payload")
            self.current_main()
            observed = self.sample_time()
            wire = {"observed_at_utc":date_at(observed),
                    "authorization_window_end_utc":self.selection["authorization_window_end_utc"],
                    "qualification":qualification, "reviewed_evidence_graph":reviewed_graph,
                    "artifacts":objects}
            result = bridge(phase, wire, sorted(evidence), out)
            require(isinstance(result, dict) and result.get("stage") == (
                "prepared" if phase == "prepare" else "qualified-computation"),
                "typed consumer did not return its selected stage", "instrument_failure")
            self.current_main()
            ended = self.sample_time()
            require(self.started <= observed <= ended
                    and ended - self.started <= self.selection["maximum_observation_age_seconds"]
                    and date_at(ended) < self.selection["authorization_window_end_utc"],
                    "qualification observation window elapsed", "provider_unavailable")
            for name, data in self.retained_controls.items():
                out.mkdir(parents=True, exist_ok=True)
                (out / name).write_bytes(data)
            # These are exact computational inputs, explicitly not a durable
            # assertion that subsequent provider reads are current.
            (out / "final-freeze.readback-input.json").write_bytes(store._json_bytes(wire))
            result = {**result, "provider_observed_at_utc":date_at(ended),
                      "provider_claim_boundary":"read-only observations for this invocation; later operations require fresh readback"}
            (out / "final-freeze.provider-observation.json").write_bytes(store._json_bytes(result))
            return result


def native_bridge(binary: Path, root: Path):
    binary = binary.resolve(strict=True)
    root = root.resolve(strict=True)

    def invoke(phase: str, wire: dict[str, Any], evidence: list[str], out: Path) -> dict[str, Any]:
        with tempfile.TemporaryDirectory(prefix="cargo-allow-freeze-bridge-") as directory:
            path = Path(directory) / "readback.json"
            path.write_bytes(store._json_bytes(wire))
            argv = [str(binary), "release-freeze", phase, "--readback-input", str(path), "--out-dir", str(out)]
            for value in evidence:
                argv.extend(["--evidence", value])
            clean = {key:value for key, value in os.environ.items() if key not in {
                "GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY",
                "GIT_ALTERNATE_OBJECT_DIRECTORIES", "CARGO_ALLOW_ROOT", "CARGO_ALLOW_CONFIG",
                "GH_TOKEN", "GITHUB_TOKEN", "CARGO_REGISTRY_TOKEN",
            }}
            with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
                child = None
                try:
                    child = subprocess.Popen(argv, cwd=root, env=clean, stdin=subprocess.DEVNULL,
                                             stdout=stdout, stderr=stderr, start_new_session=os.name == "posix")
                    child.wait(timeout=60)
                    require(child.returncode == 0, "typed freeze consumer refused its inputs", "instrument_failure")
                    stdout.seek(0)
                    raw = stdout.read(store.Limits().json_bytes + 1)
                    require(len(raw) <= store.Limits().json_bytes, "typed consumer output exceeded its bound", "instrument_failure")
                    return store._object(raw)
                except (OSError, subprocess.TimeoutExpired):
                    raise store.StoreError("instrument_failure", "typed freeze consumer unavailable or timed out") from None
                finally:
                    if child is not None:
                        if os.name == "posix":
                            try:
                                os.killpg(child.pid, signal.SIGKILL)
                            except ProcessLookupError:
                                pass
                        elif child.poll() is None:
                            child.kill()
                        child.wait()
    return invoke


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=("prepare", "qualify"))
    parser.add_argument("--selection", type=Path, required=True)
    parser.add_argument("--cargo-allow", type=Path, required=True)
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--credential-fd", type=int, required=True,
                        help="explicit already-open read-only GitHub credential descriptor; no ambient fallback")
    args = parser.parse_args()
    try:
        require(args.credential_fd >= 3, "explicit credential descriptor >= 3 required")
        try:
            raw_token = os.read(args.credential_fd, 4097)
            token = raw_token.decode("ascii").rstrip("\r\n")
        except (OSError, UnicodeError):
            raise store.StoreError("invalid_input", "credential descriptor unavailable") from None
        require(0 < len(raw_token) <= 4096, "credential descriptor exceeds its bound")
        _, selection = read_json(args.selection)
        qualifier = Qualifier(selection, selection_dir=args.selection.parent, credential=lambda: token)
        result = qualifier.run(args.phase, args.out_dir.resolve(),
                               native_bridge(args.cargo_allow, args.repository_root))
        print(json.dumps(result, sort_keys=True))
        return 0 if args.phase == "prepare" or result.get("freeze_state") == "Complete" else 2
    except store.StoreError as error:
        print(json.dumps({"freeze_state":"Incomplete", "kind":error.kind, "detail":str(error)}), file=sys.stderr)
        return 2
    except (OSError, ValueError):
        print(json.dumps({"freeze_state":"Incomplete", "kind":"instrument_failure",
                          "detail":"selected local input, output or consumer is unavailable"}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
