#!/usr/bin/env python3
"""Fail-closed characterization of the exact-subject release rehearsal."""

from __future__ import annotations

import argparse
import contextlib
from contextvars import ContextVar
import hashlib
import json
import os
from os import name as _PLATFORM_NAME, pathsep as _PATH_SEPARATOR
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
from typing import Any, Callable

ROOT = Path(__file__).resolve().parent.parent
PHASE_COMPLETE = "Complete"
PHASE_INCOMPLETE = "Incomplete"
PHASE_MISMATCH = "Mismatch"
PHASE_INSTRUMENT_FAILURE = "InstrumentFailure"
CARGO_TOKEN_ENV = "CARGO_REGISTRY_TOKEN"


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
        "publisher-proof": ([context.python, str(context.root / "scripts/test-release-topology-publisher.py")], "python"),
        "history-proof": ([context.python, str(context.root / "scripts/generate-changie-history.py"), "--check"], "python"),
        "manifest-proof": ([context.python, str(context.root / "scripts/test-final-packaged-surface.py")], "python"),
        "corpus-proof": ([context.python, str(context.root / "scripts/test-release-escaped-defect-corpus.py")], "python"),
    }
    if operation in fixed and selected is None:
        return fixed[operation]
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
        "resolve-commit", "release-identity", "candidate-package", "shared-preflight",
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


def _filtered_environment(environment: Any) -> dict[str, str]:
    """Exclude the selected token by name before retrieving any value."""
    return {name: environment[name] for name in environment if name != CARGO_TOKEN_ENV}


def _sanitized_environment() -> dict[str, str]:
    """Exclude the selected token before retrieving any environment values.

    This is not isolation from alternate credential sources or proof of the
    rehearsal's seven broader zero-mutation obligations.
    """
    return _filtered_environment(os.environ)


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

ZERO_MUTATION_FLAGS = (
    "tag_mutation_prevented",
    "token_read_prevented",
    "cargo_publish_prevented",
    "registry_mutation_prevented",
    "github_release_mutation_prevented",
    "live_setting_mutation_prevented",
    "external_repository_mutation_prevented",
)


def _quarantine_root() -> Path:
    """Create one disposable root for the reversible mutation drills."""
    return Path(tempfile.mkdtemp(prefix="rehearsal-quarantine-"))


def _argv_denied(operation: str, command: list[str], root: Path) -> bool:
    """Attempt one mutation through the selected invocation boundary."""
    try:
        _run_process(
            command, operation=operation, cwd=root,
            env=_sanitized_environment(), capture_output=True,
            text=True, timeout=15, check=False,
        )
    except OSError:
        return True
    return False


class _RecordingEndpoint:
    """Local TCP listener that records whether a direct socket reached it."""

    def __init__(self) -> None:
        import socket
        import threading

        self._socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self._socket.bind(("127.0.0.1", 0))
        self._socket.listen(1)
        self._socket.settimeout(5)
        self.port = self._socket.getsockname()[1]
        self.received: list[bytes] = []
        self._thread = threading.Thread(target=self._accept, daemon=True)
        self._thread.start()

    def _accept(self) -> None:
        try:
            connection, _ = self._socket.accept()
            with connection:
                connection.settimeout(5)
                self.received.append(connection.recv(64))
        except OSError:
            pass

    def close(self) -> None:
        self._socket.close()
        self._thread.join(timeout=5)


def _direct_socket_denied(port: int) -> tuple[bool, bool]:
    """Attempt one direct socket write.

    Returns ``(denied, measured)``. A connection refusal is a measured denial.
    A timeout or any other error is unmeasured: a failed probe cannot be told
    apart from a confinement refusal, so it must not be recorded as one.
    """
    import socket

    try:
        with socket.create_connection(("127.0.0.1", port), timeout=2) as connection:
            connection.sendall(b"mutation")
        return False, True
    except ConnectionRefusedError:
        return True, True
    except OSError:
        return False, False


def _drill_result(
    *, argv_denied: bool, direct_denied: bool, boundary: str, reason: str,
    direct_route_discriminating: bool, attempted: bool = True,
) -> dict[str, Any]:
    # A drill whose direct route runs inside the quarantine the drill itself
    # created cannot be denied by a profile that permits that root, so it can
    # never observe prevention; its false flag is an absence of measurement,
    # not evidence of confinement. Every caller states this explicitly.
    prevented = (
        attempted and argv_denied and direct_denied and direct_route_discriminating
    )
    return {
        "attempted": attempted,
        "argv_denied": argv_denied,
        "direct_denied": direct_denied,
        "attempt_succeeded": attempted and not direct_denied,
        "direct_route_discriminating": direct_route_discriminating,
        "canary_unchanged": direct_denied,
        "prevented": prevented,
        "boundary": boundary,
        "reason": (
            "denied at the selected invocation boundary and on the direct route"
            if prevented else reason
        ),
    }


TAG_WORKER_SCRIPT = '''\
import json
import subprocess
import sys
from pathlib import Path

# The supervisor already applied the read-only mount and dropped capabilities
# before this exec. The worker first tries to remount read-write: if that
# succeeds, the drill is not discriminating and must not claim prevention.
repo = Path(sys.argv[1])
remount = subprocess.run(
    ["mount", "-o", "remount,bind,rw", str(repo)],
    capture_output=True, text=True,
)
remount_denied = remount.returncode != 0
target = repo / ".git" / "refs" / "tags" / "canary"
try:
    target.write_text("0" * 40, encoding="utf-8")
    direct_denied = False
except OSError:
    direct_denied = True
print(json.dumps({
    "direct_denied": direct_denied,
    "canary_unchanged": not target.exists(),
    "remount_denied": remount_denied,
}))
'''

TAG_SUPERVISOR_SCRIPT = '''\
set -euo pipefail
repo="$1"
worker="$2"
mount --bind "$repo" "$repo"
mount -o remount,bind,ro "$repo"
# Drop every capability, including CAP_SYS_ADMIN, before the worker execs.
# The worker therefore cannot remount the repository read-write.
exec setpriv --bounding-set=-all --inh-caps=-all --ambient-caps=-all \\
    --no-new-privs python3 "$worker" "$repo"
'''


def _linux_tag_launchers() -> list[tuple[list[str], str]]:
    """Select Linux isolation launchers, strongest first."""
    if not sys.platform.startswith("linux"):
        return []
    unshare = shutil.which("unshare")
    setpriv = shutil.which("setpriv")
    if unshare is None or shutil.which("mount") is None or setpriv is None:
        return []
    # Only the unprivileged user+mount namespace is used. A passwordless-sudo
    # fallback would run the supervisor as real root, which is a stronger
    # privilege than the drill is meant to model; when userns is blocked the
    # drill stays unmeasured and the flag stays false with a named reason.
    return [
        ([unshare, "-Urm", "--", "bash"], "unprivileged user+mount namespace"),
    ]


def _linux_tag_worker() -> str | None:
    """Select the Linux isolation launcher, or None when unavailable."""
    launchers = _linux_tag_launchers()
    return launchers[0][0][0] if launchers else None


def _drill_tag(root: Path) -> dict[str, Any]:
    repo = root / "tag-canary"
    ref = repo / ".git" / "refs" / "tags" / "canary"
    ref.parent.mkdir(parents=True)
    argv = _argv_denied("tag-mutation", ["git", "tag", "canary"], root)
    launchers = _linux_tag_launchers()
    boundary = "supervised read-only mount with dropped capabilities"
    if not launchers:
        return _drill_result(
            argv_denied=argv, direct_denied=False,
            boundary=boundary,
            reason=(
                "no Linux supervisor profile (unshare/mount/setpriv) is "
                "available on this host, so the direct Git metadata route is "
                "unmeasured"
            ),
            direct_route_discriminating=False,
        )
    worker = root / "tag-worker.py"
    worker.write_text(TAG_WORKER_SCRIPT, encoding="utf-8")
    supervisor = root / "tag-supervisor.sh"
    supervisor.write_text(TAG_SUPERVISOR_SCRIPT, encoding="utf-8")
    try:
        root.chmod(0o755)
        worker.chmod(0o644)
        supervisor.chmod(0o755)
    except OSError:
        pass
    detail = "no supervisor stderr"
    for launcher, description in launchers:
        try:
            completed = subprocess.run(
                [*launcher, str(supervisor), str(repo), str(worker)],
                cwd=root, env=_sanitized_environment(),
                capture_output=True, text=True, timeout=120, check=False,
            )
        except (OSError, subprocess.SubprocessError):
            completed = None
        if completed is None or completed.returncode != 0:
            if completed is not None and completed.stderr.strip():
                detail = completed.stderr.strip().splitlines()[-1][:200]
            continue
        try:
            observed = json.loads(completed.stdout)
        except json.JSONDecodeError:
            detail = "the worker returned no usable observation"
            continue
        direct = bool(observed.get("direct_denied"))
        unchanged = bool(observed.get("canary_unchanged"))
        remount_denied = bool(observed.get("remount_denied"))
        result = _drill_result(
            argv_denied=argv, direct_denied=direct and unchanged,
            boundary=f"{boundary} ({description})",
            reason=(
                "the supervised worker could remount the repository "
                "read-write, so the denial is not discriminating"
                if not remount_denied else
                "the supervised worker did not observe the direct Git "
                "metadata write denied"
            ),
            # The supervisor applies the read-only mount and the capability
            # drop inside the drill's own unshare -Urm namespace, so a denial
            # here proves the drill supervisor, not the rehearsal's runtime
            # boundary. The tag family therefore never claims prevention.
            direct_route_discriminating=False,
        )
        result["canary_unchanged"] = unchanged
        result["remount_denied"] = remount_denied
        return result
    return _drill_result(
        argv_denied=argv, direct_denied=False,
        boundary=boundary,
        reason=(
            "the Linux supervisor did not complete, so the direct Git "
            f"metadata denial is unproven: {detail}"
        ),
        direct_route_discriminating=False,
    )


def _drill_token(root: Path) -> dict[str, Any]:
    token_file = root / "token-canary"
    token_file.write_text("canary-token", encoding="utf-8")
    child_env = _filtered_environment(
        {CARGO_TOKEN_ENV: "canary-token", "PATH": "canary"}
    )
    boundary = _argv_denied(
        "token-read", [sys.executable, "-c", "print(1)"], root,
    )
    filtered = CARGO_TOKEN_ENV not in child_env
    try:
        with token_file.open("rb") as handle:
            handle.read(1)
        direct = False
    except OSError:
        direct = True
    result = _drill_result(
        argv_denied=boundary and filtered, direct_denied=direct,
        boundary="selected token environment entry and child environment",
        reason=(
            "a direct read retrieved the token file the drill itself created "
            "in its quarantine, so this route cannot be denied by a profile "
            "that permits that root"
        ),
        direct_route_discriminating=False,
    )
    result["environment_filtered"] = filtered
    return result


def _drill_network(
    root: Path, *, operation: str, command: list[str], boundary: str, reason: str,
) -> dict[str, Any]:
    endpoint = _RecordingEndpoint()
    try:
        argv = _argv_denied(operation, command, root)
        direct, measured = _direct_socket_denied(endpoint.port)
    finally:
        endpoint.close()
    canary_unchanged = not endpoint.received
    result = _drill_result(
        argv_denied=argv, direct_denied=direct and canary_unchanged,
        boundary=boundary,
        reason=(
            reason if measured else
            "the direct socket probe failed without a refusal, so the route "
            "is unmeasured"
        ),
        direct_route_discriminating=True,
        attempted=measured,
    )
    result["canary_unchanged"] = canary_unchanged
    return result


def _drill_external(root: Path) -> dict[str, Any]:
    outside = Path(tempfile.mkdtemp(prefix="rehearsal-outside-"))
    try:
        argv = _argv_denied(
            "external-repository-mutation", ["git", "push", "origin", "HEAD"], root,
        )
        canary = outside / "canary"
        try:
            canary.write_text("mutated", encoding="utf-8")
        except OSError as exc:
            # An exception is not a denial. A confinement refusal and an
            # unrelated filesystem error look identical from here, so the
            # route is recorded as unmeasured instead of claiming prevention.
            return _drill_result(
                argv_denied=argv, direct_denied=False,
                boundary="remote push and writes outside owned roots",
                reason=(
                    f"the direct write raised {type(exc).__name__}, so the "
                    "route is unmeasured"
                ),
                direct_route_discriminating=True,
                attempted=False,
            )
        direct = not canary.exists()
    finally:
        shutil.rmtree(outside, ignore_errors=True)
    return _drill_result(
        argv_denied=argv, direct_denied=direct,
        boundary="remote push and writes outside owned roots",
        reason="a direct write created a file outside the owned root",
        direct_route_discriminating=True,
    )


def run_zero_mutation_drills() -> dict[str, dict[str, Any]]:
    """Exercise each mutation family in a disposable quarantine.

    Each drill attempts the forbidden effect through the rehearsal's selected
    invocation boundary and again on a direct route from the rehearsal process,
    then observes a disposable canary. A family is prevented only when both
    routes are denied and the canary is unchanged. The drills create no
    sandbox, so every flag is false today.

    Only the direct routes that target something outside the quarantine can
    later turn true under a confinement profile: the local socket endpoints and
    the write to a temporary directory outside the owned root. The tag and
    token direct routes run inside the drill's own writable quarantine, so no
    profile that permits that root can deny them; they record
    ``direct_route_discriminating`` false and can never report prevention. A
    false flag therefore means either "measured and not confined" or, for those
    two families, "not measurable from this drill"; the receipt keeps the two
    apart instead of presenting both as proof of confinement.
    """
    root = _quarantine_root()
    try:
        return {
            "tag_mutation_prevented": _drill_tag(root),
            "token_read_prevented": _drill_token(root),
            "cargo_publish_prevented": _drill_network(
                root, operation="cargo-publish",
                command=["cargo", "publish", "--dry-run"],
                boundary="Cargo publication invocation and direct registry socket",
                reason="a direct socket to a local recording endpoint succeeded",
            ),
            "registry_mutation_prevented": _drill_network(
                root, operation="registry-mutation",
                command=["cargo", "publish"],
                boundary="registry writes through Cargo and raw clients",
                reason="a direct socket to a local recording endpoint succeeded",
            ),
            "github_release_mutation_prevented": _drill_network(
                root, operation="github-release-mutation",
                command=["gh", "release", "create", "canary"],
                boundary="GitHub Release/asset writes through CLI and raw API",
                reason="a direct socket to a local recording endpoint succeeded",
            ),
            "live_setting_mutation_prevented": _drill_network(
                root, operation="live-setting-mutation",
                command=["gh", "api", "--method", "PATCH", "repos/canary"],
                boundary="settings/rules/environment/repository-control writes",
                reason="a direct socket to a local recording endpoint succeeded",
            ),
            "external_repository_mutation_prevented": _drill_external(root),
        }
    finally:
        shutil.rmtree(root, ignore_errors=True)


def _zero_mutation_proof_from_drills(
    drills: dict[str, dict[str, Any]],
) -> dict[str, bool]:
    """Derive the receipt flags from executed drills, never from literals."""
    return {
        name: bool(drills.get(name, {}).get("prevented"))
        for name in ZERO_MUTATION_FLAGS
    }


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


def _preflight_receipt_output(path: Path) -> Path:
    """Admit an artifact destination without changing source or output bytes."""
    path = path.absolute()
    for component in (path, *path.parents):
        try:
            metadata = component.lstat()
        except FileNotFoundError:
            continue
        if (stat.S_ISLNK(metadata.st_mode)
                or getattr(metadata, "st_file_attributes", 0)
                & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0)):
            raise OSError("receipt output cannot traverse a filesystem alias")
        if component == path:
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
                raise OSError("receipt output must be a singly linked regular file")
        elif not stat.S_ISDIR(metadata.st_mode):
            raise OSError("receipt output parent must be a directory")
    path = path.resolve()
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


def build_rehearsal_receipt(
    commit_ref: str, *, candidate_executable: Path | None = None,
    candidate_sha256: str | None = None,
) -> dict[str, Any]:
    """Build an honest characterization receipt for one verified commit."""
    _require_selected_token_absent()
    with _invocation_scope():
        return _build_rehearsal_receipt(
            commit_ref, candidate_executable=candidate_executable,
            candidate_sha256=candidate_sha256,
        )


def _build_rehearsal_receipt(
    commit_ref: str, *, candidate_executable: Path | None,
    candidate_sha256: str | None,
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
        "zero_mutation_proof": {},
        "zero_mutation_drills": {},
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
            "prevention remain unproven, so the aggregate cannot satisfy a release gate. "
            "The zero_mutation_proof flags are derived from executed quarantine "
            "drills: each family is attempted through the selected invocation "
            "boundary and again on a direct route against a disposable canary. A "
            "direct attempt that really mutates the canary keeps its flag false, so "
            "the receipt records the remaining confinement gap rather than asserting "
            "prevention. Only tag_mutation_prevented is measured in this generation: "
            "on Linux a supervisor applies the read-only bind mount, drops all "
            "capabilities, and only then execs the worker, so the worker cannot "
            "remount; the flag is true only when the worker's own remount attempt "
            "and the direct Git metadata write are both denied. On hosts without "
            "unshare/mount/setpriv the flag stays "
            "false. The other six flags remain "
            "false: the token, Cargo publication, registry, GitHub Release, live "
            "setting and external-repository direct routes are not confined."
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

    drills = run_zero_mutation_drills()
    receipt["zero_mutation_drills"] = drills
    receipt["zero_mutation_proof"] = _zero_mutation_proof_from_drills(drills)

    require_clean_checkout(commit_sha)
    if _selected_token_present():
        receipt["phases"]["authorization_boundary"] = PHASE_INSTRUMENT_FAILURE
        # The final refusal supersedes the earlier token-absence detail.
        receipt.pop("authorization_boundary", None)
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
    args = parser.parse_args()

    try:
        _require_selected_token_absent()
        with _invocation_scope():
            output_path = (_preflight_receipt_output(Path(args.output))
                           if args.output is not None else None)
            receipt = build_rehearsal_receipt(
                args.commit, candidate_executable=args.candidate_executable,
                candidate_sha256=args.candidate_sha256,
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
