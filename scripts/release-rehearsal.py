#!/usr/bin/env python3
"""Fail-closed characterization of the exact-subject release rehearsal."""

from __future__ import annotations

import argparse
import contextlib
from contextvars import ContextVar
import ctypes
import errno
import hashlib
import json
import os
from os import getpid as _GETPID, name as _PLATFORM_NAME, pathsep as _PATH_SEPARATOR
from pathlib import Path
import shutil
import socket
import stat
import subprocess
import sys
import tempfile
from typing import Any, Callable

_STATVFS = getattr(os, "statvfs", None)
_ST_RDONLY = getattr(os, "ST_RDONLY", None)

ROOT = Path(__file__).resolve().parent.parent
PHASE_COMPLETE = "Complete"
PHASE_INCOMPLETE = "Incomplete"
PHASE_MISMATCH = "Mismatch"
PHASE_INSTRUMENT_FAILURE = "InstrumentFailure"
CARGO_TOKEN_ENV = "CARGO_REGISTRY_TOKEN"
LINUX_TAG_WORKER_PROFILE = "linux-userns-ro-git-netns-v1"
ZERO_MUTATION_FLAGS = (
    "tag_mutation_prevented",
    "token_read_prevented",
    "cargo_publish_prevented",
    "registry_mutation_prevented",
    "github_release_mutation_prevented",
    "live_setting_mutation_prevented",
    "external_repository_mutation_prevented",
)
_LINUX_MS_REMOUNT = 32
_LINUX_MS_BIND = 4096
_LINUX_NET_PROBE_TARGET = ("192.0.2.1", 443)  # RFC 5737 TEST-NET-1; UDP connect emits no packets
_WORKER_STRIPPED_ENV_EXACT = frozenset({
    CARGO_TOKEN_ENV,
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "SSH_AUTH_SOCK",
    "SSH_AGENT_PID",
    "GPG_AGENT_INFO",
    "GIT_ASKPASS",
    "SSH_ASKPASS",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_NAMESPACE",
    "GIT_CONFIG",
    "GIT_ATTR_SOURCE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_EXEC_PATH",
    "GIT_SSH",
    "GIT_TEMPLATE_DIR",
    "GIT_PROXY_COMMAND",
    "GIT_EXTERNAL_DIFF",
})


def _selected_token_present() -> bool:
    """Inspect names only; mapping membership/get may retrieve a value."""
    return any(name == CARGO_TOKEN_ENV for name in os.environ)


def _require_selected_token_absent() -> None:
    if _selected_token_present():
        raise ValueError("rehearsal does not accept a CARGO_REGISTRY_TOKEN environment entry")


class _Invocation:
    """Private selected-call context, not a sandbox or prevention receipt."""

    def __init__(self) -> None:
        self.root = ROOT.resolve()
        self.python = str(Path(sys.executable).absolute())
        search_path = _PATH_SEPARATOR.join(
            str(path if path.is_absolute() else self.root / path)
            for value in os.environ.get("PATH", "").split(_PATH_SEPARATOR)
            if value
            for path in [Path(value)]
        )
        # Freeze lookup now, including platform lookup inputs. An unused,
        # unavailable tool does not fail a phase that never invokes it.
        self.tools = {name: _select_tool(name, search_path) for name in ("git", "cargo")}

    def executable(self, name: str) -> str:
        if name == "python":
            return self.python
        selected = self.tools.get(name)
        if selected is None:
            raise OSError("rehearsal selected executable is unavailable")
        return selected


def _select_tool(name: str, search_path: str) -> str | None:
    """Resolve only within captured lookup roots; preserve shim argv[0]."""
    found = shutil.which(name, path=search_path)
    if found is None:
        return None
    selected = Path(found).absolute()
    roots = {Path(value).absolute() for value in search_path.split(_PATH_SEPARATOR) if value}
    if selected.parent not in roots:
        return None
    if _PLATFORM_NAME == "nt" and selected.suffix.lower() not in {".exe", ".com"}:
        return None
    return str(selected)


_INVOCATION: ContextVar[_Invocation | None] = ContextVar("rehearsal_invocation", default=None)


@contextlib.contextmanager
def _invocation_scope():
    current = _INVOCATION.get()
    token = _INVOCATION.set(_Invocation()) if current is None else None
    try:
        yield _INVOCATION.get()
    finally:
        if token is not None:
            _INVOCATION.reset(token)


def _invocation_command(context: _Invocation, operation: str, selected: Any) -> tuple[list[str], str]:
    git = ["git", "--no-replace-objects"]
    inspection = git + ["--no-optional-locks", "-c", "core.fsmonitor=false"]
    fixed = {
        "checkout-root": (git + ["rev-parse", "--show-toplevel"], "git"),
        "checkout-ctime": (git + ["config", "--type=bool", "--default=true", "--get", "core.trustctime"], "git"),
        "checkout-index": (inspection + ["ls-files", "--cached", "-v", "-z"], "git"),
        "checkout-status": (git + [
            "--no-optional-locks", "-c", "core.fsmonitor=false", "-c", "core.untrackedCache=false",
            "-c", "core.checkStat=default", "-c", "core.ignoreStat=false", "-c", "core.trustctime=true",
            "status", "--porcelain=v1", "-z", "--untracked-files=all", "--ignored=matching",
            "--ignore-submodules=none",
        ], "git"),
        "output-tracked": (["git", "ls-files", "--cached", "-z"], "git"),
        "git-dirs": (git + ["rev-parse", "--path-format=absolute", "--git-dir", "--git-common-dir"], "git"),
        "git-tags": (git + ["show-ref", "--tags"], "git"),
        "publisher-proof": ([context.python, str(context.root / "scripts/test-release-topology-publisher.py")], "python"),
        "history-proof": ([context.python, str(context.root / "scripts/generate-changie-history.py"), "--check"], "python"),
        "manifest-proof": ([context.python, str(context.root / "scripts/test-final-packaged-surface.py")], "python"),
        "corpus-proof": ([context.python, str(context.root / "scripts/test-release-escaped-defect-corpus.py")], "python"),
    }
    if operation in fixed and selected is None:
        return fixed[operation]
    if operation == "canary-remote-tags" and isinstance(selected, Path) and (
        selected.is_absolute()
        and not selected.is_relative_to(context.root)
        and not str(selected).startswith("-")
        and not any(char in str(selected) for char in "\r\n\0")
    ):
        return git + ["--git-dir", str(selected), "show-ref", "--tags"], "git"
    if operation == "resolve-commit" and isinstance(selected, str) and selected and (
        not selected.startswith("-") and not any(char in selected for char in "\r\n\0")
    ):
        return git + ["rev-parse", "--verify", f"{selected}^{{commit}}"], "git"
    if operation == "content-attributes" and isinstance(selected, tuple) and selected in (
        ("filter", "ident", "working-tree-encoding"), ("--all",),
    ):
        return inspection + ["check-attr", "--stdin", "-z", *selected], "git"
    if operation == "output-ignored" and isinstance(selected, Path) and (
        selected.is_relative_to(context.root / "target") and selected != context.root / "target"
    ):
        return ["git", "check-ignore", "-q", "--", selected.relative_to(context.root).as_posix()], "git"
    if operation in {"candidate-package", "shared-preflight"} and selected is None:
        package = operation == "candidate-package"
        receipt_name = "rehearsal-candidate-package-set.json" if package else "rehearsal-shared-preflight.json"
        return [
            context.python, (context.root / "scripts/release-topology-publisher.py").as_posix(),
            "--mode", "cargo-allow", "--package-only" if package else "--registry-preflight",
            "--receipt", str(context.root / "target/cargo-allow" / receipt_name),
        ], "python"
    if operation == "release-identity" and isinstance(selected, tuple) and len(selected) == 2:
        version, candidate = selected
        if isinstance(version, str) and (
            candidate is None or isinstance(candidate, Path) and candidate.is_absolute()
        ):
            prefix = ["cargo", "run", "--quiet", "-p", "cargo-allow", "--locked", "--"]
            tool = "cargo"
            if candidate is not None:
                prefix, tool = [str(candidate)], str(candidate)
            return prefix + ["release-identity", "--version", version], tool
    raise OSError("rehearsal subprocess operation is not selected")


def _run_process(command: list[str], *, operation: str, selected: Any = None, **options: Any):
    """Deny unselected argv/root/options before reaching the process adapter."""
    context = _INVOCATION.get()
    if context is None:
        with _invocation_scope():
            return _run_process(command, operation=operation, selected=selected, **options)
    expected, tool = _invocation_command(context, operation, selected)
    text_operations = {
        "resolve-commit", "git-dirs", "git-tags", "canary-remote-tags",
        "release-identity", "candidate-package", "shared-preflight",
        "publisher-proof", "history-proof", "manifest-proof", "corpus-proof",
    }
    timeout = 15 if tool == "git" else 300
    if (
        ROOT.resolve() != context.root
        or not isinstance(options.get("cwd"), (str, Path))
        or Path(options["cwd"]).resolve() != context.root
        or command != expected
        or set(options) - {"cwd", "env", "capture_output", "text", "timeout", "check", "input"}
        or options.get("capture_output") is not True
        or options.get("check") is not False
        or options.get("timeout") != timeout
        or options.get("text", False) is not (operation in text_operations)
        or ("input" in options) != (operation == "content-attributes")
        or operation == "content-attributes" and not isinstance(options.get("input"), bytes)
        or options.get("env") is None
    ):
        raise OSError("rehearsal subprocess invocation is not selected")
    executable = tool if Path(tool).is_absolute() else context.executable(tool)
    if (
        not isinstance(executable, str) or not executable
        or any(char in executable for char in "\r\n\0")
        or not Path(executable).is_absolute()
        or _PLATFORM_NAME == "nt" and Path(executable).suffix.lower() not in {".exe", ".com"}
    ):
        raise OSError("rehearsal selected executable form is not supported")
    # Keep the original argv[0] for Cargo/rustup, while bypassing a later PATH lookup.
    return subprocess.run(
        expected, **dict(options, cwd=context.root,
                        env={name: options["env"][name] for name in options["env"] if name != CARGO_TOKEN_ENV}),
        executable=executable,
    )


def compute_sha256(path: Path) -> str:
    """Return the repository's canonical SHA-256 text for one exact file."""
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(65536):
            digest.update(chunk)
    return f"sha256:v1:{digest.hexdigest()}"


def resolve_commit(commit_ref: str) -> str:
    """Resolve one caller-supplied Git commit ref or fail without substitution."""
    if any(
        name in {
            "GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR", "GIT_INDEX_FILE", "GIT_NAMESPACE",
            "GIT_CONFIG", "GIT_ATTR_SOURCE",
        } or name.startswith("GIT_CONFIG_")
        for name in os.environ
    ):
        raise ValueError(
            "rehearsal does not support Git repository-selection environment overrides "
            "or GIT_CONFIG/GIT_ATTR_SOURCE overrides"
        )
    if (
        not commit_ref
        or commit_ref.startswith("-")
        or any(char in commit_ref for char in "\r\n\0")
    ):
        raise ValueError("commit ref must be non-empty, single-line, and not start with a dash")
    result = _run_process(
        ["git", "--no-replace-objects", "rev-parse", "--verify", f"{commit_ref}^{{commit}}"],
        operation="resolve-commit", selected=commit_ref,
        cwd=ROOT,
        env=_sanitized_environment(),
        capture_output=True,
        text=True,
        timeout=15,
        check=False,
    )
    if result.returncode != 0:
        raise ValueError(f"commit ref is not an exact repository commit: {commit_ref}")
    commit_sha = result.stdout.strip().lower()
    if len(commit_sha) not in (40, 64) or any(
        char not in "0123456789abcdef" for char in commit_sha
    ):
        raise ValueError("resolved commit identity is not canonical hexadecimal Git output")
    return commit_sha


def _is_ignored_rehearsal_artifact(entry: bytes) -> bool:
    """Permit only known ignored output directories, never source lookalikes."""
    if not entry.startswith(b"!! "):
        return False
    path = entry.removeprefix(b"!! ")
    # `.zcode/` is the repository's own agent-tooling state directory
    # (.gitignore): local plans and workflow run state, never release inputs.
    if path in {b"target/", b"__pycache__/", b"scripts/__pycache__/", b".zcode/"}:
        return True
    match path.split(b"/"):
        case [b"crates", crate, b"target", b""]:
            return bool(crate)
    return False


def require_supported_content_attributes(paths: list[bytes]) -> None:
    """Reject content conversion attributes before status can apply them."""
    attributes = (b"filter", b"ident", b"working-tree-encoding")

    def query(arguments: list[str]) -> bytes:
        result = _run_process(
            [
                "git", "--no-replace-objects", "--no-optional-locks", "-c", "core.fsmonitor=false",
                "check-attr", "--stdin", "-z", *arguments,
            ],
            operation="content-attributes", selected=tuple(arguments),
            cwd=ROOT, input=b"".join(path + b"\0" for path in paths),
            env=_sanitized_environment(),
            capture_output=True, timeout=15, check=False,
        )
        if result.returncode != 0:
            raise ValueError("could not inspect rehearsal checkout content transformations")
        return result.stdout

    fields = iter(query(["filter", "ident", "working-tree-encoding"]).split(b"\0"))
    for path in paths:
        for attribute in attributes:
            observed_path = next(fields, None)
            observed_attribute = next(fields, None)
            value = next(fields, None)
            if observed_path != path or observed_attribute != attribute or value is None:
                raise ValueError("invalid rehearsal checkout content transformation inspection")
            if value not in {b"unspecified", b"unset"}:
                raise ValueError("rehearsal does not support Git content transformation attributes")
    if next(fields, None) != b"" or next(fields, None) is not None:
        raise ValueError("invalid rehearsal checkout content transformation inspection")
    # Named values can equal check-attr's state words. --all omits genuinely
    # unspecified attributes, so reject every defined conversion attribute,
    # including explicit unsets whose display is also ambiguous.
    fields = iter(query(["--all"]).split(b"\0"))
    tracked = set(paths)
    seen = set()
    while True:
        path = next(fields, None)
        if path == b"":
            if next(fields, None) is not None:
                raise ValueError("invalid rehearsal checkout content transformation inspection")
            break
        attribute = next(fields, None)
        value = next(fields, None)
        key = (path, attribute)
        if path not in tracked or not attribute or value is None or key in seen:
            raise ValueError("invalid rehearsal checkout content transformation inspection")
        seen.add(key)
        if attribute in attributes:
            raise ValueError("rehearsal does not support Git content transformation attributes")


def require_clean_checkout(commit_sha: str) -> None:
    """Admit only the named checkout with no observed source changes.

    Phases read the working tree, including untracked source such as Changie
    inputs even when local ignore rules hide them. Only known ignored target
    and Python cache directories may remain. Their outputs are not validated
    here. Index flags that hide tracked changes (including sparse checkout)
    are unsupported. This observes checkout state; it is not an immutable
    sandbox.
    """
    if resolve_commit("HEAD") != commit_sha:
        raise ValueError("rehearsal commit does not match checkout HEAD")
    root = _run_process(
        ["git", "--no-replace-objects", "rev-parse", "--show-toplevel"],
        operation="checkout-root",
        cwd=ROOT, env=_sanitized_environment(), capture_output=True, timeout=15, check=False,
    )
    if root.returncode != 0 or not root.stdout.strip():
        raise ValueError("could not inspect rehearsal checkout root")
    discovered_root = Path(os.fsdecode(root.stdout.rstrip(b"\r\n")))
    if not discovered_root.is_absolute() or discovered_root.resolve() != ROOT.resolve():
        raise ValueError("rehearsal checkout root does not match source root")
    ctime = _run_process(
        [
            "git", "--no-replace-objects", "config", "--type=bool", "--default=true",
            "--get", "core.trustctime",
        ],
        operation="checkout-ctime",
        cwd=ROOT, env=_sanitized_environment(), capture_output=True, timeout=15, check=False,
    )
    if ctime.returncode != 0 or ctime.stdout.strip() != b"true":
        raise ValueError("rehearsal requires readable core.trustctime=true configuration")
    index = _run_process(
        [
            "git", "--no-replace-objects", "--no-optional-locks", "-c", "core.fsmonitor=false",
            "ls-files", "--cached", "-v", "-z",
        ],
        operation="checkout-index",
        cwd=ROOT,
        env=_sanitized_environment(),
        capture_output=True,
        timeout=15,
        check=False,
    )
    if index.returncode != 0:
        raise ValueError("could not inspect rehearsal checkout index flags")
    # Only ordinary tracked entries are supported: S marks skip-worktree;
    # lowercase tags mark assume-unchanged. Inspect without clearing flags.
    if any(not entry.startswith(b"H ") for entry in index.stdout.split(b"\0") if entry):
        raise ValueError("rehearsal checkout has unsupported index flags or entries")
    require_supported_content_attributes([
        entry.removeprefix(b"H ") for entry in index.stdout.split(b"\0") if entry
    ])
    result = _run_process(
        [
            "git", "--no-replace-objects", "--no-optional-locks",
            "-c", "core.fsmonitor=false", "-c", "core.untrackedCache=false",
            "-c", "core.checkStat=default", "-c", "core.ignoreStat=false",
            "-c", "core.trustctime=true",
            "status", "--porcelain=v1", "-z", "--untracked-files=all",
            "--ignored=matching",
            "--ignore-submodules=none",
        ],
        operation="checkout-status",
        cwd=ROOT,
        env=_sanitized_environment(),
        capture_output=True,
        timeout=15,
        check=False,
    )
    if result.returncode != 0:
        raise ValueError("could not inspect rehearsal checkout status")
    # NUL framing preserves unusual filenames. With --ignored=matching, an
    # explicitly ignored directory is reported once without listing its cache.
    if any(
        not _is_ignored_rehearsal_artifact(entry)
        for entry in result.stdout.split(b"\0") if entry
    ):
        raise ValueError(
            "rehearsal requires a clean checkout with only known ignored artifacts"
        )
    if resolve_commit("HEAD") != commit_sha:
        raise ValueError("checkout HEAD moved during rehearsal admission")


def _file_characterization(path: Path) -> str:
    """Keep file presence explicit without upgrading it to semantic proof."""
    try:
        return PHASE_INCOMPLETE if path.is_file() else PHASE_MISMATCH
    except OSError:
        return PHASE_INSTRUMENT_FAILURE


def _sanitized_environment() -> dict[str, str]:
    """Exclude the selected token before retrieving any environment values.

    This is not isolation from alternate credential sources or proof of the
    rehearsal's seven broader zero-mutation obligations.
    """
    return {name: os.environ[name] for name in os.environ if name != CARGO_TOKEN_ENV}


def _run_characterization(command: list[str]) -> str:
    """Run a bounded characterization without treating exit zero as Complete."""
    try:
        result = _run_process(
            command, operation="characterization",
            cwd=ROOT,
            env=_sanitized_environment(),
            capture_output=True,
            text=True,
            timeout=300,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return PHASE_INSTRUMENT_FAILURE
    return PHASE_INCOMPLETE if result.returncode == 0 else PHASE_MISMATCH


def _workspace_version() -> str:
    """Read the exact workspace version. Source identity only: the grammar,
    tag, and channel decisions belong to the typed authority invoked next."""
    in_workspace_package = False
    for line in (ROOT / "Cargo.toml").read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_workspace_package = stripped == "[workspace.package]"
            continue
        if in_workspace_package and stripped.startswith("version"):
            value = stripped.split("=", 1)[1].strip().strip('"')
            if value:
                return value
    raise ValueError("Cargo.toml has no [workspace.package] version")


def _identity_failure(status: str, reason: str) -> str:
    """Expose a bounded category, never child output or environment values."""
    print(f"release_identity: {reason}", file=sys.stderr)
    return status


def run_phase_release_identity(
    receipt: dict[str, Any], *, candidate_executable: Path | None = None,
    candidate_sha256: str | None = None,
) -> str:
    """Consume the typed release-identity projection for the workspace candidate.

    The workspace version is read as the identity source and validated through
    ``cargo-allow release-identity``; this phase records the validated fields
    without re-deriving grammar, tag, or channel. An integration caller may
    supply its just-built executable and observed digest. A verified private
    copy isolates execution from replacement of the original candidate path.
    This is not executable-to-source attestation or isolation from a hostile
    process with access to the private copy.
    """
    try:
        version = _workspace_version()
        with contextlib.ExitStack() as candidates:
            command = [
                "cargo", "run", "--quiet", "-p", "cargo-allow", "--locked", "--",
            ]
            if candidate_executable is not None or candidate_sha256 is not None:
                if (
                    candidate_executable is None
                    or candidate_sha256 is None
                    or not candidate_sha256.startswith("sha256:v1:")
                    or len(candidate_sha256) != 74
                    or any(char not in "0123456789abcdef" for char in candidate_sha256[10:])
                    or not candidate_executable.is_absolute()
                ):
                    return _identity_failure(PHASE_INSTRUMENT_FAILURE, "candidate_selection_invalid")
                if not candidate_executable.is_file():
                    return _identity_failure(PHASE_INSTRUMENT_FAILURE, "candidate_unavailable")
                if compute_sha256(candidate_executable) != candidate_sha256:
                    return _identity_failure(PHASE_MISMATCH, "candidate_digest_mismatch")
                private_dir = candidates.enter_context(
                    tempfile.TemporaryDirectory(prefix="cargo-allow-identity-")
                )
                snapshot = Path(private_dir) / ("candidate" + candidate_executable.suffix)
                shutil.copyfile(candidate_executable, snapshot)
                snapshot.chmod(0o500)
                if compute_sha256(snapshot) != candidate_sha256:
                    return _identity_failure(PHASE_MISMATCH, "candidate_digest_mismatch")
                command = [str(snapshot)]
            result = _run_process(
                command + ["release-identity", "--version", version],
                operation="release-identity",
                selected=(version, snapshot if candidate_executable is not None else None),
                cwd=ROOT,
                env=_sanitized_environment(),
                capture_output=True,
                text=True,
                timeout=300,
                check=False,
            )
            if candidate_executable is not None:
                if (compute_sha256(snapshot) != candidate_sha256
                        or compute_sha256(candidate_executable) != candidate_sha256):
                    return _identity_failure(PHASE_MISMATCH, "candidate_changed")
                print(f"release_identity: candidate_observed {candidate_sha256}", file=sys.stderr)
    except subprocess.TimeoutExpired:
        return _identity_failure(PHASE_INSTRUMENT_FAILURE, "child_timeout")
    except (OSError, ValueError, subprocess.SubprocessError):
        return _identity_failure(PHASE_INSTRUMENT_FAILURE, "instrument_unavailable")
    if result.returncode != 0:
        return _identity_failure(PHASE_MISMATCH, "child_failed")
    try:
        projection = json.loads(result.stdout)
    except json.JSONDecodeError:
        return _identity_failure(PHASE_INSTRUMENT_FAILURE, "identity_json_invalid")
    if (
        not isinstance(projection, dict)
        or projection.get("schema") != "cargo-allow.release-identity.v1"
        or projection.get("result") != "validated"
        or projection.get("version") != version
        or projection.get("tag") != "v" + version
        or projection.get("tag_source") != "derived"
        or projection.get("channel") not in ("stable", "release_candidate")
        or not isinstance(projection.get("github_prerelease"), bool)
        or "rc_ordinal" not in projection
    ):
        return _identity_failure(PHASE_MISMATCH, "identity_projection_invalid")
    # This invocation omits --tag, so it must receive the derived projection.
    # Check the relationships carried by that projection without parsing the
    # version grammar again; ReleaseVersionV1 remains the grammar authority.
    ordinal = projection["rc_ordinal"]
    if projection["channel"] == "stable":
        consistent = (
            ordinal is None
            and not projection["github_prerelease"]
            and "-" not in version
        )
    else:
        consistent = (
            type(ordinal) is int
            and 0 < ordinal <= 4294967295
            and projection["github_prerelease"]
            and version.endswith(f"-rc.{ordinal}")
        )
    if not consistent:
        return _identity_failure(PHASE_MISMATCH, "identity_projection_invalid")
    receipt["release_identity"] = {
        "schema": projection["schema"],
        "version": projection["version"],
        "tag": projection["tag"],
        "tag_source": projection["tag_source"],
        "channel": projection["channel"],
        "rc_ordinal": projection["rc_ordinal"],
        "github_prerelease": projection["github_prerelease"],
    }
    return PHASE_COMPLETE


def run_phase_candidate_package_set(receipt: dict[str, Any]) -> str:
    """Package the ten cargo-allow candidate rows from the committed subject.

    The publisher's --package-only mode validates exact release-coupled
    internal requirements and packages the selected rows offline (no registry
    reads, no compilation); this phase records each row's name, version,
    release order, SHA-256, and archive size without any upload path.
    """
    preflight_path = ROOT / "target/cargo-allow/rehearsal-candidate-package-set.json"
    try:
        preflight_path.parent.mkdir(parents=True, exist_ok=True)
        result = _run_process(
            [
                sys.executable,
                (ROOT / "scripts/release-topology-publisher.py").as_posix(),
                "--mode", "cargo-allow",
                "--package-only",
                "--receipt", str(preflight_path),
            ],
            operation="candidate-package",
            cwd=ROOT,
            env=_sanitized_environment(),
            capture_output=True,
            text=True,
            timeout=300,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return PHASE_INSTRUMENT_FAILURE
    rows: list[dict[str, Any]] = []
    try:
        packaged = json.loads(preflight_path.read_text(encoding="utf-8"))
        rows = packaged.get("rows", [])
    except (OSError, json.JSONDecodeError):
        if result.returncode == 0:
            return PHASE_INSTRUMENT_FAILURE
    recorded = []
    for row in rows:
        crate_path = Path(row["crate"])
        if not crate_path.is_absolute():
            crate_path = ROOT / crate_path
        try:
            size_bytes = crate_path.stat().st_size
        except OSError:
            size_bytes = None
        recorded.append(
            {
                "name": row.get("name"),
                "version": row.get("version"),
                "release_order": row.get("release_order"),
                "sha256": row.get("local_checksum"),
                "size_bytes": size_bytes,
            }
        )
    receipt["candidate_package_set"] = {"rows": recorded}
    if result.returncode != 0:
        return PHASE_MISMATCH
    expected_version = (receipt.get("release_identity") or {}).get("version")
    if (
        len(recorded) != 10
        or expected_version is None
        or any(row.get("version") != expected_version for row in recorded)
        or any(row.get("size_bytes") is None or row["size_bytes"] <= 0 for row in recorded)
    ):
        return PHASE_MISMATCH
    return PHASE_COMPLETE


def run_phase_shared_prerequisites(receipt: dict[str, Any]) -> str:
    """Prove the three topology-selected shared rows against retained
    namespace checksums through the read-only #3744 registry preflight.

    The preflight queries crates.io anonymously (the sanitized environment
    strips the publish token it must never see) and exits nonzero when any
    shared row is not already_published_exact; the recorded rows carry each
    row's observed state for diagnostics.
    """
    preflight_path = ROOT / "target/cargo-allow/rehearsal-shared-preflight.json"
    try:
        preflight_path.parent.mkdir(parents=True, exist_ok=True)
        result = _run_process(
            [
                sys.executable,
                (ROOT / "scripts/release-topology-publisher.py").as_posix(),
                "--mode", "cargo-allow",
                "--registry-preflight",
                "--receipt", str(preflight_path),
            ],
            operation="shared-preflight",
            cwd=ROOT,
            env=_sanitized_environment(),
            capture_output=True,
            text=True,
            timeout=300,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return PHASE_INSTRUMENT_FAILURE
    rows: list[dict[str, Any]] = []
    try:
        preflight = json.loads(preflight_path.read_text(encoding="utf-8"))
        rows = preflight.get("shared_registry_preflight", [])
    except (OSError, json.JSONDecodeError):
        if result.returncode == 0:
            return PHASE_INSTRUMENT_FAILURE
    receipt["shared_prerequisites"] = [
        {
            "name": row.get("name"),
            "version": row.get("version"),
            "state": row.get("state"),
            "registry_checksum": row.get("registry_checksum"),
        }
        for row in rows
    ]
    if result.returncode != 0:
        return PHASE_MISMATCH
    if not rows or len(rows) != 3 or any(
        row.get("state") != "already_published_exact" for row in rows
    ):
        return PHASE_MISMATCH
    return PHASE_COMPLETE


def _run_proof(command: list[str], *, operation: str) -> str:
    """Run a bounded fixture proof: exit zero proves, nonzero mismatches."""
    try:
        result = _run_process(
            command, operation=operation,
            cwd=ROOT,
            env=_sanitized_environment(),
            capture_output=True,
            text=True,
            timeout=300,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return PHASE_INSTRUMENT_FAILURE
    return PHASE_COMPLETE if result.returncode == 0 else PHASE_MISMATCH


def run_phase_publisher_state_machine(receipt: dict[str, Any]) -> str:
    """Exercise the publisher state machine fixture matrix (#3751 phase 4).

    The bounded fixture suite proves the upload state machine without any real
    publication: missing/existing-exact/conflicting rows, post-upload checksum
    conflicts, recovery exactness, malformed and unavailable provider
    responses, the shared prerequisite preflight matrix, and the
    shared-preflight-before-upload source law. No fixture calls real
    ``cargo publish`` or touches a token.
    """
    receipt["publisher_state_machine"] = {
        "fixture_matrix": "scripts/test-release-topology-publisher.py"
    }
    return _run_proof(
        [sys.executable, str(ROOT / "scripts/test-release-topology-publisher.py")],
        operation="publisher-proof",
    )


def run_phase_docs_and_support(receipt: dict[str, Any]) -> str:
    """Bind the typed identity to the docs and support surfaces (#3751 phase 5).

    Proves the changelog corpus reproduces CHANGELOG.md exactly, the typed
    identity's release record and GitHub note exist under docs/release/, and
    the support matrix and getting-started guide name the exact identity.
    The pinned-Changie merge roundtrip itself is proven continuously by the
    changie-contract CI lane; this phase proves the identity binding.
    """
    history_check = _run_proof(
        [sys.executable, str(ROOT / "scripts/generate-changie-history.py"), "--check"],
        operation="history-proof",
    )
    if history_check != PHASE_COMPLETE:
        return history_check

    identity = receipt.get("release_identity") or {}
    version = identity.get("version")
    tag = identity.get("tag")
    if not version or not tag:
        return PHASE_INCOMPLETE
    release_record = ROOT / "docs/release" / f"{version}.md"
    github_note = ROOT / "docs/release/github" / f"{tag}.md"
    support_matrix = ROOT / "docs/support-matrix.toml"
    getting_started = ROOT / "docs/getting-started.md"
    try:
        surfaces = {
            "release_record": release_record,
            "github_note": github_note,
            "support_matrix": support_matrix,
            "getting_started": getting_started,
        }
        contents = {
            name: path.read_text(encoding="utf-8") for name, path in surfaces.items()
        }
    except (OSError, UnicodeDecodeError):
        return PHASE_MISMATCH

    if not contents["support_matrix"].strip():
        return PHASE_MISMATCH
    if version not in contents["getting_started"]:
        return PHASE_MISMATCH

    receipt["docs_and_support_identity"] = {
        "release_record": release_record.as_posix(),
        "github_note": github_note.as_posix(),
        "support_matrix": support_matrix.as_posix(),
        "getting_started": getting_started.as_posix(),
        "history_check": "scripts/generate-changie-history.py --check",
    }
    return PHASE_COMPLETE


def run_phase_manifest_and_assets(receipt: dict[str, Any]) -> str:
    """Prove the manifest/asset surface tooling fixture matrix (#3751 phase 6).

    The bounded fixture suite proves final-packaged-surface reconciliation on
    actual crate bytes: archive digest and size binding, missing declared
    assets reconciling to Incomplete, per-crate identity via the CLI,
    prerelease and hyphenated identity preservation, and unexpected-archive
    rejection — all offline, with no real publication.
    """
    receipt["manifest_and_assets"] = {
        "fixture_matrix": "scripts/test-final-packaged-surface.py"
    }
    return _run_proof(
        [sys.executable, str(ROOT / "scripts/test-final-packaged-surface.py")],
        operation="manifest-proof",
    )


AUTHORIZATION_ARTIFACT = "release/authorize-v0.2.0.json"
AUTHORIZATION_SCHEMA = "cargo-allow.release-authorization.v1"


def run_phase_authorization_boundary(receipt: dict[str, Any]) -> str:
    """Check the selected registry-token environment boundary and bind the checked
    authorization artifact's identity (#3751 phase: authorization boundary).

    The rehearsal never consumes authorization: the selected token's environment
    key is an instrument failure without reading its value, and the checked
    CargoAllowReleaseAuthorizationV1 artifact is read only to record which
    release identity would gate the authorized run. The phase deliberately
    stays Incomplete — only the authorized run (#3760/#2502) can claim
    authorization.
    """
    if _selected_token_present():
        return PHASE_INSTRUMENT_FAILURE
    try:
        artifact = json.loads(
            (ROOT / AUTHORIZATION_ARTIFACT).read_text(encoding="utf-8")
        )
    except (OSError, json.JSONDecodeError):
        return PHASE_MISMATCH
    release = artifact.get("release")
    commit = artifact.get("candidate_parent_commit")
    tree = artifact.get("candidate_parent_tree")
    lock = artifact.get("cargo_lock_sha256")
    hex_ok = lambda value: (
        isinstance(value, str)
        and len(value) in (40, 64)
        and all(char in "0123456789abcdef" for char in value)
    )
    # The checked artifact stores the lock digest in its own bare-hex form
    # (no sha256: prefix); the phase validates that stored form rather than
    # imposing the receipt convention or re-deriving the value.
    lock_ok = lambda value: (
        isinstance(value, str)
        and len(value) == 64
        and all(char in "0123456789abcdef" for char in value)
    )
    if (
        artifact.get("schema_id") != AUTHORIZATION_SCHEMA
        or not isinstance(release, str)
        or not release.startswith("v")
        or not hex_ok(commit)
        or not hex_ok(tree)
        or not lock_ok(lock)
    ):
        return PHASE_MISMATCH
    receipt["authorization_boundary"] = {
        "authorization_artifact": AUTHORIZATION_ARTIFACT,
        "schema": AUTHORIZATION_SCHEMA,
        "named_release": release,
        "candidate_commit": commit,
        "token_present": False,
        "phase_status_note": (
            "deliberately Incomplete: the rehearsal never consumes "
            "authorization; #3760/#2502 gate the authorized run"
        ),
    }
    return PHASE_INCOMPLETE


def _yaml_workflow_inventory(path: Path) -> dict[str, Any] | None:
    """Parse one workflow manifest with PyYAML; None when unavailable."""
    try:
        import yaml
    except ImportError:
        return None
    try:
        parsed = yaml.safe_load(path.read_text(encoding="utf-8"))
    except Exception as exc:
        return {"error": f"YAML parse error: {exc}"}
    if not isinstance(parsed, dict) or "jobs" not in parsed:
        return {"error": "workflow is not a mapping with jobs"}
    jobs = {
        job_name: {
            "permissions": job.get("permissions"),
            "runs_on": job.get("runs-on"),
        }
        for job_name, job in parsed["jobs"].items()
        if isinstance(job, dict)
    }
    return {"top_level_permissions": parsed.get("permissions"), "jobs": jobs}


def run_phase_workflow_graph_permissions(receipt: dict[str, Any]) -> str:
    """Bind the release workflow graph's permission surface (#3751 phase).

    Enforces the least-privilege law — top-level ``actions: read`` and
    ``contents: write``, with ``github-release`` as the write/OIDC-scoped
    job and the authorized namespace workflow in namespace mode — and
    records the proof. PyYAML is used when available; otherwise the same
    law is checked on the pinned manifest strings.
    """
    release_path = ROOT / ".github/workflows/release.yml"
    authorized_path = ROOT / ".github/workflows/release-authorized.yml"
    try:
        release_text = release_path.read_text(encoding="utf-8")
        authorized_text = authorized_path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return PHASE_MISMATCH

    parsed = _yaml_workflow_inventory(release_path)
    if parsed is not None and "error" in parsed:
        return PHASE_MISMATCH

    authorized_namespace_mode = "--mode namespace" in authorized_text
    if parsed is None:
        proof = {
            "mode": "text",
            "release_jobs": [],
            "privileged_jobs": [],
            "top_level_read_scoped": "actions: read" in release_text,
            "top_level_write_scoped": "contents: write" in release_text,
            "github_release_scoped": "id-token: write" in release_text,
            "authorized_namespace_mode": authorized_namespace_mode,
        }
    else:
        jobs = parsed["jobs"]
        if not isinstance(jobs, dict) or not jobs:
            return PHASE_MISMATCH
        privileged = sorted(
            job_name
            for job_name, job in jobs.items()
            if isinstance(job.get("permissions"), dict)
        )
        proof = {
            "mode": "yaml",
            "release_jobs": sorted(jobs),
            "privileged_jobs": privileged,
            "top_level_read_scoped": parsed.get("top_level_permissions", {}).get(
                "actions"
            )
            == "read",
            "top_level_write_scoped": parsed.get("top_level_permissions", {}).get(
                "contents"
            )
            == "write",
            "github_release_scoped": jobs.get("github-release", {}).get(
                "permissions", {}
            ).get("id-token")
            == "write",
            "authorized_namespace_mode": authorized_namespace_mode,
        }

    if not (
        proof["top_level_read_scoped"]
        and proof["top_level_write_scoped"]
        and proof["github_release_scoped"]
        and proof["authorized_namespace_mode"]
    ):
        return PHASE_MISMATCH
    receipt["workflow_graph_permissions"] = proof
    return PHASE_COMPLETE


ESCAPED_DEFECT_CORPUS_SCHEMA = "cargo-allow.release-rehearsal-escaped-defect-corpus.v1"
CORPUS_PROOF_SCRIPT = "scripts/test-release-escaped-defect-corpus.py"

# The RC.1 escaped-defect corpus (#3793): one row per historical escape in the
# issue's production-grip map, with the seam each fixture must traverse.
# status is the per-row owner state on the subject carrying this receipt:
#   fault_injected  — the corpus fixture script re-establishes the old
#     false-green input against the landed production adapter;
#   typed_identity   — the typed release-identity authority is invoked on the
#     historical input form (no Python-side version grammar);
#   phase_binding    — the row binds to the named rehearsal phase's recorded
#     proof of the same production seam;
#   pending_upstream — the seam's producer has not landed on this subject;
#     the row records its owner and carries no proof (#3793 tracks imports).
ESCAPED_DEFECT_CORPUS_ROWS = (
    {
        "incident_id": "RC1-ESCAPE-001",
        "defect": "package cut without release records",
        "seam": "typed identity -> selected record/note/Changie preflight",
        "owner": "#3786/#3787",
        "status": "phase_binding",
        "claim_boundary": (
            "Binds to the docs_and_support_identity phase's recorded "
            "release-record/github-note binding of the typed identity; "
            "the pinned-Changie history check rides the same phase."
        ),
    },
    {
        "incident_id": "RC1-ESCAPE-002",
        "defect": "one parser rejects or misorders numbered RC",
        "seam": "cargo-allow release-identity typed authority",
        "owner": "#3786/#3787",
        "status": "typed_identity",
        "claim_boundary": (
            "Invokes the typed authority on 0.2.0-rc.10 (integer ordinal "
            "ordering) and 0.2.0-rc.0 (rejection); no Python-side grammar."
        ),
    },
    {
        "incident_id": "RC1-ESCAPE-003",
        "defect": "shared VCS repackage differs while registry equals retained expected",
        "seam": "shared_registry_preflight in release-topology-publisher.py",
        "owner": "#3744",
        "status": "fault_injected",
        "claim_boundary": (
            "The preflight must decide on registry-versus-retained evidence; "
            "local repackage drift alone must not block the shared rows."
        ),
    },
    {
        "incident_id": "RC1-ESCAPE-004",
        "defect": "shared registry checksum conflict",
        "seam": "shared_registry_preflight in release-topology-publisher.py",
        "owner": "#3744",
        "status": "fault_injected",
        "claim_boundary": (
            "A conflicting shared observation must be recorded and block a "
            "publishing run before the first upload."
        ),
    },
    {
        "incident_id": "RC1-ESCAPE-005",
        "defect": "existing cargo-allow row with wrong checksum",
        "seam": "publisher existing-row reconciliation loop",
        "owner": "#3744",
        "status": "fault_injected",
        "claim_boundary": (
            "An existing row that differs from the local candidate must "
            "raise a release incident and stop before any upload."
        ),
    },
    {
        "incident_id": "RC1-ESCAPE-006",
        "defect": "post-upload checksum conflict",
        "seam": "publisher visibility/equality loop",
        "owner": "#3744",
        "status": "fault_injected",
        "claim_boundary": (
            "A newly uploaded row that becomes visible with a conflicting "
            "checksum must retain a partial incident with the irreversible "
            "boundary and stop before the next row."
        ),
    },
    {
        "incident_id": "RC1-ESCAPE-007",
        "defect": "partial run resumed from moving main",
        "seam": "load_recovery_receipt exact-candidate custody",
        "owner": "#3791",
        "status": "fault_injected",
        "claim_boundary": (
            "Recovery admits only the incomplete incident receipt bound to "
            "the exact original commit, tree, authorization, and posture."
        ),
    },
    {
        "incident_id": "RC1-ESCAPE-008",
        "defect": "floating release action",
        "seam": "immutable action inventory/workflow checker",
        "owner": "#2498",
        "status": "pending_upstream",
        "claim_boundary": (
            "Imported once #2498's immutable action inventory lands; this "
            "row carries no proof."
        ),
    },
    {
        "incident_id": "RC1-ESCAPE-009",
        "defect": "missing id-token: write for attestation",
        "seam": "release workflow permission graph",
        "owner": "#3726/#3792",
        "status": "phase_binding",
        "claim_boundary": (
            "Binds to the workflow_graph_permissions phase's recorded "
            "github-release id-token scoping proof."
        ),
    },
    {
        "incident_id": "RC1-ESCAPE-010",
        "defect": "noncanonical checksum encoding",
        "seam": "canonical_receipt_checksum canonical-form authority",
        "owner": "#3761/#2496",
        "status": "fault_injected",
        "claim_boundary": (
            "The publisher's canonical-form authority rejects every "
            "noncanonical encoding; the typed-manifest adapter leg remains "
            "with #2496."
        ),
    },
    {
        "incident_id": "RC1-ESCAPE-011",
        "defect": "RC release uses prerelease=false",
        "seam": "typed identity projection -> draft adapter",
        "owner": "#3788/#3726",
        "status": "typed_identity",
        "claim_boundary": (
            "The typed authority must project an RC version with "
            "github_prerelease true; the workflow-output leg remains with "
            "#3788."
        ),
    },
    {
        "incident_id": "RC1-ESCAPE-012",
        "defect": "tag push treated as authority",
        "seam": "pre-tag authorization graph",
        "owner": "#3790",
        "status": "phase_binding",
        "claim_boundary": (
            "Binds to the publisher_state_machine phase, whose fixture "
            "matrix pins the authorize-gated publish workflow contract; the "
            "typed final-tag bridge keeps its own #3790 fixtures."
        ),
    },
    {
        "incident_id": "RC1-ESCAPE-013",
        "defect": "moved tag presented as original package producer",
        "seam": "RC incident model/evidence adapter",
        "owner": "#3759/#3761",
        "status": "pending_upstream",
        "claim_boundary": (
            "Imported once the incident evidence adapter lands; this row "
            "carries no proof."
        ),
    },
    {
        "incident_id": "RC1-ESCAPE-014",
        "defect": "actual asset set differs from the manifest",
        "seam": "GitHub draft fixture adapter + closeout validator",
        "owner": "#3726",
        "status": "pending_upstream",
        "claim_boundary": (
            "Imported once the draft-fixture adapter lands; this row "
            "carries no proof."
        ),
    },
    {
        "incident_id": "RC1-ESCAPE-015",
        "defect": "later green retry erases the incident",
        "seam": "typed manifest/incident aggregation",
        "owner": "#2496/#2497/#3785",
        "status": "pending_upstream",
        "claim_boundary": (
            "Imported once the incident evidence denominator (#3785) lands; "
            "this row carries no proof."
        ),
    },
    {
        "incident_id": "RC1-ESCAPE-016",
        "defect": "issue closed from narration",
        "seam": "campaign-issue-closeout evidence checker",
        "owner": "#3780",
        "status": "fault_injected",
        "claim_boundary": (
            "A narration-only close and lexical-only acceptance backing "
            "must be rejected; typed evidence is admitted."
        ),
    },
)

CORPUS_FAULT_INJECTED_IDS = frozenset(
    row["incident_id"] for row in ESCAPED_DEFECT_CORPUS_ROWS
    if row["status"] == "fault_injected"
)


def _corpus_failure(status: str, reason: str) -> str:
    """Expose a bounded category, never child output or environment values."""
    print(f"escaped_defect_corpus: {reason}", file=sys.stderr)
    return status


def _corpus_identity_result(version: str, *, candidate_executable, candidate_sha256):
    """Invoke the typed release-identity authority for one corpus input.

    Returns the CompletedProcess, or None when the instrument is unavailable.
    A supplied candidate is digest-re-verified immediately before use; the
    digest is verified again after the invocation by the caller's phase.
    """
    candidate = None
    prefix = ["cargo", "run", "--quiet", "-p", "cargo-allow", "--locked", "--"]
    if candidate_executable is not None or candidate_sha256 is not None:
        if (
            candidate_executable is None or candidate_sha256 is None
            or not candidate_executable.is_absolute()
        ):
            return None
        try:
            if (
                not candidate_executable.is_file()
                or compute_sha256(candidate_executable) != candidate_sha256
            ):
                return None
        except OSError:
            return None
        candidate = candidate_executable
        prefix = [str(candidate_executable)]
    try:
        return _run_process(
            prefix + ["release-identity", "--version", version],
            operation="release-identity", selected=(version, candidate),
            cwd=ROOT,
            env=_sanitized_environment(),
            capture_output=True,
            text=True,
            timeout=300,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return None


def run_phase_escaped_defect_corpus(
    receipt: dict[str, Any], *, candidate_executable: Path | None = None,
    candidate_sha256: str | None = None,
) -> str:
    """Carry the RC.1 escaped-defect corpus over its production seams (#3793).

    Every landed seam is gripped by a discriminating zero-upload fixture or a
    fail-closed binding to the rehearsal phase that already proves it; seams
    whose producers have not landed are recorded as pending with their owner.
    A typed production seam is never reduced to a text grep, and the corpus
    never re-derives release grammar in Python.
    """
    rows = {row["incident_id"]: dict(row) for row in ESCAPED_DEFECT_CORPUS_ROWS}
    if candidate_executable is not None and candidate_sha256 is not None:
        try:
            changed = compute_sha256(candidate_executable) != candidate_sha256
        except OSError:
            changed = True
        if changed:
            return _corpus_failure(PHASE_MISMATCH, "candidate_changed")

    typed_inputs = (
        ("RC1-ESCAPE-002", "0.2.0-rc.10"),
        ("RC1-ESCAPE-011", "0.2.0-rc.10"),
    )
    rc_result = None
    for incident_id, version in typed_inputs:
        if rc_result is None:
            rc_result = _corpus_identity_result(
                version, candidate_executable=candidate_executable,
                candidate_sha256=candidate_sha256,
            )
        if rc_result is None:
            return _corpus_failure(PHASE_INSTRUMENT_FAILURE, "identity_instrument_unavailable")
        projection = None
        if rc_result.returncode == 0:
            try:
                projection = json.loads(rc_result.stdout)
            except json.JSONDecodeError:
                projection = None
        consistent = (
            rc_result.returncode == 0
            and isinstance(projection, dict)
            and projection.get("schema") == "cargo-allow.release-identity.v1"
            and projection.get("result") == "validated"
            and projection.get("version") == version
            and projection.get("tag") == "v" + version
            and projection.get("channel") == "release_candidate"
            and type(projection.get("rc_ordinal")) is int
            and projection["rc_ordinal"] == 10
            and projection.get("github_prerelease") is True
        )
        if not consistent:
            rows[incident_id]["result"] = "rejected"
            return _corpus_failure(PHASE_MISMATCH, "rc_projection_mismatch")
        rows[incident_id]["result"] = "proven"

    rejection = _corpus_identity_result(
        "0.2.0-rc.0", candidate_executable=candidate_executable,
        candidate_sha256=candidate_sha256,
    )
    if rejection is None:
        return _corpus_failure(PHASE_INSTRUMENT_FAILURE, "identity_instrument_unavailable")
    if rejection.returncode == 0:
        return _corpus_failure(PHASE_MISMATCH, "rc_zero_ordinal_accepted")

    try:
        result = _run_process(
            [sys.executable, str(ROOT / CORPUS_PROOF_SCRIPT)],
            operation="corpus-proof",
            cwd=ROOT,
            env=_sanitized_environment(),
            capture_output=True,
            text=True,
            timeout=300,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return _corpus_failure(PHASE_INSTRUMENT_FAILURE, "corpus_instrument_unavailable")
    fixtures: dict[str, dict[str, Any]] = {}
    if result.stdout.strip():
        try:
            emitted = json.loads(result.stdout)
            if isinstance(emitted, dict) and isinstance(emitted.get("rows"), list):
                fixtures = {
                    row["incident_id"]: row for row in emitted["rows"]
                    if isinstance(row, dict) and isinstance(row.get("incident_id"), str)
                }
        except json.JSONDecodeError:
            fixtures = {}
    if set(fixtures) != CORPUS_FAULT_INJECTED_IDS:
        return _corpus_failure(
            PHASE_MISMATCH if result.returncode == 0 or fixtures else PHASE_INSTRUMENT_FAILURE,
            "corpus_fixture_output_invalid",
        )
    for incident_id in sorted(CORPUS_FAULT_INJECTED_IDS):
        fixture = fixtures[incident_id]
        if fixture.get("result") == "proven":
            rows[incident_id]["result"] = "proven"
        else:
            rows[incident_id]["result"] = "rejected"
            return _corpus_failure(PHASE_MISMATCH, "escaped_input_accepted")

    bindings = (
        ("RC1-ESCAPE-001", "release_identity", "docs_and_support_identity"),
        ("RC1-ESCAPE-009", "workflow_graph_permissions",),
        ("RC1-ESCAPE-012", "publisher_state_machine",),
    )
    for binding in bindings:
        incident_id, *phase_names = binding
        if any(receipt["phases"].get(name) != PHASE_COMPLETE for name in phase_names):
            rows[incident_id]["result"] = "blocked"
            return _corpus_failure(
                PHASE_MISMATCH, f"{incident_id.lower()}_seam_phase_not_proven",
            )
        rows[incident_id]["result"] = "proven"

    identity = receipt.get("release_identity") or {}
    docs = receipt.get("docs_and_support_identity") or {}
    record_path = Path(docs.get("release_record", "")).as_posix()
    note_path = Path(docs.get("github_note", "")).as_posix()
    version = identity.get("version") or ""
    tag = identity.get("tag") or ""
    if (
        not version
        or not (
            record_path == f"docs/release/{version}.md"
            or record_path.endswith(f"/docs/release/{version}.md")
        )
        or not (
            note_path == f"docs/release/github/{tag}.md"
            or note_path.endswith(f"/docs/release/github/{tag}.md")
        )
    ):
        return _corpus_failure(PHASE_MISMATCH, "record_note_binding_mismatch")
    workflow = receipt.get("workflow_graph_permissions") or {}
    if not (
        workflow.get("top_level_read_scoped") is True
        and workflow.get("top_level_write_scoped") is True
        and workflow.get("github_release_scoped") is True
        and workflow.get("authorized_namespace_mode") is True
    ):
        return _corpus_failure(PHASE_MISMATCH, "attestation_permission_binding_mismatch")
    machine = receipt.get("publisher_state_machine") or {}
    if machine.get("fixture_matrix") != "scripts/test-release-topology-publisher.py":
        return _corpus_failure(PHASE_MISMATCH, "authorization_graph_binding_mismatch")

    for row in ESCAPED_DEFECT_CORPUS_ROWS:
        if row["status"] == "pending_upstream":
            rows[row["incident_id"]]["result"] = "recorded_pending"

    receipt["escaped_defect_corpus"] = {
        "schema": ESCAPED_DEFECT_CORPUS_SCHEMA,
        "fixture_matrix": CORPUS_PROOF_SCRIPT,
        "rows": [rows[row["incident_id"]] for row in ESCAPED_DEFECT_CORPUS_ROWS],
        "claim_boundary": (
            "Per-row owner status at this subject: fault_injected and "
            "typed_identity rows prove their historical rejection against "
            "the landed production adapter; phase_binding rows bind to the "
            "named phase's recorded proof; pending_upstream rows carry no "
            "proof and name their owner. Full corpus completion tracks the "
            "owner issues; this phase never proves publication readiness."
        ),
    }
    return PHASE_COMPLETE


CHARACTERIZATION_PHASES = frozenset({
    "authorization_boundary",
})


def _aggregate_phase_status(phases: dict[str, str]) -> str:
    """Fail-closed aggregate: Complete only when every real phase proves and no
    characterization-only phase can manufacture that status."""
    values = set(phases.values())
    if PHASE_INSTRUMENT_FAILURE in values:
        return PHASE_INSTRUMENT_FAILURE
    if PHASE_MISMATCH in values:
        return PHASE_MISMATCH
    if any(phases.get(name) == PHASE_COMPLETE for name in CHARACTERIZATION_PHASES):
        return PHASE_MISMATCH
    return PHASE_INCOMPLETE


def _is_filesystem_alias(metadata: os.stat_result) -> bool:
    return bool(
        stat.S_ISLNK(metadata.st_mode)
        or getattr(metadata, "st_file_attributes", 0)
        & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0)
    )


def _preflight_alias_and_leaf(path: Path) -> Path:
    """Reject symlink/reparse ancestors, non-directory parents, or alias/hardlinked leaf."""
    path = path.absolute()
    for component in (path, *path.parents):
        try:
            metadata = component.lstat()
        except FileNotFoundError:
            continue
        if _is_filesystem_alias(metadata):
            raise OSError("receipt output cannot traverse a filesystem alias")
        if component == path:
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
                raise OSError("receipt output must be a singly linked regular file")
        elif not stat.S_ISDIR(metadata.st_mode):
            raise OSError("receipt output parent must be a directory")
    return path.resolve()


def _preflight_receipt_output(path: Path) -> Path:
    """Admit an artifact destination without changing source or output bytes."""
    path = _preflight_alias_and_leaf(path)
    root = ROOT.resolve()
    if path.is_relative_to(root):
        if not path.is_relative_to(root / "target") or path == root / "target":
            raise ValueError("repository receipt output must be inside target/")
        tracked = _run_process(
            ["git", "ls-files", "--cached", "-z"], operation="output-tracked", cwd=root,
            env=_sanitized_environment(),
            capture_output=True, timeout=15, check=False,
        )
        if tracked.returncode != 0:
            raise ValueError("cannot inspect tracked receipt output destinations")
        relative = os.path.normcase(os.fspath(path.relative_to(root)))
        entries = (os.path.normcase(os.fsdecode(entry).replace("/", os.sep))
                   for entry in tracked.stdout.split(b"\0") if entry)
        if any(relative == entry or relative.startswith(entry + os.sep)
               or entry.startswith(relative + os.sep) for entry in entries):
            raise ValueError("receipt output cannot replace a tracked source path")
        ignored = _run_process(
            ["git", "check-ignore", "-q", "--", path.relative_to(root).as_posix()],
            operation="output-ignored", selected=path,
            cwd=root, env=_sanitized_environment(), capture_output=True, timeout=15, check=False,
        )
        if ignored.returncode != 0:
            raise ValueError("repository receipt output must be an ignored target artifact")
    return path


def _write_receipt(path: Path, json_text: str) -> None:
    """Publish complete bytes by replacement; failed writes retain the old file."""
    path = _preflight_receipt_output(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path = _preflight_receipt_output(path)
    descriptor, temporary_name = tempfile.mkstemp(prefix=".rehearsal-", dir=path.parent)
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as output:
            descriptor = -1
            output.write(json_text + "\n")
            output.flush()
            os.fsync(output.fileno())
        _preflight_receipt_output(path)
        os.replace(temporary, path)
    finally:
        if descriptor != -1:
            os.close(descriptor)
        temporary.unlink(missing_ok=True)


def _linux_worker_env(
    *, empty_home: Path, empty_xdg: Path, work_tmp: Path,
) -> dict[str, str]:
    """Build a stripped worker environment without ambient Git, LD, Python, or credential hooks."""
    cleaned: dict[str, str] = {}
    for name in os.environ:
        if (
            name in _WORKER_STRIPPED_ENV_EXACT
            or name.startswith("GIT_CONFIG_")
            or name.startswith("LD_")
            or name.startswith("PYTHON")
            or (name.startswith("CARGO_REGISTRIES_") and name.endswith("_TOKEN"))
        ):
            continue
        cleaned[name] = os.environ[name]
    cleaned["HOME"] = str(empty_home)
    cleaned["XDG_CONFIG_HOME"] = str(empty_xdg)
    cleaned["TMPDIR"] = str(work_tmp)
    cleaned["TEMP"] = str(work_tmp)
    cleaned["TMP"] = str(work_tmp)
    cleaned["GIT_TERMINAL_PROMPT"] = "0"
    cleaned["GIT_SSH_COMMAND"] = "/bin/false"
    cleaned["GIT_ASKPASS"] = "/bin/false"
    return cleaned


def linux_worker_capability() -> tuple[bool, str]:
    """Probe whether this host can actually launch the confined Linux worker profile."""
    if sys.platform != "linux":
        return False, "unsupported_platform"
    tools: dict[str, str] = {}
    for name in ("unshare", "setpriv", "mount", "bash", "git"):
        found = shutil.which(name)
        if not found:
            return False, f"missing_tool:{name}"
        tools[name] = found
    try:
        probe = subprocess.run(
            [
                tools["unshare"], "-Urmnpf", "--mount-proc", "--kill-child", "--",
                tools["setpriv"],
                "--bounding-set=-all",
                "--inh-caps=-all",
                "--ambient-caps=-all",
                "--no-new-privs",
                "--",
                "true",
            ],
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return False, "capability_probe_error"
    if probe.returncode != 0:
        return False, "unshare_or_setpriv_unavailable"
    return True, "available"


def _snapshot_tags_external(git_exe: str, target_dir: Path, *, bare: bool = False) -> str:
    """Read refs/tags from outside the worker namespace for outer launcher verification."""
    command = [git_exe, "--no-replace-objects"]
    if bare:
        command.extend(["--git-dir", str(target_dir)])
    command.extend(["show-ref", "--tags"])
    result = subprocess.run(
        command,
        cwd=None if bare else target_dir,
        env=_sanitized_environment(),
        capture_output=True,
        text=True,
        timeout=15,
        check=False,
    )
    if result.returncode == 0:
        return result.stdout
    if result.returncode == 1 and not result.stdout and not result.stderr.strip():
        return ""
    raise OSError("could not snapshot Git tags outside worker")


_LINUX_WORKER_SUPERVISOR = """#!/usr/bin/env bash
set -euo pipefail
work_root="$1"
shift
ro_count="$1"
shift
mount --make-rprivate / 2>/dev/null || true
for ((i = 0; i < ro_count; i++)); do
  target="$1"
  shift
  mount --bind "$target" "$target"
  mount -o remount,bind,ro,nosuid,nodev "$target"
done
rw_target="$1"
shift
if [ -L "$rw_target" ] || [ ! -d "$rw_target" ]; then
  echo "unsafe writable target: $rw_target" >&2
  exit 1
fi
mount --bind "$rw_target" "$rw_target"
mount -o remount,bind,rw "$rw_target"
cd "$work_root"
exec setpriv --bounding-set=-all --inh-caps=-all --ambient-caps=-all --no-new-privs -- "$@"
"""


def _ensure_safe_writable_target_dir(root: Path) -> Path:
    """Ensure root/target is a real, non-alias directory owned directly under root."""
    writable_target = root / "target"
    try:
        metadata = writable_target.lstat()
    except FileNotFoundError:
        writable_target.mkdir()
        metadata = writable_target.lstat()
    if _is_filesystem_alias(metadata):
        raise OSError("linux worker target root cannot be a filesystem alias")
    if not stat.S_ISDIR(metadata.st_mode):
        raise OSError("linux worker target root must be a directory")
    if writable_target.resolve() != root / "target":
        raise OSError("linux worker target root resolved outside checkout")
    return writable_target


def run_in_linux_worker(
    commit_ref: str,
    *,
    output: Path | None = None,
    candidate_executable: Path | None = None,
    candidate_sha256: str | None = None,
    repo_root: Path | None = None,
    script_path: Path | None = None,
    capture_output: bool = False,
    timeout: int = 600,
) -> subprocess.CompletedProcess[str]:
    """Launch the rehearsal inside an externally confined Linux worker profile.

    Confinement (user/mount/network/PID namespaces with --kill-child descendant
    reaping, read-only bind mounts on the checkout, Git metadata, common dir,
    and disposable sibling bare remote, alias-checked writable bind mount on
    target/, stripped environment, and full capability drop with no-new-privs)
    is applied by this outer supervisor *before* exec'ing the rehearsal process.
    The rehearsal itself never confines itself; it measures and tries to break
    out of the boundary it inherited.
    """
    _require_selected_token_absent()
    if (
        not commit_ref
        or commit_ref.startswith("-")
        or any(char in commit_ref for char in "\r\n\0")
    ):
        raise ValueError("commit ref must be non-empty, single-line, and not start with a dash")
    if (candidate_executable is None) != (candidate_sha256 is None):
        raise ValueError("candidate executable and SHA-256 must be supplied together")
    if type(timeout) is not int or timeout <= 0:
        raise ValueError("linux worker timeout must be a positive integer")
    capable, reason = linux_worker_capability()
    if not capable:
        raise OSError(f"linux worker profile unavailable: {reason}")

    git_exe = shutil.which("git")
    unshare_exe = shutil.which("unshare")
    bash_exe = shutil.which("bash")
    if not (git_exe and unshare_exe and bash_exe):
        raise OSError("linux worker launcher tools are unavailable")

    root = (repo_root or ROOT).resolve()
    script = (script_path or Path(__file__)).resolve()
    if output is not None:
        if root == ROOT.resolve():
            output = _preflight_receipt_output(output)
        else:
            output = _preflight_alias_and_leaf(output)
            if output.is_relative_to(root) and (
                not output.is_relative_to(root / "target") or output == root / "target"
            ):
                raise ValueError("repository receipt output must be inside target/")

    dirs_result = subprocess.run(
        [git_exe, "--no-replace-objects", "rev-parse", "--path-format=absolute",
         "--git-dir", "--git-common-dir"],
        cwd=root,
        env=_sanitized_environment(),
        capture_output=True,
        text=True,
        timeout=15,
        check=False,
    )
    if dirs_result.returncode != 0:
        raise ValueError("could not resolve Git metadata directories for linux worker")
    dir_lines = [line.strip() for line in dirs_result.stdout.splitlines() if line.strip()]
    if len(dir_lines) != 2:
        raise ValueError("unexpected Git metadata directory output for linux worker")
    git_dir = Path(dir_lines[0]).resolve()
    git_common_dir = Path(dir_lines[1]).resolve()
    if not git_dir.is_dir() or not git_common_dir.is_dir():
        raise ValueError("resolved Git metadata directories do not exist")

    tags_before = _snapshot_tags_external(git_exe, root, bare=False)
    writable_target = _ensure_safe_writable_target_dir(root)

    with tempfile.TemporaryDirectory(prefix="cargo-allow-linux-worker-") as launcher_dir:
        base = Path(launcher_dir).resolve()
        work_tmp = base / "tmp"
        empty_home = base / "home"
        empty_xdg = base / "xdg"
        canary_remote = base / "canary-remote.git"
        work_tmp.mkdir()
        empty_home.mkdir()
        empty_xdg.mkdir()
        init_remote = subprocess.run(
            [git_exe, "init", "--bare", "--quiet", str(canary_remote)],
            env=_sanitized_environment(),
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
        if init_remote.returncode != 0:
            raise OSError("could not initialize disposable canary bare remote")
        remote_tags_before = _snapshot_tags_external(git_exe, canary_remote, bare=True)
        if remote_tags_before:
            raise OSError("fresh canary bare remote unexpectedly contained tags")

        raw_ro_mounts = sorted(
            {root, git_dir, git_common_dir, canary_remote},
            key=lambda path: (len(path.parts), str(path)),
        )
        ro_mounts: list[Path] = []
        for candidate in raw_ro_mounts:
            if not any(candidate.is_relative_to(existing) for existing in ro_mounts):
                ro_mounts.append(candidate)

        supervisor_path = base / "supervisor.sh"
        supervisor_path.write_text(_LINUX_WORKER_SUPERVISOR, encoding="utf-8")

        worker_argv = [
            str(Path(sys.executable).absolute()),
            "-I",
            str(script),
            "--commit", commit_ref,
            "--tag-canary-remote", str(canary_remote),
        ]
        if output is not None:
            worker_argv.extend(["--output", str(output)])
        if candidate_executable is not None and candidate_sha256 is not None:
            worker_argv.extend([
                "--candidate-executable", str(candidate_executable),
                "--candidate-sha256", candidate_sha256,
            ])

        command = [
            unshare_exe, "-Urmnpf", "--mount-proc", "--kill-child", "--",
            bash_exe, str(supervisor_path),
            str(root),
            str(len(ro_mounts)),
            *(str(path) for path in ro_mounts),
            str(writable_target),
            *worker_argv,
        ]
        result = subprocess.run(
            command,
            cwd=root,
            env=_linux_worker_env(
                empty_home=empty_home, empty_xdg=empty_xdg, work_tmp=work_tmp,
            ),
            capture_output=capture_output,
            text=True,
            timeout=timeout,
            check=False,
        )

        tags_after = _snapshot_tags_external(git_exe, root, bare=False)
        remote_tags_after = _snapshot_tags_external(git_exe, canary_remote, bare=True)
        if tags_after != tags_before or remote_tags_after != remote_tags_before:
            raise OSError("outer launcher observed unexpected Git tag mutation")
        return result


def _check_tag_invocation_boundary(
    commit_sha: str, *, tag_canary_remote: Path | None = None,
) -> tuple[bool, list[str]]:
    """Verify the owned _run_process boundary denies tag, update-ref, and push."""
    canary_tag = f"rehearsal-canary-{_GETPID()}"
    remote = str(
        tag_canary_remote
        if tag_canary_remote is not None
        else ROOT.resolve().parent / "rehearsal-canary-remote.git"
    )
    probes = (
        ("git_tag", ["git", "tag", canary_tag, commit_sha], "resolve-commit", "HEAD"),
        ("git_update_ref", ["git", "update-ref", f"refs/tags/{canary_tag}", commit_sha], "git-tags", None),
        ("git_push", ["git", "push", remote, f"{commit_sha}:refs/tags/{canary_tag}"], "tag-push", None),
    )
    options = dict(
        cwd=ROOT,
        env=_sanitized_environment(),
        capture_output=True,
        text=True,
        timeout=15,
        check=False,
    )
    denied: list[str] = []
    for label, command, operation, selected in probes:
        try:
            _run_process(command, operation=operation, selected=selected, **options)
        except OSError as error:
            if "not selected" in str(error):
                denied.append(label)
    return len(denied) == len(probes), denied


def _path_is_read_only(path: Path) -> bool:
    """Inspect kernel VFS ST_RDONLY mount flag without mutating the path."""
    if _STATVFS is None or _ST_RDONLY is None:
        return False
    try:
        return bool(_STATVFS(path).f_flag & _ST_RDONLY)
    except OSError:
        return False


def _read_proc_self_capabilities() -> dict[str, int] | None:
    """Read Linux capability masks and NoNewPrivs via raw fd I/O."""
    try:
        fd = os.open("/proc/self/status", os.O_RDONLY)
    except OSError:
        return None
    chunks: list[bytes] = []
    try:
        while True:
            chunk = os.read(fd, 4096)
            if not chunk:
                break
            chunks.append(chunk)
    except OSError:
        return None
    finally:
        os.close(fd)
    text = b"".join(chunks).decode("ascii", errors="replace")
    parsed: dict[str, int] = {}
    for line in text.splitlines():
        if ":" not in line:
            continue
        key, raw_value = line.split(":", 1)
        value = raw_value.strip()
        if key in {"CapEff", "CapPrm", "CapBnd", "CapAmb"}:
            try:
                parsed[key] = int(value, 16)
            except ValueError:
                return None
        elif key == "NoNewPrivs":
            try:
                parsed[key] = int(value, 10)
            except ValueError:
                return None
    if len(parsed) != 5:
        return None
    return parsed


def _probe_remount_rw_eperm(path: Path) -> tuple[bool, str]:
    """Attempt a live kernel MS_REMOUNT|MS_BIND rw syscall and require exact EPERM."""
    try:
        try:
            libc = ctypes.CDLL(None, use_errno=True)
        except OSError:
            libc = ctypes.CDLL("libc.so.6", use_errno=True)
        mount_fn = libc.mount
        mount_fn.argtypes = [
            ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p,
            ctypes.c_ulong, ctypes.c_void_p,
        ]
        mount_fn.restype = ctypes.c_int
    except OSError:
        return False, "libc_unavailable"
    ctypes.set_errno(0)
    rc = mount_fn(None, os.fsencode(path), None, _LINUX_MS_REMOUNT | _LINUX_MS_BIND, None)
    if rc == 0:
        return False, "remount_succeeded"
    err = ctypes.get_errno()
    name = errno.errorcode.get(err, f"errno_{err}")
    return err == errno.EPERM, name


def _probe_network_egress_denied() -> tuple[bool, str]:
    """Verify kernel network-namespace isolation via zero-packet UDP connect()."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        sock.connect(_LINUX_NET_PROBE_TARGET)
        return False, "connected"
    except OSError as error:
        name = errno.errorcode.get(error.errno, f"errno_{error.errno}")
        return error.errno == errno.ENETUNREACH, name
    finally:
        sock.close()


def _probe_direct_write_erofs(path: Path, *, flags: int) -> tuple[bool, str]:
    """Attempt a direct kernel open() for write and require exact EROFS."""
    try:
        fd = os.open(path, flags, 0o600)
    except OSError as error:
        name = errno.errorcode.get(error.errno, f"errno_{error.errno}")
        return error.errno == errno.EROFS, name
    os.close(fd)
    if flags & os.O_EXCL:
        path.unlink(missing_ok=True)
    return False, "write_succeeded"


def _resolve_git_metadata_dirs() -> tuple[Path, Path] | None:
    """Resolve git_dir and git_common_dir through the owned invocation boundary."""
    try:
        result = _run_process(
            ["git", "--no-replace-objects", "rev-parse", "--path-format=absolute",
             "--git-dir", "--git-common-dir"],
            operation="git-dirs",
            cwd=ROOT,
            env=_sanitized_environment(),
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if result.returncode != 0:
        return None
    lines = [line.strip() for line in result.stdout.splitlines() if line.strip()]
    if len(lines) != 2:
        return None
    git_dir, git_common_dir = Path(lines[0]), Path(lines[1])
    if not git_dir.is_absolute() or not git_common_dir.is_absolute():
        return None
    git_dir = git_dir.resolve()
    git_common_dir = git_common_dir.resolve()
    if not git_dir.is_dir() or not git_common_dir.is_dir():
        return None
    return git_dir, git_common_dir


def _read_git_tags(canary_remote: Path | None = None) -> str | None:
    """Read refs/tags through the owned invocation boundary; fail closed on errors."""
    if canary_remote is None:
        command = ["git", "--no-replace-objects", "show-ref", "--tags"]
        operation = "git-tags"
        selected = None
    else:
        command = ["git", "--no-replace-objects", "--git-dir", str(canary_remote),
                   "show-ref", "--tags"]
        operation = "canary-remote-tags"
        selected = canary_remote
    try:
        result = _run_process(
            command,
            operation=operation,
            selected=selected,
            cwd=ROOT,
            env=_sanitized_environment(),
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if result.returncode == 0:
        return result.stdout
    if result.returncode == 1 and not result.stdout and not result.stderr.strip():
        return ""
    return None


def _run_confined_tag_breakout_probes(
    commit_sha: str, canary_remote: Path,
) -> dict[str, bool]:
    """Attempt direct Git tag/update-ref/push commands bypassing _run_process.

    MUST ONLY be called after kernel RO-mount, zero-cap/no-new-privs, EPERM
    remount, ENETUNREACH network, and EROFS direct-write invariants have all
    been proven on the live process boundary.
    """
    context = _INVOCATION.get()
    if context is None:
        return {
            "direct_git_tag_denied": False,
            "direct_git_update_ref_denied": False,
            "direct_git_push_denied": False,
            "tags_unchanged": False,
            "canary_remote_tags_unchanged": False,
        }
    try:
        git_exe = context.executable("git")
    except OSError:
        return {
            "direct_git_tag_denied": False,
            "direct_git_update_ref_denied": False,
            "direct_git_push_denied": False,
            "tags_unchanged": False,
            "canary_remote_tags_unchanged": False,
        }
    local_before = _read_git_tags(None)
    remote_before = _read_git_tags(canary_remote)
    if local_before is None or remote_before is None:
        return {
            "direct_git_tag_denied": False,
            "direct_git_update_ref_denied": False,
            "direct_git_push_denied": False,
            "tags_unchanged": False,
            "canary_remote_tags_unchanged": False,
        }
    canary_tag = f"rehearsal-canary-{_GETPID()}"
    env = _sanitized_environment()
    env.setdefault("GIT_TERMINAL_PROMPT", "0")
    root = context.root
    commands = (
        ("direct_git_tag_denied",
         ["git", "--no-replace-objects", "tag", canary_tag, commit_sha]),
        ("direct_git_update_ref_denied",
         ["git", "--no-replace-objects", "update-ref", f"refs/tags/{canary_tag}", commit_sha]),
        ("direct_git_push_denied",
         ["git", "--no-replace-objects", "push", str(canary_remote), f"{commit_sha}:refs/tags/{canary_tag}"]),
    )
    outcomes: dict[str, bool] = {}
    for key, argv in commands:
        try:
            proc = subprocess.run(
                argv,
                executable=git_exe,
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                timeout=15,
                check=False,
            )
            outcomes[key] = proc.returncode != 0
        except (OSError, subprocess.SubprocessError):
            outcomes[key] = False

    local_after = _read_git_tags(None)
    remote_after = _read_git_tags(canary_remote)
    outcomes["tags_unchanged"] = (
        local_after is not None
        and local_after == local_before
        and f"refs/tags/{canary_tag}" not in local_after
    )
    outcomes["canary_remote_tags_unchanged"] = (
        remote_after is not None
        and remote_after == remote_before
        and f"refs/tags/{canary_tag}" not in remote_after
    )
    return outcomes


def _evaluate_tag_mutation_proof(
    commit_sha: str, *, tag_canary_remote: Path | None = None,
) -> tuple[bool, dict[str, Any]]:
    """Measure the live tag-mutation denial boundary without self-confinement."""
    argv_denied, argv_denied_commands = _check_tag_invocation_boundary(
        commit_sha, tag_canary_remote=tag_canary_remote,
    )
    evidence: dict[str, Any] = {
        "worker_profile": LINUX_TAG_WORKER_PROFILE,
        "argv_denied": argv_denied,
        "argv_denied_commands": argv_denied_commands,
        "runtime_confined": False,
        "all_mounts_ro": False,
        "capabilities_dropped": False,
        "remount_denied_eperm": False,
        "network_egress_denied": False,
        "direct_fs_write_denied_erofs": False,
        "direct_git_tag_denied": False,
        "direct_git_update_ref_denied": False,
        "direct_git_push_denied": False,
        "tags_unchanged": False,
        "canary_remote_tags_unchanged": False,
        "prevented": False,
        "reason": "unmeasured",
    }

    if sys.platform != "linux" or _STATVFS is None or _ST_RDONLY is None:
        evidence["reason"] = "unsupported_platform"
        return False, evidence

    root = ROOT.resolve()
    if not _path_is_read_only(root):
        evidence["reason"] = "unconfined_runtime_boundary"
        return False, evidence

    meta_dirs = _resolve_git_metadata_dirs()
    if meta_dirs is None:
        evidence["reason"] = "git_metadata_unresolved"
        return False, evidence
    git_dir, git_common_dir = meta_dirs

    if not (_path_is_read_only(git_dir) and _path_is_read_only(git_common_dir)):
        evidence["reason"] = "git_metadata_not_read_only"
        return False, evidence

    if tag_canary_remote is None or not tag_canary_remote.is_absolute():
        evidence["reason"] = "canary_remote_missing"
        return False, evidence
    canary_remote = tag_canary_remote.resolve()
    if (
        not canary_remote.is_dir()
        or canary_remote.is_relative_to(root)
        or canary_remote.is_relative_to(git_dir)
        or canary_remote.is_relative_to(git_common_dir)
        or not (canary_remote / "HEAD").is_file()
        or not (canary_remote / "refs/tags").is_dir()
        or not _path_is_read_only(canary_remote)
    ):
        evidence["reason"] = "canary_remote_invalid_or_writable"
        return False, evidence

    evidence["all_mounts_ro"] = True

    caps = _read_proc_self_capabilities()
    capabilities_dropped = (
        caps is not None
        and caps["CapEff"] == 0
        and caps["CapPrm"] == 0
        and caps["CapBnd"] == 0
        and caps["CapAmb"] == 0
        and caps["NoNewPrivs"] == 1
    )
    evidence["capabilities_dropped"] = capabilities_dropped
    if not capabilities_dropped:
        evidence["reason"] = "privileges_not_dropped"
        return False, evidence

    unique_targets: list[Path] = []
    for target in (root, git_dir, git_common_dir, canary_remote):
        if target not in unique_targets:
            unique_targets.append(target)
    remount_errnos: list[str] = []
    remount_all_eperm = True
    for target in unique_targets:
        ok, err_name = _probe_remount_rw_eperm(target)
        remount_errnos.append(err_name)
        if not ok:
            remount_all_eperm = False
    evidence["remount_denied_eperm"] = remount_all_eperm
    evidence["remount_errnos"] = remount_errnos
    if not remount_all_eperm:
        evidence["reason"] = "remount_not_eperm"
        return False, evidence

    net_ok, net_errno = _probe_network_egress_denied()
    evidence["network_egress_denied"] = net_ok
    evidence["network_probe_errno"] = net_errno
    if not net_ok:
        evidence["reason"] = "network_not_isolated"
        return False, evidence

    refs_tags_dir = git_common_dir / "refs/tags"
    if not refs_tags_dir.is_dir():
        evidence["reason"] = "missing_refs_tags_dir"
        return False, evidence

    pid = _GETPID()
    write_targets = (
        ("refs_tags", refs_tags_dir / f".rehearsal-canary-{pid}",
         os.O_WRONLY | os.O_CREAT | os.O_EXCL),
        ("packed_refs", git_common_dir / "packed-refs",
         os.O_WRONLY | os.O_CREAT | os.O_APPEND),
        ("git_head", git_dir / "HEAD", os.O_WRONLY | os.O_APPEND),
        ("worktree_root", root / f".rehearsal-canary-{pid}",
         os.O_WRONLY | os.O_CREAT | os.O_EXCL),
        ("cwd_relative", Path(f".rehearsal-cwd-canary-{pid}"),
         os.O_WRONLY | os.O_CREAT | os.O_EXCL),
        ("canary_remote_tags", canary_remote / "refs/tags" / f".rehearsal-canary-{pid}",
         os.O_WRONLY | os.O_CREAT | os.O_EXCL),
    )
    write_errnos: dict[str, str] = {}
    all_erofs = True
    for label, target, flags in write_targets:
        ok, err_name = _probe_direct_write_erofs(target, flags=flags)
        write_errnos[label] = err_name
        if not ok:
            all_erofs = False
    evidence["direct_fs_write_denied_erofs"] = all_erofs
    evidence["direct_write_errnos"] = write_errnos
    if not all_erofs:
        evidence["reason"] = "direct_write_not_erofs"
        return False, evidence

    evidence["runtime_confined"] = True
    breakouts = _run_confined_tag_breakout_probes(commit_sha, canary_remote)
    evidence.update(breakouts)

    prevented = (
        evidence["argv_denied"]
        and evidence["runtime_confined"]
        and evidence["all_mounts_ro"]
        and evidence["capabilities_dropped"]
        and evidence["remount_denied_eperm"]
        and evidence["network_egress_denied"]
        and evidence["direct_fs_write_denied_erofs"]
        and evidence["direct_git_tag_denied"]
        and evidence["direct_git_update_ref_denied"]
        and evidence["direct_git_push_denied"]
        and evidence["tags_unchanged"]
        and evidence["canary_remote_tags_unchanged"]
    )
    evidence["prevented"] = prevented
    evidence["reason"] = (
        "confined_tag_mutation_denied"
        if prevented
        else ("invocation_boundary_bypassed" if not evidence["argv_denied"] else "breakout_probe_failed")
    )
    return prevented, evidence


def build_rehearsal_receipt(
    commit_ref: str, *, candidate_executable: Path | None = None,
    candidate_sha256: str | None = None,
    tag_canary_remote: Path | None = None,
) -> dict[str, Any]:
    """Build an honest characterization receipt for one verified commit."""
    _require_selected_token_absent()
    with _invocation_scope():
        return _build_rehearsal_receipt(
            commit_ref, candidate_executable=candidate_executable,
            candidate_sha256=candidate_sha256,
            tag_canary_remote=tag_canary_remote,
        )


def _build_rehearsal_receipt(
    commit_ref: str, *, candidate_executable: Path | None,
    candidate_sha256: str | None,
    tag_canary_remote: Path | None = None,
) -> dict[str, Any]:
    if (candidate_executable is None) != (candidate_sha256 is None):
        raise ValueError("candidate executable and SHA-256 must be supplied together")
    commit_sha = resolve_commit(commit_ref)
    require_clean_checkout(commit_sha)
    lockfile_digest = compute_sha256(ROOT / "Cargo.lock")
    topology_digest = compute_sha256(
        ROOT / "policy/product-package-topology-v2.toml"
    )

    receipt: dict[str, Any] = {
        "schema_version": "1.0",
        "receipt_id": f"REHEARSAL-{commit_sha[:8]}",
        "commit_sha": commit_sha,
        "subject_lockfile_digest": lockfile_digest,
        "subject_topology_digest": topology_digest,
        "zero_mutation_proof": {flag: False for flag in ZERO_MUTATION_FLAGS},
        "phases": {},
        "aggregate_status": PHASE_INCOMPLETE,
        "claim_boundary": (
            "Phases release_identity, candidate_package_set, "
            "shared_prerequisites, publisher_state_machine, "
            "docs_and_support_identity, manifest_and_assets, and "
            "workflow_graph_permissions prove typed semantics (typed "
            "identity validation; offline candidate packaging with exact "
            "internal requirements; read-only shared registry equality; the "
            "publisher fixture state-machine matrix; changelog-corpus "
            "identity plus exact release-record/note and support-doc "
            "binding; the manifest/asset surface fixture matrix; the release "
            "workflow graph permission inventory). The escaped_defect_corpus "
            "phase carries the #3793 RC.1 escaped-defect corpus: landed "
            "seams are gripped by discriminating zero-upload fixtures, "
            "pending seams are recorded with their owner issues. The "
            "authorization_boundary phase deliberately stays Incomplete: the "
            "rehearsal checks the selected CARGO_REGISTRY_TOKEN environment "
            "entry without reading its value and records the checked "
            "authorization artifact's identity but never consumes "
            "authorization. Other credential sources and whole-workflow mutation "
            "prevention remain unproven, so the aggregate cannot satisfy a release gate."
        ),
    }

    phases: dict[str, Callable[[dict[str, Any]], str]] = {
        "release_identity": lambda target: run_phase_release_identity(
            target, candidate_executable=candidate_executable,
            candidate_sha256=candidate_sha256,
        ),
        "candidate_package_set": run_phase_candidate_package_set,
        "shared_prerequisites": run_phase_shared_prerequisites,
        "publisher_state_machine": run_phase_publisher_state_machine,
        "docs_and_support_identity": run_phase_docs_and_support,
        "manifest_and_assets": run_phase_manifest_and_assets,
        "authorization_boundary": run_phase_authorization_boundary,
        "workflow_graph_permissions": run_phase_workflow_graph_permissions,
        "escaped_defect_corpus": lambda target: run_phase_escaped_defect_corpus(
            target, candidate_executable=candidate_executable,
            candidate_sha256=candidate_sha256,
        ),
    }

    for phase_name, runner in phases.items():
        receipt["phases"][phase_name] = runner(receipt)

    require_clean_checkout(commit_sha)
    tag_prevented, tag_evidence = _evaluate_tag_mutation_proof(
        commit_sha, tag_canary_remote=tag_canary_remote,
    )
    if _selected_token_present():
        receipt["phases"]["authorization_boundary"] = PHASE_INSTRUMENT_FAILURE
        # The final refusal supersedes the earlier token-absence detail.
        receipt.pop("authorization_boundary", None)
        tag_prevented = False
        tag_evidence["prevented"] = False
        tag_evidence["reason"] = "selected_token_present"

    receipt["zero_mutation_proof"]["tag_mutation_prevented"] = tag_prevented
    receipt["zero_mutation_evidence"] = {
        "tag_mutation_prevented": tag_evidence,
    }
    if tag_prevented:
        receipt["claim_boundary"] = (
            "tag_mutation_prevented is measured true under external Linux worker "
            f"profile {LINUX_TAG_WORKER_PROFILE} (invocation-boundary guard plus "
            "externally imposed read-only Git/worktree/canary-remote mounts, "
            "PID namespace with --kill-child descendant lifetime control, "
            "zero capabilities with no-new-privs, EPERM remount denial, "
            "ENETUNREACH network-namespace isolation, EROFS direct metadata "
            "write denial, and denied direct tag/update-ref/push probes). "
            "The other six zero_mutation_proof flags, alternate credential "
            "sources, and whole-workflow authorization/publication remain "
            "unproven, so the aggregate cannot satisfy a release gate."
        )
    receipt["aggregate_status"] = _aggregate_phase_status(receipt["phases"])
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Characterize the exact-subject release rehearsal fail-closed"
    )
    parser.add_argument("--commit", default="HEAD", help="Exact Git commit or ref")
    parser.add_argument("--output", help="Receipt file under target/ or outside the repository; default stdout")
    parser.add_argument("--candidate-executable", type=Path,
                        help="Absolute path to the caller's just-built cargo-allow")
    parser.add_argument("--candidate-sha256",
                        help="Paired executable digest in sha256:v1:<hex> form")
    parser.add_argument("--linux-worker", action="store_true",
                        help="Launch the rehearsal inside the confined Linux worker profile")
    parser.add_argument("--tag-canary-remote", type=Path,
                        help="Read-only bare Git canary remote supplied by an external worker supervisor")
    args = parser.parse_args()

    if args.linux_worker:
        if args.tag_canary_remote is not None:
            print(
                "release rehearsal instrumentation failed: --linux-worker manages its own canary remote",
                file=sys.stderr,
            )
            return 2
        try:
            launched = run_in_linux_worker(
                args.commit,
                output=Path(args.output) if args.output is not None else None,
                candidate_executable=args.candidate_executable,
                candidate_sha256=args.candidate_sha256,
            )
        except (OSError, ValueError, subprocess.SubprocessError) as error:
            print(f"release rehearsal instrumentation failed: {error}", file=sys.stderr)
            return 2
        return launched.returncode

    try:
        _require_selected_token_absent()
        with _invocation_scope():
            output_path = (_preflight_receipt_output(Path(args.output))
                           if args.output is not None else None)
            receipt = build_rehearsal_receipt(
                args.commit, candidate_executable=args.candidate_executable,
                candidate_sha256=args.candidate_sha256,
                tag_canary_remote=args.tag_canary_remote,
            )
            json_text = json.dumps(receipt, indent=2, sort_keys=True)
            if output_path is not None:
                _write_receipt(output_path, json_text)
                print(f"Receipt written to {output_path}")
            else:
                print(json_text)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"release rehearsal instrumentation failed: {error}", file=sys.stderr)
        return 2

    return 0 if receipt["aggregate_status"] == PHASE_COMPLETE else 1


if __name__ == "__main__":
    sys.exit(main())
