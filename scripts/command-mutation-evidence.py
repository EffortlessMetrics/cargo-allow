#!/usr/bin/env python3
"""Collect bounded #4725 C-family mutation observations from an explicit binary.

This is the first stateful case-execution transport slice. Like the A-family
collector it is a transport harness, not a semantic evaluator: it runs each
selected case's ordered mutation steps once inside newly owned disposable Git
fixtures, retains the command's own outputs (including absence), snapshots the
controlled fixture around every step, and classifies each changed path as a
declared or undeclared effect. It never approves exceptions, never relaxes
policy, and never executes repository code. Admission is performed by
scripts/verify-command-mutation-evidence.py against a separately pinned
expected context; this collector is not an installation, qualification, or
release authority.

Every fixture is created outside any repository checkout, exercised, captured,
and discarded; the retained bundle keeps the captured bytes and digests.
"""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys


JSON_LIMIT = 8 * 1024 * 1024
BINARY_LIMIT = 512 * 1024 * 1024
SNAPSHOT_ENTRY_LIMIT = 512
SCHEMA = "cargo-allow.command-mutation-bundle.v1"
CONTEXT_SCHEMA = "cargo-allow.command-mutation-context.v1"
MUTATION_COMMANDS = {"init", "propose", "add", "add_from_plan"}
C_FAMILY_PREFIXES = ("C.init.", "C.propose.", "C.add.", "C.add_from_plan.")

SOURCE_BYTES = (
    "pub fn a() {\n    panic!(\"intentional fixture finding\");\n}\n"
)
FINDING = {"kind": "panic", "path": "src/lib.rs", "line": 2}
ADD_BASE = [
    "--kind", "panic", "--path", "src/lib.rs", "--line", "2",
    "--owner", "mutation-fixture",
    "--reason", "controlled #4725 mutation fixture",
    "--classification", "baseline_debt",
    "--expires", "2027-01-11", "--review-after", "2026-12-15",
]
PLAN_OWNER = ["--owner", "mutation-fixture", "--reason", "controlled #4725 plan apply"]


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
    named_before = path.lstat()
    if not stat.S_ISREG(named_before.st_mode) or named_before.st_size > limit:
        raise ValueError("not a bounded regular file: " + str(path))
    flags = os.O_RDONLY | getattr(os, "O_BINARY", 0)
    if os.name == "posix":
        if not hasattr(os, "O_NONBLOCK") or not hasattr(os, "O_NOFOLLOW"):
            raise ValueError("bounded regular file opening is unsupported on this host")
        flags |= os.O_NONBLOCK | os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    with os.fdopen(descriptor, "rb") as handle:
        before = os.fstat(handle.fileno())
        if not stat.S_ISREG(before.st_mode) or before.st_size > limit:
            raise ValueError("not a bounded regular file: " + str(path))
        if file_identity(named_before, cross_view=True) != file_identity(before, cross_view=True):
            raise ValueError("file changed before open: " + str(path))
        data = handle.read(limit + 1)
        after = os.fstat(handle.fileno())
    reject_symlinks(path)
    named = path.lstat()
    if (file_identity(before) != file_identity(after)
            or file_identity(named_before) != file_identity(named)
            or file_identity(after, cross_view=True) != file_identity(named, cross_view=True)
            or len(data) != before.st_size):
        raise ValueError("file changed while reading: " + str(path))
    return data


def file_identity(metadata, *, cross_view=False):
    mode = metadata.st_mode
    if cross_view and os.name == "nt":
        mode &= ~0o111
    return (metadata.st_dev, metadata.st_ino, mode, metadata.st_nlink,
            metadata.st_size, metadata.st_mtime_ns)


def safe_relative(value):
    if not value or "\\" in value or ":" in value or any(
        part in ("", ".", "..") for part in value.split("/")
    ):
        raise ValueError("unsafe member path: " + repr(value))
    return value


class Store:
    """Retains members once; existing members are never overwritten."""

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
    if "%" in str(path):
        raise ValueError("collection root cannot contain an LLVM profile filename pattern")
    if any((parent / ".git").exists() for parent in (path.parent,) + tuple(path.parents)):
        raise ValueError("collection fixtures must be outside a repository checkout")
    path.mkdir()  # Exclusive ownership: existing files, directories and links all fail.
    return path.resolve()


def child_environment(home, git):
    if not git.is_absolute():
        raise ValueError("child Git executable must have an absolute path")
    defaults = [entry for entry in os.defpath.split(os.pathsep) if Path(entry).is_absolute()]
    path = os.pathsep.join(dict.fromkeys([str(git.parent)] + defaults))
    home.mkdir()
    cargo_home = home / "cargo"
    cargo_home.mkdir()
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
                argv, cwd=str(cwd), env=environment, stdin=subprocess.DEVNULL,
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


def snapshot(root, store, prefix):
    """Byte-level observation of the controlled fixture outside target/."""
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
            else:
                raise ValueError("unexpected special file in controlled fixture: " + relative)
            entries.append(entry)
            if len(entries) > SNAPSHOT_ENTRY_LIMIT:
                raise ValueError("controlled fixture exceeds the bounded snapshot")
    entries.sort(key=lambda item: item["path"])
    return store.put(prefix + "/snapshot.json", json_bytes({"entries": entries}))


def logical_view(member, reader_bytes):
    data = json.loads(reader_bytes(member), object_pairs_hook=unique_object)
    view = {}
    for entry in data["entries"]:
        content = entry.get("content")
        view[entry["path"]] = (
            entry["kind"], entry["mode"], entry["size_bytes"],
            None if content is None else content["digest"], entry.get("link_target"),
        )
    return view


def changed_paths(before_member, after_member, reader_bytes):
    """Ordered, reproducible diff of two retained snapshots."""
    before = logical_view(before_member, reader_bytes)
    after = logical_view(after_member, reader_bytes)
    changes = []
    for path in sorted(set(before) | set(after)):
        old, new = before.get(path), after.get(path)
        if old == new:
            continue
        if old is None:
            changes.append({"path": path, "change": "added",
                            "before_digest": None, "after_digest": new[3]})
        elif new is None:
            changes.append({"path": path, "change": "removed",
                            "before_digest": old[3], "after_digest": None})
        else:
            changes.append({"path": path, "change": "modified",
                            "before_digest": old[3], "after_digest": new[3]})
    return changes


def capture_optional(store, prefix, role, path):
    reject_symlinks(path)
    if not path.exists() or path.is_dir():
        return None
    if not path.is_file():
        raise ValueError("unexpected nonregular output: " + str(path))
    return store.capture(prefix + "/" + role, path)


def git_setup(root, git, env, timeout, store, prefix, extra_tracked=()):
    commands = [
        [str(git), "init", "--quiet"],
        [str(git), "add", "--", "src/lib.rs"],
    ]
    for relative in extra_tracked:
        commands.append([str(git), "add", "--", relative])
    commands.append([
        str(git), "-c", "user.name=command-mutation-fixture",
        "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false",
        "commit", "--quiet", "--message", "controlled mutation fixture",
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


def build_fixture(recipe, fixture_root, binary, git, env, timeout, store, prefix):
    """Create one disposable Git fixture; retained, deterministic, owned.

    A recipe ledger is bootstrapped by the supplied binary before the single
    fixture commit. A committed ledger is tracked in that same commit so any
    later fixture-level rollback is a plain Git restore of retained bytes.
    """
    fixture_root.mkdir(parents=True)
    (fixture_root / "src").mkdir()
    (fixture_root / "src/lib.rs").write_bytes(SOURCE_BYTES.encode("utf-8"))
    extra_tracked = ()
    if recipe.get("ledger"):
        binary_argv = [str(binary), "init"]
        if recipe.get("strict"):
            binary_argv.append("--strict")
        result, _, _ = run_process(
            binary_argv, fixture_root, env, timeout, store, prefix + "/setup-init"
        )
        store.put(prefix + "/setup-init.json", json_bytes({
            "argv": binary_argv, "cwd": str(fixture_root), "environment": env,
            "process": result,
        }))
        if result["exit_code"] != 0 or not (fixture_root / "policy/allow.toml").is_file():
            raise ValueError("fixture ledger bootstrap failed; raw members retained")
        if recipe.get("commit_ledger"):
            extra_tracked = ("policy/allow.toml",)
    return git_setup(fixture_root, git, env, timeout, store, prefix + "/setup",
                     extra_tracked=extra_tracked)


def resolve_argv(template, binary, fixture_root):
    argv = []
    for part in template:
        if part.startswith("BINARY:"):
            argv.append(str(binary))
        elif part.startswith("ROOT:"):
            argv.append(str(fixture_root / part[5:]))
        else:
            argv.append(part)
    return argv


def run_case_step(step, fixture_root, binary, git, env, timeout, store, prefix, index):
    argv = resolve_argv(step["argv"], binary, fixture_root)
    before = snapshot(fixture_root, store, prefix + f"/step-{index}/before")
    result, stdout, stderr = run_process(
        argv, fixture_root, env, timeout, store, prefix + f"/step-{index}/command"
    )
    after = snapshot(fixture_root, store, prefix + f"/step-{index}/after")
    artifacts = []
    for role, relative in step.get("artifacts", {}).items():
        member = capture_optional(store, prefix + f"/step-{index}", role,
                                  fixture_root / relative)
        artifacts.append({"role": role, "requested_path": relative, "member": member})
    changes = changed_paths(before, after, lambda member: regular_bytes(
        store.root / member["path"], JSON_LIMIT))
    declared = set(step.get("declared_writes", []))
    for change in changes:
        change["declared"] = change["path"] in declared
    record = {
        "index": index,
        "purpose": step["purpose"],
        "route": step.get("route", "mutation"),
        "observation_class": step.get("observation_class", "outcome"),
        "argv": argv,
        "cwd": str(fixture_root),
        "process": result,
        "stdout": stdout,
        "stderr": stderr,
        "before": before,
        "after": after,
        "changed_paths": changes,
        "artifacts": artifacts,
        "expected_exit": step.get("expected_exit", "zero"),
    }
    store.put(prefix + f"/step-{index}/record.json", json_bytes(record))
    return record


def rollback_step(fixture_root, binary, git, env, timeout, store, prefix, index,
                  purpose, source_relative, target_relative):
    """Explicit fixture-level restore from retained bytes; never product proof."""
    before = snapshot(fixture_root, store, prefix + f"/step-{index}/before")
    source = fixture_root / source_relative
    reject_symlinks(source)
    target = fixture_root / target_relative
    reject_symlinks(target)
    launch_error = None
    data = None
    try:
        data = regular_bytes(source, JSON_LIMIT)
    except (OSError, ValueError) as error:
        launch_error = f"rollback state missing or unreadable: {error}"
    if data is not None:
        with target.open("wb") as handle:
            handle.write(data)
    after = snapshot(fixture_root, store, prefix + f"/step-{index}/after")
    changes = changed_paths(before, after, lambda member: regular_bytes(
        store.root / member["path"], JSON_LIMIT))
    for change in changes:
        change["declared"] = True  # Harness-owned restore of retained bytes.
    record = {
        "index": index,
        "purpose": purpose,
        "route": "fixture_rollback",
        "observation_class": "fixture_control",
        "argv": None,
        "cwd": str(fixture_root),
        "process": {
            "started_at_utc": utc_now(), "finished_at_utc": utc_now(),
            "started": data is not None,
            "exit_code": 0 if data is not None else None,
            "launch_error": launch_error, "timed_out": False,
        },
        "stdout": None, "stderr": None,
        "before": before, "after": after,
        "changed_paths": changes,
        "artifacts": [],
        "expected_exit": "zero",
    }
    store.put(prefix + f"/step-{index}/record.json", json_bytes(record))
    return record


CONTROLLED_ADD = ["BINARY:", "add", "--config", "ROOT:policy/allow.toml"] + ADD_BASE
CONTROLLED_PROPOSE = ["BINARY:", "propose", "--config", "ROOT:policy/allow.toml"]
CONTROLLED_WHY = [
    "BINARY:", "why", "--config", "ROOT:policy/allow.toml",
    "--kind", "panic", "--path", "src/lib.rs", "--line", "2",
]
PLAN_WITH_OWNER = ["BINARY:", "add", "--config", "ROOT:policy/allow.toml",
                   "--from-plan", "ROOT:target/add-plan.json"] + PLAN_OWNER


def c_cases():
    """The 20 accepted C-family mutation obligations of lane #4725."""
    return [
        {
            "id": "C.init.preview",
            "recipe": {},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "dry-run reports would-create guidance on a clean tree",
                 "route": "preview", "argv": ["BINARY:", "init", "--dry-run"],
                 "declared_writes": [], "expected_exit": "zero"},
                {"purpose": "selected alternate candidate bootstrap",
                 "route": "candidate_write",
                 "argv": ["BINARY:", "init", "--config", "ROOT:policy/allow.candidate.toml"],
                 "declared_writes": ["policy", "policy/allow.candidate.toml"],
                 "expected_exit": "zero"},
                {"purpose": "dry-run over an existing selected target keeps bytes and shows guidance",
                 "route": "preview",
                 "argv": ["BINARY:", "init", "--config", "ROOT:policy/allow.candidate.toml",
                          "--dry-run"],
                 "declared_writes": [], "expected_exit": "zero"},
            ],
        },
        {
            "id": "C.init.alternate_config",
            "recipe": {},
            "forbidden_paths": ["policy/allow.toml"],
            "steps": [
                {"purpose": "alternate-path bootstrap writes exactly the selected new path",
                 "route": "candidate_write",
                 "argv": ["BINARY:", "init", "--config", "ROOT:policy/allow.candidate.toml"],
                 "declared_writes": ["policy", "policy/allow.candidate.toml"],
                 "expected_exit": "zero"},
                {"purpose": "re-running bootstrap over the existing alternate refuses without --force",
                 "route": "refusal",
                 "argv": ["BINARY:", "init", "--config", "ROOT:policy/allow.candidate.toml"],
                 "declared_writes": [], "expected_exit": "nonzero"},
                {"purpose": "dry-run over the existing alternate target preserves bytes",
                 "route": "preview",
                 "argv": ["BINARY:", "init", "--config", "ROOT:policy/allow.candidate.toml",
                          "--dry-run"],
                 "declared_writes": [], "expected_exit": "zero"},
            ],
        },
        {
            "id": "C.init.apply",
            "recipe": {},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "strict init creation seeds distinct live policy bytes",
                 "route": "mutation", "argv": ["BINARY:", "init", "--strict"],
                 "declared_writes": ["policy", "policy/allow.toml"],
                 "expected_exit": "zero"},
                {"purpose": "--force replacement with backup and typed summary receipt",
                 "route": "mutation",
                 "argv": ["BINARY:", "init", "--force", "--command-summary-output",
                          "ROOT:target/init-summary.json"],
                 "declared_writes": ["policy/allow.toml", "policy/allow.toml.bak"],
                 "change_expectation": "optional",
                 "expected_exit": "zero",
                 "artifacts": {"summary": "target/init-summary.json"}},
                {"purpose": "supported rollback restores ledger from retained backup bytes",
                 "route": "fixture_rollback",
                 "fixture_control": {"action": "restore",
                                     "source": "policy/allow.toml.bak",
                                     "target": "policy/allow.toml"},
                 "declared_writes": ["policy/allow.toml"], "expected_exit": "zero"},
                {"purpose": "the ordinary gate re-exposes the unreceipted finding "
                            "after the restore",
                 "route": "proof", "observation_class": "outcome",
                 "argv": ["BINARY:", "check", "--config", "ROOT:policy/allow.toml",
                          "--mode", "no-new"],
                 "declared_writes": [], "expected_exit": "nonzero"},
            ],
        },
        {
            "id": "C.init.stale_conflict",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "existing ledger refuses init without --force and preserves bytes",
                 "route": "refusal", "argv": ["BINARY:", "init"],
                 "declared_writes": [], "expected_exit": "nonzero"},
                {"purpose": "selected existing config target refuses without --force",
                 "route": "refusal",
                 "argv": ["BINARY:", "init", "--config", "ROOT:policy/allow.toml"],
                 "declared_writes": [], "expected_exit": "nonzero"},
                {"purpose": "strict bootstrap over the existing ledger refuses",
                 "route": "refusal",
                 "argv": ["BINARY:", "init", "--config", "ROOT:policy/allow.toml",
                          "--strict"],
                 "declared_writes": [], "expected_exit": "nonzero"},
            ],
        },
        {
            "id": "C.init.presentation",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "human dry-run route over an existing ledger keeps bytes",
                 "route": "preview",
                 "argv": ["BINARY:", "init", "--dry-run",
                          "--config", "ROOT:policy/allow.toml"],
                 "declared_writes": [], "expected_exit": "zero"},
                {"purpose": "typed common-summary route evaluates the same outcome",
                 "route": "preview",
                 "argv": ["BINARY:", "init", "--dry-run",
                          "--config", "ROOT:policy/allow.toml",
                          "--command-summary-output", "ROOT:target/init-summary.json"],
                 "declared_writes": [], "expected_exit": "zero",
                 "artifacts": {"summary": "target/init-summary.json"}},
                {"purpose": "quiet human route evaluates the same outcome",
                 "route": "preview",
                 "argv": ["BINARY:", "init", "--dry-run", "-q",
                          "--config", "ROOT:policy/allow.toml"],
                 "declared_writes": [], "expected_exit": "zero"},
                {"purpose": "explicit color styling changes presentation, not evaluation",
                 "route": "preview",
                 "argv": ["BINARY:", "init", "--dry-run", "--color", "always",
                          "--config", "ROOT:policy/allow.toml"],
                 "declared_writes": [], "expected_exit": "zero"},
                {"purpose": "unsupported profile value is refused without mutation",
                 "route": "refusal",
                 "argv": ["BINARY:", "init", "--dry-run", "--profile", "nonexistent"],
                 "declared_writes": [], "expected_exit": "nonzero"},
            ],
        },
        {
            "id": "C.propose.preview",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "proposal without --write streams candidate bytes with no mutation",
                 "route": "preview", "argv": list(CONTROLLED_PROPOSE),
                 "declared_writes": [], "expected_exit": "zero"},
                {"purpose": "quiet proposal route evaluates the same outcome",
                 "route": "preview", "argv": CONTROLLED_PROPOSE + ["-q"],
                 "declared_writes": [], "expected_exit": "zero"},
            ],
        },
        {

            "id": "C.propose.candidate",
            "recipe": {"ledger": True},
            "forbidden_paths": ["policy/candidate-2.toml.bak"],
            "steps": [
                {"purpose": "candidate write owns exactly the requested new path",
                 "route": "candidate_write",
                 "argv": CONTROLLED_PROPOSE + ["--write", "ROOT:policy/candidate.toml"],
                 "declared_writes": ["policy/candidate.toml"], "expected_exit": "zero"},
                {"purpose": "second candidate path with typed JSON summary ownership",
                 "route": "candidate_write",
                 "argv": CONTROLLED_PROPOSE + ["--write", "ROOT:policy/candidate-2.toml",
                                               "--summary-format", "json",
                                               "--summary-output",
                                               "ROOT:target/propose-summary.json"],
                 "declared_writes": ["policy/candidate-2.toml"], "expected_exit": "zero",
                 "artifacts": {"summary": "target/propose-summary.json"}},
            ],
        },
        {
            "id": "C.propose.replacement_boundary",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "seed candidate file for the replacement boundary",
                 "route": "candidate_write",
                 "argv": CONTROLLED_PROPOSE + ["--write", "ROOT:policy/candidate.toml"],
                 "declared_writes": ["policy/candidate.toml"], "expected_exit": "zero"},
                {"purpose": "existing candidate refuses replacement without --force",
                 "route": "refusal",
                 "argv": CONTROLLED_PROPOSE + ["--write", "ROOT:policy/candidate.toml"],
                 "declared_writes": [], "expected_exit": "nonzero"},
                {"purpose": "--force replaces the non-ledger candidate and retains recovery bytes",
                 "route": "mutation",
                 "argv": CONTROLLED_PROPOSE + ["--write", "ROOT:policy/candidate.toml",
                                               "--force"],
                 "declared_writes": ["policy/candidate.toml", "policy/candidate.toml.bak"],
                 "expected_exit": "zero"},
                {"purpose": "--force at the live ledger path replaces the current ledger itself",
                 "route": "mutation",
                 "argv": CONTROLLED_PROPOSE + ["--write", "ROOT:policy/allow.toml", "--force"],
                 "declared_writes": ["policy/allow.toml", "policy/allow.toml.bak"],
                 "expected_exit": "zero"},
                {"purpose": "supported rollback restores the ledger from its retained backup",
                 "route": "fixture_rollback",
                 "fixture_control": {"action": "restore",
                                     "source": "policy/allow.toml.bak",
                                     "target": "policy/allow.toml"},
                 "declared_writes": ["policy/allow.toml"], "expected_exit": "zero"},
                {"purpose": "the ordinary gate re-exposes the unreceipted finding "
                            "after the restore",
                 "route": "proof", "observation_class": "outcome",
                 "argv": ["BINARY:", "check", "--config", "ROOT:policy/allow.toml",
                          "--mode", "no-new"],
                 "declared_writes": [], "expected_exit": "nonzero"},
            ],
        },
        {
            "id": "C.propose.stale_conflict",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "live ledger path refuses create-only candidate write",
                 "route": "refusal",
                 "argv": CONTROLLED_PROPOSE + ["--write", "ROOT:policy/allow.toml"],
                 "declared_writes": [], "expected_exit": "nonzero"},
                {"purpose": "missing instrument config refuses with no mutation",
                 "route": "refusal",
                 "argv": ["BINARY:", "propose", "--config", "ROOT:policy/missing.toml",
                          "--write", "ROOT:policy/candidate.toml"],
                 "declared_writes": [], "expected_exit": "nonzero"},
            ],
        },
        {
            "id": "C.propose.presentation",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "human proposal route",
                 "route": "preview", "argv": list(CONTROLLED_PROPOSE),
                 "declared_writes": [], "expected_exit": "zero"},
                {"purpose": "typed JSON summary route evaluates the same outcome",
                 "route": "preview",
                 "argv": CONTROLLED_PROPOSE + ["--summary-format", "json",
                                               "--summary-output",
                                               "ROOT:target/propose-summary.json"],
                 "declared_writes": [], "expected_exit": "zero",
                 "artifacts": {"summary": "target/propose-summary.json"}},
                {"purpose": "quiet proposal route evaluates the same outcome",
                 "route": "preview", "argv": CONTROLLED_PROPOSE + ["-q"],
                 "declared_writes": [], "expected_exit": "zero"},
                {"purpose": "unsupported summary format value is refused without mutation",
                 "route": "refusal",
                 "argv": CONTROLLED_PROPOSE + ["--summary-format", "markdown"],
                 "declared_writes": [], "expected_exit": "nonzero"},
            ],
        },
        {
            "id": "C.add.preview",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "dry-run previews the candidate entry without any mutation",
                 "route": "preview", "argv": CONTROLLED_ADD + ["--dry-run"],
                 "declared_writes": [], "expected_exit": "zero"},
                {"purpose": "probe: dry-run plus candidate path must not write",
                 "route": "preview", "observation_class": "contract_probe",
                 "probe_write_path": "policy/add-candidate.toml",
                 "argv": CONTROLLED_ADD + ["--dry-run", "--write",
                                           "ROOT:policy/add-candidate.toml"],
                 "declared_writes": ["policy/add-candidate.toml"],
                 "expected_exit": "any"},
            ],
        },
        {
            "id": "C.add.candidate",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "candidate write owns exactly the requested new path",
                 "route": "candidate_write",
                 "argv": CONTROLLED_ADD + ["--write", "ROOT:policy/add-candidate.toml"],
                 "declared_writes": ["policy/add-candidate.toml"], "expected_exit": "zero"},
                {"purpose": "existing candidate refuses replacement without --force",
                 "route": "refusal",
                 "argv": CONTROLLED_ADD + ["--write", "ROOT:policy/add-candidate.toml"],
                 "declared_writes": [], "expected_exit": "nonzero"},
                {"purpose": "--force replaces the candidate with recovery bytes",
                 "route": "candidate_write",
                 "argv": CONTROLLED_ADD + ["--write", "ROOT:policy/add-candidate.toml",
                                           "--force"],
                 "declared_writes": ["policy/add-candidate.toml",
                                     "policy/add-candidate.toml.bak"],
                 "expected_exit": "zero"},
            ],
        },
        {
            "id": "C.add.apply",
            "recipe": {"ledger": True, "commit_ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "live apply adds the reviewed entry to the ledger",
                 "route": "mutation", "argv": CONTROLLED_ADD + ["--update"],
                 "declared_writes": ["policy/allow.toml"], "expected_exit": "zero"},
                {"purpose": "full no-new gate after the targeted apply",
                 "route": "proof", "observation_class": "outcome",
                 "argv": ["BINARY:", "check", "--config", "ROOT:policy/allow.toml",
                          "--mode", "no-new"],
                 "declared_writes": [], "expected_exit": "zero"},
                {"purpose": "fixture-level rollback restores the committed ledger bytes",
                 "route": "fixture_rollback",
                 "fixture_control": {"action": "git_restore", "target": "policy/allow.toml"},
                 "declared_writes": ["policy/allow.toml", ".git/index"],
                 "expected_exit": "zero"},
                {"purpose": "the ordinary gate re-exposes the finding after rollback",
                 "route": "proof", "observation_class": "outcome",
                 "argv": ["BINARY:", "check", "--config", "ROOT:policy/allow.toml",
                          "--mode", "no-new"],
                 "declared_writes": [], "expected_exit": "nonzero"},
            ],
        },
        {
            "id": "C.add.stale_conflict",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "selector for a path without findings refuses mutation",
                 "route": "refusal",
                 "argv": ["BINARY:", "add", "--config", "ROOT:policy/allow.toml",
                          "--kind", "panic", "--path", "src/nonexistent.rs", "--line", "2",
                          "--owner", "mutation-fixture",
                          "--reason", "controlled #4725 mutation fixture",
                          "--classification", "baseline_debt",
                          "--expires", "2027-01-11", "--review-after", "2026-12-15",
                          "--update"],
                 "declared_writes": [], "expected_exit": "nonzero"},
                {"purpose": "observed: --update with --dry-run previews; dry-run "
                            "wins and no bytes are written",
                 "route": "preview", "argv": CONTROLLED_ADD + ["--update", "--dry-run"],
                 "declared_writes": [], "expected_exit": "zero"},
                {"purpose": "first valid apply seeds the replay state",
                 "route": "mutation", "argv": CONTROLLED_ADD + ["--update"],
                 "declared_writes": ["policy/allow.toml"], "expected_exit": "zero"},
                {"purpose": "replaying a receipted finding refuses and preserves bytes",
                 "route": "refusal", "argv": CONTROLLED_ADD + ["--update"],
                 "declared_writes": [], "expected_exit": "nonzero"},
            ],
        },
        {
            "id": "C.add.presentation",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "human dry-run route",
                 "route": "preview", "argv": CONTROLLED_ADD + ["--dry-run"],
                 "declared_writes": [], "expected_exit": "zero"},
                {"purpose": "typed JSON summary route evaluates the same outcome",
                 "route": "preview",
                 "argv": CONTROLLED_ADD + ["--dry-run", "--summary-format", "json",
                                           "--summary-output",
                                           "ROOT:target/add-summary.json"],
                 "declared_writes": [], "expected_exit": "zero",
                 "artifacts": {"summary": "target/add-summary.json"}},
                {"purpose": "quiet dry-run route evaluates the same outcome",
                 "route": "preview", "argv": CONTROLLED_ADD + ["--dry-run", "-q"],
                 "declared_writes": [], "expected_exit": "zero"},
                {"purpose": "unsupported summary format value is refused without mutation",
                 "route": "refusal",
                 "argv": CONTROLLED_ADD + ["--dry-run", "--summary-format", "markdown"],
                 "declared_writes": [], "expected_exit": "nonzero"},
            ],
        },
        {
            "id": "C.add_from_plan.unsupported_preview",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "plan producer retains the linked candidate plan",
                 "route": "plan_generation",
                 "argv": CONTROLLED_WHY + ["--plan", "ROOT:target/add-plan.json"],
                 "declared_writes": [], "expected_exit": "zero",
                 "artifacts": {"plan": "target/add-plan.json"}},
                {"purpose": "from-plan without --update is an explicit unsupported refusal",
                 "route": "refusal", "argv": PLAN_WITH_OWNER,
                 "declared_writes": [], "expected_exit": "nonzero"},
                {"purpose": "probe: --dry-run with --update must not silently apply",
                 "route": "unsupported_preview_probe", "observation_class": "contract_probe",
                 "probe_write_path": "policy/allow.toml",
                 "argv": PLAN_WITH_OWNER + ["--update", "--dry-run"],
                 "declared_writes": ["policy/allow.toml"], "expected_exit": "any"},
            ],
        },
        {
            "id": "C.add_from_plan.plan_linkage",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "why --plan retains the candidate plan and its identity",
                 "route": "plan_generation",
                 "argv": CONTROLLED_WHY + ["--plan", "ROOT:target/add-plan.json"],
                 "declared_writes": [], "expected_exit": "zero",
                 "artifacts": {"plan": "target/add-plan.json"}},
                {"purpose": "the linked add --from-plan --update consumes that exact plan",
                 "route": "mutation", "argv": PLAN_WITH_OWNER + ["--update"],
                 "declared_writes": ["policy/allow.toml"], "expected_exit": "zero"},
                {"purpose": "from-plan conflicts with --write and preserves bytes",
                 "route": "refusal",
                 "argv": PLAN_WITH_OWNER + ["--write", "ROOT:policy/cand.toml"],
                 "declared_writes": [], "expected_exit": "nonzero"},
            ],
        },
        {
            "id": "C.add_from_plan.apply",
            "recipe": {"ledger": True, "commit_ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "plan generation for the linked apply",
                 "route": "plan_generation",
                 "argv": CONTROLLED_WHY + ["--plan", "ROOT:target/add-plan.json"],
                 "declared_writes": [], "expected_exit": "zero",
                 "artifacts": {"plan": "target/add-plan.json"}},
                {"purpose": "live apply of the linked plan with targeted recheck",
                 "route": "mutation", "argv": PLAN_WITH_OWNER + ["--update"],
                 "declared_writes": ["policy/allow.toml"], "expected_exit": "zero"},
                {"purpose": "full no-new gate after the targeted plan apply",
                 "route": "proof", "observation_class": "outcome",
                 "argv": ["BINARY:", "check", "--config", "ROOT:policy/allow.toml",
                          "--mode", "no-new"],
                 "declared_writes": [], "expected_exit": "zero"},
                {"purpose": "fixture-level rollback restores the committed ledger bytes",
                 "route": "fixture_rollback",
                 "fixture_control": {"action": "git_restore", "target": "policy/allow.toml"},
                 "declared_writes": ["policy/allow.toml", ".git/index"],
                 "expected_exit": "zero"},
                {"purpose": "the ordinary gate re-exposes the finding after rollback",
                 "route": "proof", "observation_class": "outcome",
                 "argv": ["BINARY:", "check", "--config", "ROOT:policy/allow.toml",
                          "--mode", "no-new"],
                 "declared_writes": [], "expected_exit": "nonzero"},
            ],
        },
        {
            "id": "C.add_from_plan.stale_conflict",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "plan generation for the stale routes",
                 "route": "plan_generation",
                 "argv": CONTROLLED_WHY + ["--plan", "ROOT:target/add-plan.json"],
                 "declared_writes": [], "expected_exit": "zero",
                 "artifacts": {"plan": "target/add-plan.json"}},
                {"purpose": "first apply seeds the consumed-plan state",
                 "route": "mutation", "argv": PLAN_WITH_OWNER + ["--update"],
                 "declared_writes": ["policy/allow.toml"], "expected_exit": "zero"},
                {"purpose": "replaying the consumed plan refuses and preserves bytes",
                 "route": "refusal", "argv": PLAN_WITH_OWNER + ["--update"],
                 "declared_writes": [], "expected_exit": "nonzero"},
                {"purpose": "harness edits the retained plan to a stale location",
                 "route": "fixture_control", "observation_class": "fixture_control",
                 "fixture_control": {"action": "edit_json", "path": "target/add-plan.json",
                                     "set": {"finding.line": 3}},
                 "declared_writes": [], "expected_exit": "zero"},
                {"purpose": "stale plan location refuses with the ledger unchanged",
                 "route": "refusal", "argv": PLAN_WITH_OWNER + ["--update"],
                 "declared_writes": [], "expected_exit": "nonzero"},
                {"purpose": "missing plan file refuses with no mutation",
                 "route": "refusal",
                 "argv": ["BINARY:", "add", "--config", "ROOT:policy/allow.toml",
                          "--from-plan", "ROOT:target/missing-plan.json"] + PLAN_OWNER
                         + ["--update"],
                 "declared_writes": [], "expected_exit": "nonzero"},
            ],
        },
        {
            "id": "C.add_from_plan.presentation",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "plan generation for the presentation parity routes",
                 "route": "plan_generation",
                 "argv": CONTROLLED_WHY + ["--plan", "ROOT:target/add-plan.json"],
                 "declared_writes": [], "expected_exit": "zero",
                 "artifacts": {"plan": "target/add-plan.json"}},
                {"purpose": "missing-plan refusal on the human route",
                 "route": "refusal",
                 "argv": ["BINARY:", "add", "--config", "ROOT:policy/allow.toml",
                          "--from-plan", "ROOT:target/missing-plan.json"] + PLAN_OWNER
                         + ["--update"],
                 "declared_writes": [], "expected_exit": "nonzero"},
                {"purpose": "missing-plan refusal on the typed JSON summary route",
                 "route": "refusal",
                 "argv": ["BINARY:", "add", "--config", "ROOT:policy/allow.toml",
                          "--from-plan", "ROOT:target/missing-plan.json"] + PLAN_OWNER
                         + ["--update", "--summary-format", "json", "--summary-output",
                            "ROOT:target/fp-summary.json"],
                 "declared_writes": [], "expected_exit": "nonzero",
                 "artifacts": {"summary": "target/fp-summary.json"}},
                {"purpose": "missing-plan refusal on the quiet route",
                 "route": "refusal",
                 "argv": ["BINARY:", "add", "--config", "ROOT:policy/allow.toml",
                          "--from-plan", "ROOT:target/missing-plan.json"] + PLAN_OWNER
                         + ["--update", "-q"],
                 "declared_writes": [], "expected_exit": "nonzero"},
                {"purpose": "unsupported summary format value is refused without mutation",
                 "route": "refusal",
                 "argv": PLAN_WITH_OWNER + ["--update", "--summary-format", "markdown"],
                 "declared_writes": [], "expected_exit": "nonzero"},
            ],
        },
    ]


def c_controls():
    """Adversarial route controls retained beside the 20 obligations."""
    return [
        {
            "id": "control.init-summary-backup-collision",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "seed a recovery backup with a --force replacement",
                 "route": "mutation",
                 "argv": ["BINARY:", "init", "--force"],
                 "declared_writes": ["policy/allow.toml", "policy/allow.toml.bak"],
                 "expected_exit": "zero"},
                {"purpose": "probe: summary destination at the backup path must not destroy recovery bytes",
                 "route": "mutation", "observation_class": "contract_probe",
                 "probe_write_path": "policy/allow.toml.bak",
                 "probe_expect_bytes": "backup",
                 "argv": ["BINARY:", "init", "--force", "--command-summary-output",
                          "ROOT:policy/allow.toml.bak"],
                 "declared_writes": ["policy/allow.toml", "policy/allow.toml.bak"],
                 "expected_exit": "any"},
            ],
        },
        {
            "id": "control.outside-root-refusal",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "init refuses a config target outside the source-tree root",
                 "route": "refusal",
                 "argv": ["BINARY:", "init", "--config", "ROOT:../outside-candidate.toml"],
                 "declared_writes": [], "expected_exit": "nonzero"},
                {"purpose": "propose refuses a candidate target outside the source-tree root",
                 "route": "refusal",
                 "argv": ["BINARY:", "propose", "--config", "ROOT:policy/allow.toml",
                          "--write", "ROOT:../outside-candidate.toml"],
                 "declared_writes": [], "expected_exit": "nonzero"},
            ],
        },
        {
            "id": "control.target-collision-refusal",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "candidate target under a file parent is refused",
                 "route": "refusal",
                 "argv": CONTROLLED_PROPOSE + ["--write", "ROOT:src/lib.rs/child.toml"],
                 "declared_writes": [], "expected_exit": "nonzero"},
                {"purpose": "candidate target that is an existing directory is refused",
                 "route": "refusal",
                 "argv": CONTROLLED_PROPOSE + ["--write", "ROOT:src", "--force"],
                 "declared_writes": [], "expected_exit": "nonzero"},
            ],
        },
        {
            "id": "control.in-root-subdirectory-write",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "in-root selected subdirectory creation is retained as observed scope",
                 "route": "candidate_write",
                 "argv": CONTROLLED_PROPOSE + ["--write", "ROOT:src/deeper/candidate.toml"],
                 "declared_writes": ["src/deeper", "src/deeper/candidate.toml"],
                 "expected_exit": "zero"},
            ],
        },
        {
            "id": "control.invalid-policy-refusal",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "harness corrupts the ledger into invalid bytes",
                 "route": "fixture_control", "observation_class": "fixture_control",
                 "fixture_control": {"action": "write_bytes", "path": "policy/allow.toml",
                                     "content": "not = [valid\n"},
                 "declared_writes": ["policy/allow.toml"], "expected_exit": "zero"},
                {"purpose": "add refuses mutation against an invalid ledger",
                 "route": "refusal", "argv": CONTROLLED_ADD + ["--update"],
                 "declared_writes": [], "expected_exit": "nonzero"},
                {"purpose": "no-new gate refuses against an invalid ledger",
                 "route": "refusal", "observation_class": "outcome",
                 "argv": ["BINARY:", "check", "--config", "ROOT:policy/allow.toml",
                          "--mode", "no-new"],
                 "declared_writes": [], "expected_exit": "nonzero"},
            ],
        },
        {
            "id": "control.renamed-target-alias",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "harness renames the ledger to a supported alias path",
                 "route": "fixture_control", "observation_class": "fixture_control",
                 "fixture_control": {"action": "rename", "source": "policy/allow.toml",
                                     "target": "policy/allow-moved.toml"},
                 "declared_writes": ["policy/allow.toml", "policy/allow-moved.toml"],
                 "expected_exit": "zero"},
                {"purpose": "mutation via the moved alias path owns that exact file",
                 "route": "mutation",
                 "argv": ["BINARY:", "add", "--config", "ROOT:policy/allow-moved.toml"]
                         + ADD_BASE + ["--update"],
                 "declared_writes": ["policy/allow-moved.toml"], "expected_exit": "zero"},
                {"purpose": "harness restores the canonical ledger name",
                 "route": "fixture_control", "observation_class": "fixture_control",
                 "fixture_control": {"action": "rename", "source": "policy/allow-moved.toml",
                                     "target": "policy/allow.toml"},
                 "declared_writes": ["policy/allow.toml", "policy/allow-moved.toml"],
                 "expected_exit": "zero"},
            ],
        },
        {
            "id": "control.symlink-alias",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "reparse-point alias creation attempt is observed on its real platform terms",
                 "route": "fixture_control", "observation_class": "platform_probe",
                 "fixture_control": {"action": "symlink",
                                     "link": "policy/allow-link.toml",
                                     "target": "policy/allow.toml"},
                 "declared_writes": ["policy/allow-link.toml"], "expected_exit": "any"},
                {"purpose": "when the alias exists, mutation through it is observed",
                 "route": "mutation", "observation_class": "platform_probe",
                 "argv": ["BINARY:", "add", "--config", "ROOT:policy/allow-link.toml"]
                         + ADD_BASE + ["--update"],
                 "declared_writes": ["policy/allow.toml", "policy/allow-link.toml"],
                 "expected_exit": "any",
                 "conditional_on_alias": True},
            ],
        },
        {
            "id": "control.selector-line-not-matching",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "observed: the add selector line is not part of matching; "
                            "a line matching no finding still receipts the located "
                            "finding",
                 "route": "mutation",
                 "argv": ["BINARY:", "add", "--config", "ROOT:policy/allow.toml",
                          "--kind", "panic", "--path", "src/lib.rs", "--line", "99",
                          "--owner", "mutation-fixture",
                          "--reason", "controlled #4725 mutation fixture",
                          "--classification", "baseline_debt",
                          "--expires", "2027-01-11", "--review-after", "2026-12-15",
                          "--update"],
                 "declared_writes": ["policy/allow.toml"],
                 "expected_exit": "zero"},
            ],
        },
        {
            "id": "control.missing-rollback-state",
            "recipe": {"ledger": True},
            "forbidden_paths": [],
            "steps": [
                {"purpose": "seed a backup for the rollback state",
                 "route": "mutation", "argv": ["BINARY:", "init", "--force"],
                 "declared_writes": ["policy/allow.toml", "policy/allow.toml.bak"],
                 "expected_exit": "zero"},
                {"purpose": "harness removes the rollback state bytes",
                 "route": "fixture_control", "observation_class": "fixture_control",
                 "fixture_control": {"action": "remove", "path": "policy/allow.toml.bak"},
                 "declared_writes": ["policy/allow.toml.bak"], "expected_exit": "zero"},
                {"purpose": "rollback without retained state fails instead of declaring success",
                 "route": "fixture_rollback",
                 "fixture_control": {"action": "restore", "source": "policy/allow.toml.bak",
                                     "target": "policy/allow.toml"},
                 "declared_writes": ["policy/allow.toml"], "expected_exit": "zero"},
            ],
        },
    ]


def change_expectation(step):
    """Route-default observation contract; explicit values override."""
    explicit = step.get("change_expectation")
    if explicit:
        return explicit
    route = step["route"]
    if route == "mutation":
        return "required"
    if route in ("preview", "proof", "plan_generation", "refusal"):
        return "none"
    return "optional"


def expected_context(definition, catalogue_digest, binary_context, collection_id):
    cases = []
    for case in definition:
        cases.append({
            "case_id": case["id"],
            "recipe": case["recipe"],
            "source_digest": digest(SOURCE_BYTES.encode("utf-8")),
            "forbidden_paths": case["forbidden_paths"],
            "steps": [{
                "purpose": step["purpose"],
                "route": step["route"],
                "observation_class": step.get("observation_class", "outcome"),
                "argv_template": step.get("argv", []),
                "declared_writes": step["declared_writes"],
                "expected_exit": step["expected_exit"],
                "probe_write_path": step.get("probe_write_path"),
                "fixture_control": step.get("fixture_control"),
                "change_expectation": change_expectation(step),
                "artifact_roles": sorted(step.get("artifacts", {}).keys()),
            } for step in case["steps"]],
        })
    return {
        "schema_id": CONTEXT_SCHEMA, "schema_version": 1,
        "collection_id": collection_id,
        "catalogue_digest": catalogue_digest,
        "binary": binary_context,
        "source_bytes_digest": digest(SOURCE_BYTES.encode("utf-8")),
        "cases": cases,
    }


def collect(args):
    catalogue, catalogue_bytes = load_json(args.catalogue.absolute())
    if catalogue.get("schema_id") != "cargo-allow.command-migration-catalogue.v1" \
            or catalogue.get("schema_version") != 1:
        raise ValueError("unsupported catalogue generation")
    specs = {item["id"]: item for item in catalogue["cases"]}
    if len(specs) != len(catalogue["cases"]):
        raise ValueError("duplicate catalogue case")
    selected = args.case or [case["id"] for case in c_cases()]
    if len(set(selected)) != len(selected) or not selected:
        raise ValueError("case selection is empty or duplicated")
    definitions = {case["id"]: case for case in c_cases() + c_controls()}
    if any((case not in specs and not case.startswith("control."))
           or case not in definitions for case in selected):
        raise ValueError("unknown case requested")
    if any(not case.startswith("control.")
           and (not specs[case]["id"].startswith(C_FAMILY_PREFIXES)
                or specs[case]["family"] != "C") for case in selected):
        raise ValueError("selection is outside the C-family mutation transport")
    if any(specs[case]["command"] not in MUTATION_COMMANDS
           for case in selected if not case.startswith("control.")):
        raise ValueError("selection includes a non-mutation command case")
    binary, git = args.binary.absolute(), args.git.absolute()
    if not args.binary.is_absolute() or not args.git.is_absolute():
        raise ValueError("--binary and --git must be explicit absolute paths")
    binary_bytes = regular_bytes(binary, BINARY_LIMIT)
    if not binary_bytes:
        raise ValueError("supplied binary is empty")
    regular_bytes(git, BINARY_LIMIT)
    if args.provenance == "supplied_installed_candidate" \
            and not (args.candidate_identity and args.install_identity):
        raise ValueError("a supplied installed candidate requires explicit candidate "
                         "and install identities")
    if args.provenance == "source_build" and (args.candidate_identity or args.install_identity):
        raise ValueError("a source build cannot claim installed-candidate identities")
    binary_context = {
        "path": str(binary), "size_bytes": len(binary_bytes), "digest": digest(binary_bytes),
        "tool_version": args.tool_version, "provenance": args.provenance,
        "source_generation": args.source_generation,
        "candidate_identity": args.candidate_identity,
        "install_identity": args.install_identity,
    }
    root = owned_root(args.output_dir)
    store = Store(root)
    (root / "homes").mkdir()
    store.put("binary/cargo-allow", binary_bytes)
    catalogue_digest = digest(catalogue_bytes)
    context = expected_context(
        [definitions[case] for case in selected], catalogue_digest, binary_context,
        args.collection_id)
    context_member = store.put("expected-context.json", json_bytes(context))
    cases = []
    for case_id in selected:
        definition = definitions[case_id]
        fixture_root = root / "fixtures" / safe_relative(case_id)
        env = child_environment(root / "homes" / safe_relative(case_id), git)
        commit = build_fixture(definition["recipe"], fixture_root, binary, git,
                               env, args.timeout, store,
                               "cases/" + safe_relative(case_id) + "/fixture")
        steps = []
        alias_exists = False
        for index, step in enumerate(definition["steps"]):
            prefix = "cases/" + safe_relative(case_id) + f"/step-{index}"
            control = step.get("fixture_control")
            if control is not None:
                if control["action"] == "restore":
                    steps.append(rollback_step(
                        fixture_root, binary, git, env, args.timeout, store, prefix,
                        index, step["purpose"], control["source"], control["target"]))
                elif control["action"] == "git_restore":
                    steps.append(git_restore_step(
                        fixture_root, git, env, args.timeout, store, prefix,
                        index, step["purpose"], control["target"]))
                else:
                    steps.append(fixture_control_step(
                        fixture_root, env, store, prefix, index, step, control))
                    if control["action"] == "symlink" and (
                            fixture_root / control["link"]).is_symlink():
                        alias_exists = True
                continue
            if step.get("conditional_on_alias") and not alias_exists:
                steps.append(skipped_alias_step(store, prefix, index, step))
                continue
            steps.append(run_case_step(step, fixture_root, binary, git, env,
                                       args.timeout, store,
                                       "cases/" + safe_relative(case_id), index))
        record = {
            "case_id": case_id,
            "fixture_root": str(fixture_root),
            "fixture_commit": commit,
            "environment": env,
            "steps": steps,
        }
        store.put("cases/" + safe_relative(case_id) + "/case.json", json_bytes(record))
        cases.append(record)
    bundle = {
        "schema_id": SCHEMA, "schema_version": 1,
        "collection_id": args.collection_id,
        "catalogue_digest": catalogue_digest,
        "context_digest": context_member["digest"],
        "binary": binary_context,
        "binary_digest_after": digest(regular_bytes(binary, BINARY_LIMIT)),
        "cases": cases,
    }
    bundle_member = store.put("bundle.json", json_bytes(bundle))
    print(json.dumps({
        "bundle": str(root / "bundle.json"),
        "bundle_digest": bundle_member["digest"],
        "expected_context": str(root / "expected-context.json"),
        "selected_cases": len(cases), "catalogue_cases": len(catalogue["cases"]),
        "semantic_admission": "not_performed", "qualification": "not_claimed",
    }, sort_keys=True))


def git_restore_step(fixture_root, git, env, timeout, store, prefix, index,
                     purpose, target_relative):
    before = snapshot(fixture_root, store, prefix + "/before")
    result, stdout, stderr = run_process(
        [str(git), "checkout", "--", target_relative], fixture_root, env, timeout,
        store, prefix + "/git-restore")
    after = snapshot(fixture_root, store, prefix + "/after")
    changes = changed_paths(before, after, lambda member: regular_bytes(
        store.root / member["path"], JSON_LIMIT))
    for change in changes:
        change["declared"] = True  # Git restore owns the ledger and its index.
    record = {
        "index": index, "purpose": purpose, "route": "fixture_rollback",
        "observation_class": "fixture_control", "argv": [str(git), "checkout", "--",
                                                         target_relative],
        "cwd": str(fixture_root), "process": result,
        "stdout": stdout, "stderr": stderr,
        "before": before, "after": after, "changed_paths": changes,
        "artifacts": [], "expected_exit": "zero",
    }
    store.put(prefix + "/record.json", json_bytes(record))
    return record


def skipped_alias_step(store, prefix, index, step):
    record = {
        "index": index, "purpose": step["purpose"], "route": step["route"],
        "observation_class": "platform_probe",
        "argv": None,
        "process": {
            "started_at_utc": utc_now(), "finished_at_utc": utc_now(),
            "started": False, "exit_code": None,
            "launch_error": "alias creation unavailable on this platform; "
                            "retained as NotProven rather than skipped-as-green",
            "timed_out": False,
        },
        "stdout": None, "stderr": None, "before": None, "after": None,
        "changed_paths": [], "artifacts": [], "expected_exit": "any",
    }
    store.put(prefix + "/record.json", json_bytes(record))
    return record


def fixture_control_step(fixture_root, env, store, prefix, index, step, control):
    before = snapshot(fixture_root, store, prefix + "/before")
    outcome = {"action": control["action"], "performed": True, "error": None}
    try:
        if control["action"] == "rename":
            (fixture_root / control["source"]).rename(fixture_root / control["target"])
        elif control["action"] == "write_bytes":
            path = fixture_root / control["path"]
            reject_symlinks(path)
            with path.open("wb") as handle:
                handle.write(control["content"].encode("utf-8"))
        elif control["action"] == "remove":
            (fixture_root / control["path"]).unlink()
        elif control["action"] == "edit_json":
            path = fixture_root / control["path"]
            data = json.loads(regular_bytes(path, JSON_LIMIT),
                              object_pairs_hook=unique_object)
            for key, value in control["set"].items():
                parent = data
                parts = key.split(".")
                for part in parts[:-1]:
                    parent = parent[part]
                parent[parts[-1]] = value
            with path.open("w", encoding="utf-8") as handle:
                json.dump(data, handle, indent=2)
        elif control["action"] == "symlink":
            link = fixture_root / control["link"]
            reject_symlinks(link.parent)
            # The stored link target is relative to the link's own directory.
            os.symlink(Path(control["target"]).name, link)
        else:
            raise ValueError("unknown fixture control action: " + control["action"])
    except OSError as error:
        outcome = {"action": control["action"], "performed": False,
                   "error": str(error)}
    after = snapshot(fixture_root, store, prefix + "/after")
    changes = changed_paths(before, after, lambda member: regular_bytes(
        store.root / member["path"], JSON_LIMIT))
    declared = set(step.get("declared_writes", []))
    for change in changes:
        change["declared"] = change["path"] in declared
    record = {
        "index": index, "purpose": step["purpose"], "route": step["route"],
        "observation_class": step.get("observation_class", "fixture_control"),
        "argv": None, "cwd": str(fixture_root),
        "process": {
            "started_at_utc": utc_now(), "finished_at_utc": utc_now(),
            "started": outcome["performed"], "exit_code": 0 if outcome["performed"] else None,
            "launch_error": outcome["error"], "timed_out": False,
        },
        "stdout": None, "stderr": None,
        "before": before, "after": after, "changed_paths": changes,
        "artifacts": [], "expected_exit": step.get("expected_exit", "zero"),
        "control_outcome": outcome,
    }
    store.put(prefix + "/record.json", json_bytes(record))
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--git", type=Path, required=True)
    parser.add_argument("--catalogue", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--collection-id", required=True)
    parser.add_argument("--tool-version", required=True)
    parser.add_argument("--source-generation", required=True)
    parser.add_argument("--provenance", choices=["source_build",
                                                 "supplied_installed_candidate"],
                        required=True)
    parser.add_argument("--candidate-identity")
    parser.add_argument("--install-identity")
    parser.add_argument("--case", action="append")
    parser.add_argument("--timeout", type=float, default=240.0)
    args = parser.parse_args()
    if args.timeout <= 0 or args.timeout > 600:
        parser.error("--timeout must be greater than zero and at most 600 seconds")
    try:
        collect(args)
    except (OSError, ValueError, KeyError, TypeError) as error:
        print("command-mutation collection failed: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
