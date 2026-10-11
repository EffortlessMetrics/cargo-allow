#!/usr/bin/env python3
"""Collect bounded #3149 A-family observations from an explicitly supplied binary.

This is a transport harness, not a semantic evaluator. It runs each selected
case once, retains the command's own outputs (including absence), and snapshots
the controlled fixture. Native command-migration-evidence performs admission.
The emitted expected-context file must be pinned by its caller; this collector
is not a trusted-worker, installation, or release-qualification authority.
"""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys


JSON_LIMIT = 8 * 1024 * 1024
BINARY_LIMIT = 256 * 1024 * 1024
FIRST_FAMILY_COMMANDS = {"adopt", "doctor", "audit", "check"}
MAX_SELECTED_CASES = 29
SCHEMA = "cargo-allow.command-case-bundle.v1"


def digest(data):
    return "sha256:v1:" + hashlib.sha256(data).hexdigest()


def json_bytes(value):
    return (json.dumps(value, indent=2, ensure_ascii=False, sort_keys=True) + "\n").encode()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON key: " + key)
        result[key] = value
    return result


def load_json(path):
    raw = regular_bytes(path, JSON_LIMIT)
    return json.loads(raw, object_pairs_hook=unique_object), raw


def utc_now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat().replace("+00:00", "Z")


def reject_symlinks(path):
    if not path.is_absolute() or ".." in path.parts:
        raise ValueError("an absolute, unambiguous path is required: " + str(path))
    for item in reversed((path,) + tuple(path.parents)):
        if item.is_symlink():
            raise ValueError("symlink path refused: " + str(item))


def regular_bytes(path, limit):
    reject_symlinks(path)
    with path.open("rb") as handle:
        before = os.fstat(handle.fileno())
        if not stat.S_ISREG(before.st_mode) or before.st_size > limit:
            raise ValueError("not a bounded regular file: " + str(path))
        data = handle.read(limit + 1)
        after = os.fstat(handle.fileno())
    reject_symlinks(path)
    named = path.stat()
    if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) != (
        after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns
    ) or (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns) != (
        named.st_dev, named.st_ino, named.st_size, named.st_mtime_ns
    ) or len(data) != before.st_size:
        raise ValueError("file changed while reading: " + str(path))
    return data


def safe_relative(value):
    if not value or "\\" in value or ":" in value or any(
        part in ("", ".", "..") for part in value.split("/")
    ):
        raise ValueError("unsafe member path: " + repr(value))
    return value


class Store:
    def __init__(self, root):
        self.root = root

    def put(self, relative, data):
        safe_relative(relative)
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        reject_symlinks(path.parent)
        with path.open("xb") as handle:
            handle.write(data)
        observed = regular_bytes(path, max(JSON_LIMIT, len(data)))
        if observed != data:
            raise ValueError("retained member readback mismatch: " + relative)
        return {"path": relative, "size_bytes": len(data), "digest": digest(data)}

    def capture(self, relative, source):
        return self.put(relative, regular_bytes(source, JSON_LIMIT))


def owned_root(path):
    path = path.absolute()
    reject_symlinks(path.parent)
    if any((parent / ".git").exists() for parent in (path.parent,) + tuple(path.parents)):
        raise ValueError("collection fixtures must be outside a repository checkout")
    path.mkdir()  # Exclusive ownership: existing files, directories and links all fail.
    return path.resolve()


def child_environment(home, git):
    home.mkdir()
    cargo_home = home / "cargo"
    cargo_home.mkdir()
    path = os.pathsep.join(dict.fromkeys([str(git.parent)] + os.defpath.split(os.pathsep)))
    env = {
        "PATH": path,
        "HOME": str(home),
        "USERPROFILE": str(home),
        "CARGO_HOME": str(cargo_home),
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_TERMINAL_PROMPT": "0",
        "GIT_OPTIONAL_LOCKS": "0",
        "LANG": "C",
        "LC_ALL": "C",
        "NO_COLOR": "1",
    }
    # Windows process creation needs its actual system directory. This is a
    # recorded child-local value, never an inherited policy or Git selector.
    if os.name == "nt":
        env["SYSTEMROOT"] = os.environ.get("SYSTEMROOT", "C:\\Windows")
        env["TEMP"] = str(home)
        env["TMP"] = str(home)
    return env


def run_process(argv, cwd, environment, timeout, store, prefix):
    out_path = store.root / (prefix + ".stdout")
    err_path = store.root / (prefix + ".stderr")
    out_path.parent.mkdir(parents=True, exist_ok=True)
    started_at = utc_now()
    started, exit_code, launch_error, timed_out = False, None, None, False
    with out_path.open("xb") as stdout, err_path.open("xb") as stderr:
        try:
            process = subprocess.Popen(
                argv, cwd=cwd, env=environment, stdin=subprocess.DEVNULL,
                stdout=stdout, stderr=stderr,
            )
            started = True
            try:
                exit_code = process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                timed_out = True
                process.kill()
                exit_code = process.wait()
        except OSError as error:
            launch_error = str(error)
    result = {
        "started_at_utc": started_at,
        "finished_at_utc": utc_now(),
        "started": started,
        "exit_code": exit_code,
        "launch_error": launch_error,
        "timed_out": timed_out,
    }
    members = []
    for path in (out_path, err_path):
        data = regular_bytes(path, JSON_LIMIT)
        members.append({
            "path": path.relative_to(store.root).as_posix(),
            "size_bytes": len(data), "digest": digest(data),
        })
    return result, members[0], members[1]


def setup_git(root, git, env, timeout, store, prefix):
    commands = [
        [str(git), "init", "--quiet"],
        [str(git), "add", "--", "src/lib.rs"],
    ]
    if (root / "policy/allow.toml").exists():
        commands.append([str(git), "add", "--", "policy/allow.toml"])
    commands.append([
        str(git), "-c", "user.name=command-case-fixture",
        "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false",
        "commit", "--quiet", "--message", "controlled command fixture",
    ])
    setup_env = dict(env)
    setup_env["GIT_AUTHOR_DATE"] = "2000-01-01T00:00:00Z"
    setup_env["GIT_COMMITTER_DATE"] = "2000-01-01T00:00:00Z"
    for number, argv in enumerate(commands):
        result, stdout, stderr = run_process(
            argv, root, setup_env, timeout, store, f"{prefix}/git-{number}"
        )
        store.put(f"{prefix}/git-{number}.json", json_bytes({
            "argv": argv, "cwd": str(root), "environment": setup_env,
            "process": result, "stdout": stdout, "stderr": stderr,
        }))
        if result["exit_code"] != 0:
            raise ValueError("Git fixture setup failed; raw setup members retained")
    result, stdout, stderr = run_process(
        [str(git), "rev-parse", "--verify", "HEAD"], root, env, timeout,
        store, prefix + "/git-head",
    )
    if result["exit_code"] != 0:
        raise ValueError("Git fixture HEAD observation failed")
    head = regular_bytes(store.root / stdout["path"], JSON_LIMIT).decode("ascii").strip()
    if len(head) not in (40, 64) or any(c not in "0123456789abcdef" for c in head):
        raise ValueError("Git returned an invalid object identity")
    store.put(prefix + "/git-head.json", json_bytes({
        "argv": [str(git), "rev-parse", "--verify", "HEAD"],
        "cwd": str(root), "environment": env, "process": result,
        "stdout": stdout, "stderr": stderr,
    }))
    return head


def snapshot(root, store, prefix):
    entries = []
    for parent, directories, files in os.walk(root, followlinks=False):
        parent = Path(parent)
        if parent == root:
            directories[:] = [name for name in directories if name != "target"]
        directories.sort()
        for name in sorted(directories + files):
            path = parent / name
            relative = path.relative_to(root).as_posix()
            safe_relative(relative)
            metadata = path.lstat()
            entry = {
                "path": relative, "kind": "other",
                "mode": stat.S_IMODE(metadata.st_mode), "size_bytes": 0,
                "content": None, "link_target": None,
            }
            if stat.S_ISLNK(metadata.st_mode):
                entry["kind"] = "symlink"
                entry["link_target"] = os.readlink(path)
            elif stat.S_ISDIR(metadata.st_mode):
                entry["kind"] = "directory"
            elif stat.S_ISREG(metadata.st_mode):
                entry["kind"] = "file"
                entry["content"] = store.capture(prefix + "/files/" + relative, path)
                entry["size_bytes"] = entry["content"]["size_bytes"]
            entries.append(entry)
            if len(entries) > 512:
                raise ValueError("controlled fixture exceeds the bounded snapshot")
    entries.sort(key=lambda item: item["path"])
    return store.put(prefix + "/snapshot.json", json_bytes({"entries": entries}))


def capture_optional(store, prefix, role, path, blocked_directory=False):
    reject_symlinks(path)
    if not path.exists() and not path.is_symlink():
        return None
    if blocked_directory and path.is_dir():
        # Only this deliberately owned directory represents expected absence.
        # Other nonregular paths fail collection; they are not silent absence.
        return None
    if not path.is_file():
        raise ValueError("unexpected nonregular output: " + str(path))
    return store.capture(prefix + "/" + role + ".json", path)


def collect_case(spec, fixture, root, store, binary, git, timeout):
    case_id = spec["id"]
    prefix = "cases/" + safe_relative(case_id)
    fixture_root = root / "fixtures" / case_id
    fixture_root.mkdir(parents=True)
    (fixture_root / "src").mkdir()
    (fixture_root / "src/lib.rs").write_bytes(fixture["source"].encode("utf-8"))
    if fixture["policy"] is not None:
        (fixture_root / "policy").mkdir()
        (fixture_root / "policy/allow.toml").write_bytes(fixture["policy"].encode("utf-8"))
    env = child_environment(root / "homes" / case_id, git)
    fixture_commit = setup_git(fixture_root, git, env, timeout, store, prefix + "/setup")
    outputs = fixture_root / "target/command-case"
    outputs.mkdir(parents=True)
    detail_path, summary_path = outputs / "detail.json", outputs / "summary.json"
    receipt_path = outputs / "receipt.json"
    output_guard = None
    if fixture["output_failure"]:
        detail_path.mkdir()
        (detail_path / "prior-owner.txt").write_bytes(b"prior owner\n")
        output_guard = {
            "before": store.capture(prefix + "/blocked-output-before.txt", detail_path / "prior-owner.txt"),
            "after": None,
        }
    argv = [
        str(binary), "--command-summary-output", str(summary_path), spec["command"],
        "--root", str(fixture_root), "--format", "json", "--output", str(detail_path),
    ]
    if fixture["policy"] is not None:
        argv.extend(["--config", "policy/allow.toml"])
    if spec["command"] == "check":
        argv.extend([
            "--mode", "no-new", "--persistent-cache", "off", "--receipt", str(receipt_path)
        ])
    if spec["command"] == "doctor" and fixture["require_clean"]:
        argv.append("--require-clean")
    before = snapshot(fixture_root, store, prefix + "/before")
    process, stdout, stderr = run_process(
        argv, fixture_root, env, timeout, store, prefix + "/command"
    )
    after = snapshot(fixture_root, store, prefix + "/after")
    detail = capture_optional(store, prefix, "detail", detail_path, fixture["output_failure"])
    summary = capture_optional(store, prefix, "summary", summary_path)
    receipt = capture_optional(store, prefix, "receipt", receipt_path)
    if output_guard is not None:
        output_guard["after"] = store.capture(
            prefix + "/blocked-output-after.txt", detail_path / "prior-owner.txt"
        )
    output_digests = {role: None if member is None else member["digest"] for role, member in [
        ("stdout", stdout), ("stderr", stderr), ("detail", detail), ("summary", summary), ("receipt", receipt)
    ]}
    context = {
        "case_id": case_id, "root": str(fixture_root), "cwd": str(fixture_root),
        "argv": argv, "environment": env, "source_snapshot_digest": before["digest"],
        "fixture_commit": fixture_commit, "output_digests": output_digests,
        "policy_digest": None if fixture["policy"] is None else digest(fixture["policy"].encode()),
        "config_path": None if fixture["policy"] is None else "policy/allow.toml",
        "mode": "no-new" if spec["command"] == "check" else None,
        "profile": None, "resolved_config_identity": None,
    }
    return {
        "context": context, "process": process, "before": before, "after": after,
        "stdout": stdout, "stderr": stderr, "detail": detail, "summary": summary, "receipt": receipt,
        "output_guard": output_guard,
    }


def collect(args):
    catalogue, catalogue_bytes = load_json(args.catalogue.absolute())
    if catalogue.get("schema_id") != "cargo-allow.command-migration-catalogue.v1" or catalogue.get("schema_version") != 1:
        raise ValueError("unsupported catalogue generation")
    specs = {item["id"]: item for item in catalogue["cases"]}
    if len(specs) != len(catalogue["cases"]):
        raise ValueError("duplicate catalogue case")
    selected = args.case or [item["id"] for item in catalogue["cases"] if item["first_family_collector"]]
    if len(set(selected)) != len(selected) or not selected:
        raise ValueError("case selection is empty or duplicated")
    if any(case not in specs or not specs[case]["first_family_collector"] for case in selected):
        raise ValueError("unknown or unimplemented case requested")
    # This is an execution boundary, not semantic admission of the catalogue.
    # A rewritten first-family flag must not dispatch a later mutation/release
    # command before the native reader can reject its denominator.
    if len(selected) > MAX_SELECTED_CASES or any(
        specs[case]["family"] != "A" or specs[case]["command"] not in FIRST_FAMILY_COMMANDS
        for case in selected
    ):
        raise ValueError("selection exceeds the bounded read-only A-family transport")
    binary, git = args.binary.absolute(), args.git.absolute()
    if not args.binary.is_absolute() or not args.git.is_absolute():
        raise ValueError("--binary and --git must be explicit absolute paths")
    binary_bytes = regular_bytes(binary, BINARY_LIMIT)
    if not binary_bytes:
        raise ValueError("supplied binary is empty")
    regular_bytes(git, BINARY_LIMIT)
    if args.provenance == "supplied_installed_candidate" and not (args.candidate_identity and args.install_identity):
        raise ValueError("a supplied installed candidate requires explicit candidate and install identities")
    if args.provenance == "source_build" and (args.candidate_identity or args.install_identity):
        raise ValueError("a source build cannot claim installed-candidate identities")
    root = owned_root(args.output_dir)
    store = Store(root)
    (root / "homes").mkdir()
    binary_member = store.put("binary/cargo-allow", binary_bytes)
    binary_context = {
        "path": str(binary), "size_bytes": len(binary_bytes), "digest": digest(binary_bytes),
        "tool_version": args.tool_version, "provenance": args.provenance,
        "source_generation": args.source_generation, "candidate_identity": args.candidate_identity,
        "install_identity": args.install_identity,
    }
    store.put("catalogue.json", catalogue_bytes)
    cases = []
    for case_id in selected:
        spec = specs[case_id]
        cases.append(collect_case(
            spec, catalogue["first_family_fixtures"][spec["scenario"]], root, store,
            binary, git, args.timeout,
        ))
    context = {
        "schema_id": "cargo-allow.command-case-context.v1", "schema_version": 1,
        "collection_id": args.collection_id, "catalogue_digest": digest(catalogue_bytes),
        "binary": binary_context, "cases": [case["context"] for case in cases],
    }
    context_member = store.put("expected-context.json", json_bytes(context))
    bundle = {
        "schema_id": SCHEMA, "schema_version": 1, "collection_id": args.collection_id,
        "catalogue_digest": digest(catalogue_bytes), "context_digest": context_member["digest"],
        "binary": binary_context, "binary_member": binary_member,
        "binary_digest_after": digest(regular_bytes(binary, BINARY_LIMIT)), "cases": cases,
    }
    store.put("bundle.json", json_bytes(bundle))
    print(json.dumps({
        "bundle": str(root / "bundle.json"), "expected_context": str(root / "expected-context.json"),
        "selected_cases": len(cases), "catalogue_cases": len(catalogue["cases"]),
        "semantic_admission": "not_performed", "qualification": "not_claimed",
    }, sort_keys=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--git", type=Path, required=True)
    parser.add_argument("--catalogue", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--collection-id", required=True)
    parser.add_argument("--tool-version", required=True)
    parser.add_argument("--source-generation", required=True)
    parser.add_argument("--provenance", choices=["source_build", "supplied_installed_candidate"], required=True)
    parser.add_argument("--candidate-identity")
    parser.add_argument("--install-identity")
    parser.add_argument("--case", action="append")
    parser.add_argument("--timeout", type=float, default=30.0)
    args = parser.parse_args()
    if args.timeout <= 0 or args.timeout > 300:
        parser.error("--timeout must be greater than zero and at most 300 seconds")
    try:
        collect(args)
    except (OSError, ValueError, KeyError, TypeError) as error:
        print("command-case collection failed: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
