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
        result = subprocess.run(
            [
                sys.executable,
                (ROOT / "scripts/release-topology-publisher.py").as_posix(),
                "--mode", "cargo-allow",
                "--package-only",
                "--receipt", str(preflight_path),
            ],
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
        result = subprocess.run(
            [
                sys.executable,
                (ROOT / "scripts/release-topology-publisher.py").as_posix(),
                "--mode", "cargo-allow",
                "--registry-preflight",
                "--receipt", str(preflight_path),
            ],
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


def _run_proof(command: list[str]) -> str:
    """Run a bounded fixture proof: exit zero proves, nonzero mismatches."""
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
        [sys.executable, str(ROOT / "scripts/test-release-topology-publisher.py")]
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
        [sys.executable, str(ROOT / "scripts/generate-changie-history.py"), "--check"]
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
        [sys.executable, str(ROOT / "scripts/test-final-packaged-surface.py")]
    )


AUTHORIZATION_ARTIFACT = "release/authorize-v0.2.0.json"
AUTHORIZATION_SCHEMA = "cargo-allow.release-authorization.v1"


def run_phase_authorization_boundary(receipt: dict[str, Any]) -> str:
    """Prove the token-free rehearsal posture and bind the checked
    authorization artifact's identity (#3751 phase: authorization boundary).

    The rehearsal never consumes authorization: a publish token in the
    environment is an instrument failure, and the checked
    CargoAllowReleaseAuthorizationV1 artifact is read only to record which
    release identity would gate the authorized run. The phase deliberately
    stays Incomplete — only the authorized run (#3760/#2502) can claim
    authorization.
    """
    if os.environ.get(CARGO_TOKEN_ENV):
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
            "Phases release_identity, candidate_package_set, "
            "shared_prerequisites, publisher_state_machine, "
            "docs_and_support_identity, and manifest_and_assets prove typed "
            "semantics (typed identity validation; offline candidate packaging "
            "with exact internal requirements; read-only shared registry "
            "equality; the publisher fixture state-machine matrix; "
            "changelog-corpus identity plus exact release-record/note and "
            "support-doc binding; the manifest/asset surface fixture matrix; "
            "the release workflow graph permission inventory). The "
            "authorization_boundary phase deliberately stays Incomplete: the "
            "rehearsal proves the token-free posture and records the checked "
            "authorization artifact's identity but never consumes "
            "authorization, so the aggregate cannot satisfy a release gate."
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

