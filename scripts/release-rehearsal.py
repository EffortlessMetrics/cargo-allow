#!/usr/bin/env python3
"""Fail-closed characterization of the exact-subject release rehearsal."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from typing import Any, Callable

ROOT = Path(__file__).resolve().parent.parent
PHASE_COMPLETE = "Complete"
PHASE_INCOMPLETE = "Incomplete"
PHASE_MISMATCH = "Mismatch"
PHASE_INSTRUMENT_FAILURE = "InstrumentFailure"
CARGO_TOKEN_ENV = "CARGO_REGISTRY_TOKEN"


def compute_sha256(path: Path) -> str:
    """Return the repository's canonical SHA-256 text for one exact file."""
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(65536):
            digest.update(chunk)
    return f"sha256:v1:{digest.hexdigest()}"


def resolve_commit(commit_ref: str) -> str:
    """Resolve one caller-supplied Git commit ref or fail without substitution."""
    if (
        not commit_ref
        or commit_ref.startswith("-")
        or any(char in commit_ref for char in "\r\n\0")
    ):
        raise ValueError("commit ref must be non-empty, single-line, and not start with a dash")
    result = subprocess.run(
        ["git", "rev-parse", "--verify", f"{commit_ref}^{{commit}}"],
        cwd=ROOT,
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


def _file_characterization(path: Path) -> str:
    """Keep file presence explicit without upgrading it to semantic proof."""
    try:
        return PHASE_INCOMPLETE if path.is_file() else PHASE_MISMATCH
    except OSError:
        return PHASE_INSTRUMENT_FAILURE


def _sanitized_environment() -> dict[str, str]:
    """Prevent child characterizations from receiving the registry secret."""
    environment = dict(os.environ)
    environment.pop(CARGO_TOKEN_ENV, None)
    return environment


def _run_characterization(command: list[str]) -> str:
    """Run a bounded characterization without treating exit zero as Complete."""
    try:
        result = subprocess.run(
            command,
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


def run_phase_release_identity(receipt: dict[str, Any]) -> str:
    """Consume the typed release-identity projection for the workspace candidate.

    The workspace version is read as the identity source and validated through
    ``cargo-allow release-identity``; this phase records the validated fields
    without re-deriving grammar, tag, or channel.
    """
    try:
        version = _workspace_version()
        result = subprocess.run(
            [
                "cargo", "run", "--quiet", "-p", "cargo-allow", "--locked", "--",
                "release-identity", "--version", version,
            ],
            cwd=ROOT,
            env=_sanitized_environment(),
            capture_output=True,
            text=True,
            timeout=300,
            check=False,
        )
    except (OSError, ValueError, subprocess.SubprocessError):
        return PHASE_INSTRUMENT_FAILURE
    if result.returncode != 0:
        return PHASE_MISMATCH
    try:
        projection = json.loads(result.stdout)
    except json.JSONDecodeError:
        return PHASE_INSTRUMENT_FAILURE
    if (
        projection.get("schema") != "cargo-allow.release-identity.v1"
        or projection.get("result") != "validated"
    ):
        return PHASE_MISMATCH
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


def run_phase_candidate_package_set(_receipt: dict[str, Any]) -> str:
    return _file_characterization(ROOT / "policy/product-package-topology-v2.toml")


def run_phase_shared_prerequisites(_receipt: dict[str, Any]) -> str:
    return _file_characterization(ROOT / "policy/product-package-topology-v2.toml")


def run_phase_publisher_state_machine(_receipt: dict[str, Any]) -> str:
    return _run_characterization(
        [sys.executable, str(ROOT / "scripts/test-release-topology-publisher.py")]
    )


def run_phase_docs_and_support(_receipt: dict[str, Any]) -> str:
    return _file_characterization(ROOT / "SUPPORT.md")


def run_phase_manifest_and_assets(_receipt: dict[str, Any]) -> str:
    return _run_characterization(
        [sys.executable, str(ROOT / "scripts/test-final-packaged-surface.py")]
    )


def run_phase_authorization_boundary(_receipt: dict[str, Any]) -> str:
    try:
        if os.environ.get(CARGO_TOKEN_ENV):
            return PHASE_INSTRUMENT_FAILURE
        return PHASE_INCOMPLETE
    except OSError:
        return PHASE_INSTRUMENT_FAILURE


def run_phase_workflow_graph_permissions(_receipt: dict[str, Any]) -> str:
    return _file_characterization(ROOT / ".github/workflows/release.yml")


CHARACTERIZATION_PHASES = frozenset({
    "candidate_package_set",
    "shared_prerequisites",
    "publisher_state_machine",
    "docs_and_support_identity",
    "manifest_and_assets",
    "authorization_boundary",
    "workflow_graph_permissions",
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


def _write_receipt(path: Path, json_text: str) -> None:
    """Write a receipt without following a symlink at the output leaf."""
    if path.is_symlink() or path.is_dir():
        raise OSError("output path cannot be a symlink or directory")
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.is_symlink() or path.is_dir():
        raise OSError("output path cannot be a symlink or directory")

    flags = os.O_WRONLY | os.O_CREAT | os.O_TRUNC
    flags |= getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags, 0o600)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as output:
            descriptor = -1
            output.write(json_text + "\n")
    finally:
        if descriptor != -1:
            os.close(descriptor)


def build_rehearsal_receipt(commit_ref: str) -> dict[str, Any]:
    """Build an honest characterization receipt for one verified commit."""
    commit_sha = resolve_commit(commit_ref)
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
        "zero_mutation_proof": {
            "tag_mutation_prevented": False,
            "token_read_prevented": False,
            "cargo_publish_prevented": False,
            "registry_mutation_prevented": False,
            "github_release_mutation_prevented": False,
            "live_setting_mutation_prevented": False,
            "external_repository_mutation_prevented": False,
        },
        "phases": {},
        "aggregate_status": PHASE_INCOMPLETE,
        "claim_boundary": (
            "Characterization only: current phases do not yet prove exact-subject "
            "semantics or zero mutation and cannot satisfy a release gate."
        ),
    }

    phases: dict[str, Callable[[dict[str, Any]], str]] = {
        "release_identity": run_phase_release_identity,
        "candidate_package_set": run_phase_candidate_package_set,
        "shared_prerequisites": run_phase_shared_prerequisites,
        "publisher_state_machine": run_phase_publisher_state_machine,
        "docs_and_support_identity": run_phase_docs_and_support,
        "manifest_and_assets": run_phase_manifest_and_assets,
        "authorization_boundary": run_phase_authorization_boundary,
        "workflow_graph_permissions": run_phase_workflow_graph_permissions,
    }

    for phase_name, runner in phases.items():
        receipt["phases"][phase_name] = runner(receipt)

    receipt["aggregate_status"] = _aggregate_phase_status(receipt["phases"])
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Characterize the exact-subject release rehearsal fail-closed"
    )
    parser.add_argument("--commit", default="HEAD", help="Exact Git commit or ref")
    parser.add_argument("--output", help="Path to write receipt JSON")
    args = parser.parse_args()

    try:
        receipt = build_rehearsal_receipt(args.commit)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"release rehearsal instrumentation failed: {error}", file=sys.stderr)
        return 2

    json_text = json.dumps(receipt, indent=2, sort_keys=True)
    if args.output:
        output_path = Path(args.output)
        _write_receipt(output_path, json_text)
        print(f"Receipt written to {output_path}")
    else:
        print(json_text)

    return 0 if receipt["aggregate_status"] == PHASE_COMPLETE else 1


if __name__ == "__main__":
    sys.exit(main())

