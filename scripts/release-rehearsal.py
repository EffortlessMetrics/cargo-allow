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
    if not commit_ref or any(char in commit_ref for char in "\r\n\0"):
        raise ValueError("commit ref must be non-empty and single-line")
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


def run_phase_release_identity(_receipt: dict[str, Any]) -> str:
    return _file_characterization(ROOT / "docs/support-matrix.toml")


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


def _aggregate_phase_status(phases: dict[str, str]) -> str:
    """Return a fail-closed aggregate until real typed phase adapters exist."""
    values = set(phases.values())
    if PHASE_INSTRUMENT_FAILURE in values:
        return PHASE_INSTRUMENT_FAILURE
    if PHASE_MISMATCH in values:
        return PHASE_MISMATCH
    return PHASE_INCOMPLETE


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
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(json_text + "\n", encoding="utf-8")
        print(f"Receipt written to {output_path}")
    else:
        print(json_text)

    return 0 if receipt["aggregate_status"] == PHASE_COMPLETE else 1


if __name__ == "__main__":
    sys.exit(main())
