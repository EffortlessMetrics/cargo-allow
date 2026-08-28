#!/usr/bin/env python3
"""Resolved-graph comparison and receipt assembly for #2925.

The bash orchestrator drives Cargo; this helper owns the pure decisions:

* compare an actual `cargo metadata` resolution against the typed candidate
  rows (exact names/versions, no path sources, no unselected families);
* assemble and redaction-scan the typed
  `cargo-allow.isolated-install.v2` receipt payload;
* classify the offline characterization negatives.
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any

SCHEMA_ID = "cargo-allow.isolated-install.v2"
SCHEMA_VERSION = 2
UNSELECTED_FAMILIES = ("intent-", "proof-")
PRIVATE_PATH_MARKERS = (
    "/home/",
    "/users/",
    "c:\\",
    "/runner/work/",
    "\\cargo-allow\\",
    "/cargo-allow/crates/",
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(65536), b""):
            digest.update(block)
    return f"sha256:{digest.hexdigest()}"


def compare_resolution(
    metadata: dict[str, Any], candidate_rows: list[dict[str, Any]]
) -> dict[str, Any]:
    """Compare `cargo metadata` resolve output with the candidate rows.

    `metadata` is the parsed `cargo metadata --format-version 1` JSON. Every
    candidate row must resolve at exactly its version; no path/workspace
    source may appear; no unselected intent/proof package may appear.
    """
    resolved: dict[str, set[str]] = {}
    path_sources: list[str] = []
    for package in metadata.get("packages", []):
        name = package.get("name", "")
        version = package.get("version", "")
        resolved.setdefault(name, set()).add(version)
        manifest_path = str(package.get("manifest_path", "")).replace("\\", "/")
        if "/crates/" in manifest_path:
            path_sources.append(name)
    unexpected: list[str] = []
    missing: list[str] = []
    mismatches: list[str] = []
    matched = 0
    for row in candidate_rows:
        name = row["cargo_package_name"]
        expected_version = row["cargo_package_version"]
        versions = resolved.get(name)
        if not versions:
            missing.append(f"{name} {expected_version}")
        elif expected_version in versions:
            matched += 1
            for extra in sorted(versions - {expected_version}):
                mismatches.append(
                    f"{name} expected {expected_version} also resolved {extra}"
                )
        else:
            mismatches.append(
                f"{name} expected {expected_version} resolved {sorted(versions)}"
            )
    for name in resolved:
        lowered = name.lower()
        if lowered.startswith(UNSELECTED_FAMILIES) and name not in {
            row["cargo_package_name"] for row in candidate_rows
        }:
            unexpected.append(name)

    return {
        "expected_packages": len(candidate_rows),
        "matched_packages": matched,
        "unexpected_packages": sorted(unexpected),
        "missing_packages": sorted(missing),
        "version_mismatches": sorted(mismatches),
        "path_sources": sorted(set(path_sources)),
    }


def sep() -> str:
    return "\\" if sys.platform == "win32" else "/"


def receipt_payload(
    candidate_artifact_path: Path,
    candidate_rows: list[dict[str, Any]],
    graph_comparison: dict[str, Any],
    *,
    commit: str,
    tree: str,
    cargo_lock_digest: str,
    registry_index_digest: str,
    external_cache_identity: str,
    source_checkout_denied: bool,
    install_root_identity: str,
    cargo_home_identity: str,
    installed_executable_digest: str,
    installed_version_output: str,
    platform: str,
    toolchain: str,
) -> dict[str, Any]:
    rows = []
    for row in candidate_rows:
        rows.append(
            {
                "package_name": row["cargo_package_name"],
                "package_version": row["cargo_package_version"],
                "crate_digest": row["crate_digest"],
                "index_checksum": row["index_checksum"],
                "resolved_version": row["cargo_package_version"],
            }
        )
    return {
        "schema_id": SCHEMA_ID,
        "schema_version": SCHEMA_VERSION,
        "candidate_artifact_digest": sha256_file(candidate_artifact_path),
        "repository_commit": commit,
        "repository_tree": tree,
        "cargo_lock_digest": cargo_lock_digest,
        "registry_index_digest": registry_index_digest,
        "external_cache_identity": external_cache_identity,
        "source_checkout_denied": source_checkout_denied,
        "install_root_identity": install_root_identity,
        "cargo_home_identity": cargo_home_identity,
        "installed_executable_digest": installed_executable_digest,
        "installed_version_output": installed_version_output,
        "platform": platform,
        "toolchain": toolchain,
        "package_rows": rows,
        "graph_comparison": graph_comparison,
        "limitations": [
            "linux hosted claim only; other platforms need their own rows",
            "no publication, tag, or live registry change occurs",
        ],
        "claim_boundary": (
            "Exact offline installation and resolved-graph evidence for the "
            "topology-selected cargo-allow candidate from an isolated local "
            "registry with workspace and ambient fallbacks denied."
        ),
    }


def redaction_scan(payload: dict[str, Any]) -> list[str]:
    """Negative 15: report any private absolute path retained in the receipt."""
    leaks: list[str] = []

    def walk(node: Any, path: str) -> None:
        if isinstance(node, dict):
            for key, value in node.items():
                walk(value, f"{path}.{key}")
        elif isinstance(node, list):
            for index, value in enumerate(node):
                walk(value, f"{path}[{index}]")
        elif isinstance(node, str):
            for marker in PRIVATE_PATH_MARKERS:
                if marker in node.lower():
                    leaks.append(f"{path} retains {marker!r}")

    walk(payload, "payload")
    return leaks


def classify(payload: dict[str, Any]) -> str:
    """Offline classification mirroring the Rust validator's top levels."""
    if redaction_scan(payload):
        return "PathLeakInReceipt"
    if not payload.get("source_checkout_denied", False):
        return "SourceFallbackDetected"
    graph = payload.get("graph_comparison") or {}
    matched = graph.get("matched_packages", 0)
    if (
        matched != graph.get("expected_packages", -1)
        or graph.get("unexpected_packages")
        or graph.get("missing_packages")
        or graph.get("version_mismatches")
        or graph.get("path_sources")
    ):
        return "GraphMismatch"
    for field in (
        "candidate_artifact_digest",
        "cargo_lock_digest",
        "registry_index_digest",
        "installed_executable_digest",
    ):
        value = payload.get(field, "")
        if not re.fullmatch(r"sha256:[0-9a-f]{64}", value or ""):
            return "StaleInput"
    return "Complete"


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--mode",
        choices=("compare", "receipt", "classify", "redaction-scan", "assemble"),
        required=True,
    )
    parser.add_argument("--params", type=Path)
    parser.add_argument("--metadata", type=Path)
    parser.add_argument("--candidate-artifact", type=Path)
    parser.add_argument("--receipt-out", type=Path)
    parser.add_argument("--input-receipt", type=Path)
    args = parser.parse_args()

    modes_requiring_artifact = {"compare", "receipt", "assemble"}
    if args.mode in modes_requiring_artifact and args.candidate_artifact is None:
        print(f"--candidate-artifact is required for --mode {args.mode}", file=sys.stderr)
        return 2
    artifact = (
        json.loads(args.candidate_artifact.read_text(encoding="utf-8"))
        if args.candidate_artifact is not None
        else None
    )
    if args.mode == "compare":
        metadata = json.loads(args.metadata.read_text(encoding="utf-8"))
        comparison = compare_resolution(metadata, artifact["rows"])
        print(json.dumps(comparison, indent=2, sort_keys=True))
        return 0 if not any(
            (
                comparison["unexpected_packages"],
                comparison["missing_packages"],
                comparison["version_mismatches"],
                comparison["path_sources"],
            )
        ) else 1
    if args.mode == "classify":
        receipt = json.loads(args.input_receipt.read_text(encoding="utf-8"))
        print(classify(receipt))
        return 0
    if args.mode == "redaction-scan":
        receipt = json.loads(args.input_receipt.read_text(encoding="utf-8"))
        leaks = redaction_scan(receipt)
        for leak in leaks:
            print(leak, file=sys.stderr)
        return 1 if leaks else 0
    if args.mode == "assemble":
        params = json.loads(args.params.read_text(encoding="utf-8"))
        payload = receipt_payload(
            args.candidate_artifact,
            params["candidate_rows"],
            params["graph_comparison"],
            commit=params["repository_commit"],
            tree=params["repository_tree"],
            cargo_lock_digest=params["cargo_lock_digest"],
            registry_index_digest=params["registry_index_digest"],
            external_cache_identity=params["external_cache_identity"],
            source_checkout_denied=params["source_checkout_denied"],
            install_root_identity=params["install_root_identity"],
            cargo_home_identity=params["cargo_home_identity"],
            installed_executable_digest=params["installed_executable_digest"],
            installed_version_output=params["installed_version_output"],
            platform=params["platform"],
            toolchain=params["toolchain"],
        )
        classification = classify(payload)
        if classification != "Complete":
            print(f"assembled receipt classified {classification}", file=sys.stderr)
            return 1
        if args.receipt_out is None:
            print("assemble requires --receipt-out", file=sys.stderr)
            return 1
        args.receipt_out.write_text(
            json.dumps(payload, indent=2) + "\n", encoding="utf-8", newline="\n"
        )
        print(f"isolated-install receipt: {args.receipt_out}")
        return 0
    # receipt mode: validates an assembled receipt against its artifact.
    receipt = json.loads(args.input_receipt.read_text(encoding="utf-8"))
    expected_digest = sha256_file(args.candidate_artifact)
    if receipt.get("candidate_artifact_digest") != expected_digest:
        print("receipt does not bind the consumed candidate artifact", file=sys.stderr)
        return 1
    print("receipt binds the consumed candidate artifact")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
