#!/usr/bin/env python3
"""Qualify actual bounded CLI pages against a retained, unfiltered full queue."""

import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time
import unicodedata

CAP = 524288
LIMIT = 25
SCHEMA = "cargo-allow.worklist-page-coverage.v1"
PROBE = "crates/cargo-allow/src/agent_loop_probe.rs"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def decode(data):
    def pairs(rows):
        value = {}
        for key, item in rows:
            require(key not in value, f"duplicate JSON key: {key}")
            value[key] = item
        return value

    def bad_constant(value):
        raise ValueError(f"nonfinite JSON number: {value}")

    return json.loads(data, object_pairs_hook=pairs, parse_constant=bad_constant)


def integer(value):
    return type(value) is int and value >= 0


def source_identity_path(text):
    # Reports normalize native separators/verbatim roots; invocation argv must
    # retain its actual spelling. Compare their lexical identities only here.
    text = unicodedata.normalize("NFC", text.replace("\\", "/"))
    if text.startswith("//?/UNC/"):
        text = "//" + text[8:]
    elif text.startswith("//?/"):
        text = text[4:]
    elif text.startswith("/?/"):
        text = text[3:]
    absolute = text.startswith("/")
    parts = []
    for part in text.split("/"):
        if part in ("", "."):
            continue
        if part == ".." and parts and parts[-1] != "..":
            parts.pop()
        elif part != ".." or not absolute:
            parts.append(part)
    return ("/" if absolute else "") + "/".join(parts)


def keys(value, expected):
    require(type(value) is dict and set(value) == set(expected.split()), "unexpected contract fields")


def summary(items):
    value = {
        "work_items": len(items),
        **{risk: sum(item["risk"] == risk for item in items) for risk in ("high", "medium", "low")},
        **{f"{level}_difficulty": sum(item["difficulty"] == level for item in items)
           for level in ("small", "medium")},
    }
    if items:
        value["item_kinds"] = dict(Counter(item["item_kind"] for item in items))
    return value


def population(raw):
    value = decode(raw)
    require(value.get("schema_id") == "cargo-allow.worklist.v1", "wrong worklist schema")
    require(type(value.get("schema_version")) is int and value["schema_version"] == 1,
            "wrong worklist version")
    require(value.get("command") == "worklist" and value.get("tool") == "cargo-allow", "wrong producer")
    require("paging" not in value, "full population must be unpaged")
    require(value["inventory"]["source"] == "git_tracked", "unexpected source inventory")
    keys(value["filters"], "kind family item_kind status allow_id path source_package owner classification "
         "baseline_debt broad_scope risk difficulty missing_evidence broken_evidence weak_evidence")
    require(all(item is None or item is False for item in value["filters"].values()), "filtered population")
    items = value["work_items"]
    require(type(items) is list, "work_items must be an array")
    ids = [item["id"] for item in items]
    require(all(type(item) is str and item for item in ids) and len(set(ids)) == len(ids), "duplicate/invalid work IDs")
    require(encoded(value["summary"]) == encoded(summary(items)), "full summary drift")
    return value


def page_matches(full, raw, offset):
    require(len(raw) <= CAP, "worklist page exceeds 524288 bytes")
    page = decode(raw)
    total = len(full["work_items"])
    require(encoded(page.get("paging")) == encoded({"limit": LIMIT, "offset": offset,
            "total": total, "unfiltered_total": total}), "paging identity drift")
    require(encoded(page["work_items"]) == encoded(full["work_items"][offset:offset + LIMIT]),
            "page omission, duplication, reordering or item drift")
    require(encoded(page["summary"]) == encoded(summary(page["work_items"])), "page summary drift")
    ignored = {"work_items", "summary", "paging"}
    require(encoded({k: v for k, v in page.items() if k not in ignored}) ==
            encoded({k: v for k, v in full.items() if k not in ignored}), "page context drift")
    return page


def validate_coverage(raw, read_member, ceiling=60000):
    """Rehash every member and compare complete objects, not just page IDs/counts."""
    require(len(raw) <= CAP, "coverage companion exceeds 524288 bytes")
    record = decode(raw)
    keys(record, "schema_id schema_version result limit byte_cap ceiling_ms source_before source_after "
         "full pages total counts sample_page sample_elapsed_ms routing_probe routing_pages "
         "traversal_elapsed_ms qualification_elapsed_ms invocation")
    require(record["schema_id"] == SCHEMA and type(record["schema_version"]) is int and
            record["schema_version"] == 1 and record["result"] == "passed", "coverage not passed")
    require(type(record["limit"]) is int and record["limit"] == LIMIT and
            type(record["byte_cap"]) is int and record["byte_cap"] == CAP, "coverage bounds drift")
    require(type(record["ceiling_ms"]) is int and record["ceiling_ms"] == ceiling, "time ceiling drift")
    before, after = record["source_before"], record["source_after"]
    keys(before, "head parent tracked_sha256 policy_sha256 binary_sha256 utc_day")
    require(encoded(before) == encoded(after), "source, binary or UTC date changed during coverage")
    require(re.fullmatch(r"[0-9a-f]{40}", before["head"]) is not None, "invalid source head")
    require(before["parent"] is None or re.fullmatch(r"[0-9a-f]{40}", before["parent"]) is not None,
            "invalid source parent")
    for field in ("tracked_sha256", "policy_sha256", "binary_sha256"):
        require(re.fullmatch(r"[0-9a-f]{64}", before[field]) is not None, "invalid source digest")
    require(datetime.strptime(before["utc_day"], "%Y-%m-%d").date().isoformat() == before["utc_day"], "invalid UTC day")
    invocation = record["invocation"]
    keys(invocation, "binary root cwd")
    require(all(type(value) is str and value for value in invocation.values()), "invalid invocation")

    def member(row, path, offset=None):
        keys(row, "path sha256 bytes argv exit_code elapsed_ms")
        require(row["path"] == path, "member path/ordering drift")
        expected = [invocation["binary"], "worklist", "--format", "json"]
        if offset is not None:
            expected += ["--limit", str(LIMIT), "--offset", str(offset)]
        expected += ["--output", path, "--root", invocation["root"]]
        require(row["argv"] == expected, "worklist command must be unfiltered and exact")
        require(type(row["exit_code"]) is int and row["exit_code"] == 0, "worklist command failed")
        require(integer(row["elapsed_ms"]) and row["elapsed_ms"] <= ceiling, "command latency exceeds ceiling")
        data = read_member(path)
        require(integer(row["bytes"]) and row["bytes"] == len(data) and row["sha256"] == digest(data),
                "retained member bytes/hash drift")
        return data

    full = population(member(record["full"], "full.json"))
    require(source_identity_path(full["inventory"]["root"]) == source_identity_path(invocation["root"]),
            "source root drift")
    total = len(full["work_items"])
    require(type(record["total"]) is int and record["total"] == total, "population total drift")
    counts = {field: dict(Counter(item[field] for item in full["work_items"]))
              for field in ("status", "item_kind")}
    require(encoded(record["counts"]) == encoded(counts), "population counts drift")
    offsets = list(range(0, total, LIMIT)) or [0]
    require(type(record["pages"]) is list and len(record["pages"]) == len(offsets), "missing/extra page")
    probe = record["routing_probe"]
    require(probe in (None, PROBE), "unexpected routing target")
    routed = None
    for index, (row, offset) in enumerate(zip(record["pages"], offsets)):
        page = page_matches(full, member(row, f"page-{offset:06d}.json", offset), offset)
        if probe and routed is None and any(item.get("path") == probe and
                item.get("item_kind") == "new_unreceipted_finding" for item in page["work_items"]):
            routed = index + 1
    require(not probe or routed is not None, "routing probe absent")
    route_pages = routed if probe else 1
    require(type(record["routing_pages"]) is int and record["routing_pages"] == route_pages, "routing coverage drift")
    require(record["sample_page"] == record["pages"][route_pages - 1]["path"], "sample must bind the observed page")
    elapsed = record["sample_elapsed_ms"]
    route_sum = sum(row["elapsed_ms"] for row in record["pages"][:route_pages])
    require(integer(elapsed) and route_sum <= elapsed <= ceiling, "routing/sample latency invalid")
    if not probe:
        require(elapsed == record["pages"][0]["elapsed_ms"], "first-page sample latency drift")
    traversal = record["traversal_elapsed_ms"]
    require(integer(traversal) and traversal >= sum(row["elapsed_ms"] for row in record["pages"])
            and traversal >= elapsed, "full traversal cost understated")
    qualification = record["qualification_elapsed_ms"]
    require(integer(qualification) and qualification >= record["full"]["elapsed_ms"] + traversal,
            "qualification cost understated")
    return record


def validate_sample(sample, artifact_root, ceiling, binary_sha256, repository_head):
    companion = artifact_root / sample["semantic_artifact"]["path"]
    require(not companion.is_symlink(), "symlink companion")
    def read_member(name):
        path = companion.parent / name
        require(not path.is_symlink(), "symlink member")
        return path.read_bytes()
    raw = companion.read_bytes()
    record = validate_coverage(raw, read_member, ceiling)
    require(record["source_before"]["binary_sha256"] == binary_sha256, "binary identity drift")
    commit_field = "parent" if sample["name"] == "agent_loop_worklist" else "head"
    require(record["source_before"][commit_field] == repository_head, "source commit drift")
    require(sample["elapsed_ms"] == record["sample_elapsed_ms"], "sample elapsed drift")
    expected_probe = PROBE if sample["name"] == "agent_loop_worklist" else None
    require(record["routing_probe"] == expected_probe, "sample routing target drift")
    require((artifact_root / sample["artifact"]["path"]).resolve() ==
            (companion.parent / record["sample_page"]).resolve(), "sample page drift")
    require(sample["argv"] == ["qualify-worklist-pages", "--limit", str(LIMIT),
            "--routing-probe", expected_probe or "none"], "sample command drift")
    require(sample["semantic_payload_bytes"] == len(raw) and
            sample["semantic_artifact"]["sha256"] == digest(raw), "sample companion drift")
    selected = read_member(record["sample_page"])
    require(sample["payload_bytes"] == len(selected) and sample["artifact"]["sha256"] == digest(selected),
            "sample page bytes/hash drift")
    return record


def snapshot(root, binary, env):
    def git(*args):
        return subprocess.check_output(["git", "-C", str(root), *args], env=env)
    paths = git("ls-files", "-z").split(b"\0")
    hashed = hashlib.sha256()
    for name in sorted(path for path in paths if path):
        path = root / os.fsdecode(name)
        require(not path.is_symlink() and path.is_file(), "tracked input is missing or a symlink")
        hashed.update(name + b"\0" + hashlib.sha256(path.read_bytes()).digest())
    # Root and depth-one ordinary checkouts have no available parent. The
    # agent-loop consumer separately requires its real mutation parent.
    ancestry = git("rev-list", "--parents", "-n", "1", "HEAD").decode().split()
    return {"head": ancestry[0], "parent": ancestry[1] if len(ancestry) > 1 else None,
            "tracked_sha256": hashed.hexdigest(),
            "policy_sha256": digest((root / "policy/allow.toml").read_bytes()),
            "binary_sha256": digest(binary.read_bytes()),
            "utc_day": datetime.now(timezone.utc).date().isoformat()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--ceiling-ms", type=int, default=60000)
    parser.add_argument("--routing-probe", choices=[PROBE])
    args = parser.parse_args()
    root, binary, output = args.root.resolve(), args.binary.resolve(), args.output_dir.resolve()
    require(output.is_relative_to(root), "outputs must remain inside the source tree")
    require(not output.exists(), "refusing to replace retained qualification evidence")
    output.mkdir(parents=True)
    env = {k: v for k, v in os.environ.items() if k not in ("GIT_DIR", "GIT_WORK_TREE")}
    started = time.monotonic_ns()
    before = snapshot(root, binary, env)

    def command(name, offset=None):
        argv = ["worklist", "--format", "json"]
        if offset is not None:
            argv += ["--limit", str(LIMIT), "--offset", str(offset)]
        argv += ["--output", name]
        # Output paths are relative to this source-root subdirectory; retain
        # the actual argv and cwd separately from portable member references.
        actual = [str(binary), *argv, "--root", str(root)]
        start = time.monotonic_ns()
        with (output / (name + ".stdout")).open("wb") as stdout, (output / (name + ".stderr")).open("wb") as stderr:
            result = subprocess.run(actual, cwd=output, env=env, stdout=stdout, stderr=stderr,
                                    timeout=args.ceiling_ms / 1000 + 1, check=False)
        elapsed = (time.monotonic_ns() - start) // 1_000_000
        require(result.returncode == 0, f"worklist failed; retained {name}.stderr")
        raw = (output / name).read_bytes()
        return {"path": name, "sha256": digest(raw), "bytes": len(raw), "argv": actual,
                "exit_code": result.returncode, "elapsed_ms": elapsed}, raw

    full_row, full_raw = command("full.json")
    full = population(full_raw)
    total = len(full["work_items"])
    pages, routed, routing_elapsed = [], None, None
    traversal_start = time.monotonic_ns()
    for offset in range(0, max(1, total), LIMIT):
        row, raw = command(f"page-{offset:06d}.json", offset)
        pages.append(row)
        page = page_matches(full, raw, offset)
        if args.routing_probe and routed is None and any(item.get("path") == args.routing_probe and
                item.get("item_kind") == "new_unreceipted_finding" for item in page["work_items"]):
            routed = len(pages)
            routing_elapsed = (time.monotonic_ns() - traversal_start) // 1_000_000
    traversal_elapsed = (time.monotonic_ns() - traversal_start) // 1_000_000
    require(not args.routing_probe or routed is not None, "routing probe absent")
    after = snapshot(root, binary, env)
    route_pages = routed if args.routing_probe else 1
    record = {"schema_id": SCHEMA, "schema_version": 1, "result": "passed", "limit": LIMIT,
              "byte_cap": CAP, "ceiling_ms": args.ceiling_ms, "source_before": before, "source_after": after,
              "invocation": {"binary": str(binary), "root": str(root), "cwd": str(output)},
              "full": full_row, "pages": pages, "total": total,
              "counts": {field: dict(Counter(item[field] for item in full["work_items"]))
                         for field in ("status", "item_kind")},
              "sample_page": pages[route_pages - 1]["path"],
              "sample_elapsed_ms": routing_elapsed if args.routing_probe else pages[0]["elapsed_ms"],
              "routing_probe": args.routing_probe, "routing_pages": route_pages,
              "traversal_elapsed_ms": traversal_elapsed,
              "qualification_elapsed_ms": (time.monotonic_ns() - started) // 1_000_000}
    raw = encoded(record) + b"\n"
    validate_coverage(raw, lambda name: (output / name).read_bytes(), args.ceiling_ms)
    (output / "coverage.json").write_bytes(raw)
    print(f"{record['sample_page']}\t{record['sample_elapsed_ms']}")


if __name__ == "__main__":
    main()
