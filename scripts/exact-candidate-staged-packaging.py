#!/usr/bin/env python3
"""Staged outside-workspace packaging of the topology-selected exact candidate.

#2886: the ambient workspace packaging paths (`cargo package --workspace` in
the harness worktree and the publisher's `--exclude` form) make candidate
bytes depend on workspace membership and `[workspace.package]` inheritance.
This producer assembles the V2-selected candidate closure in an isolated
staging tree outside the workspace, rewrites every manifest to standalone
form (workspace inheritance resolved, internal path dependencies expressed
as exact topology version requirements, workspace sections replaced), and
packages one crate per topology row in release order against a staged local
registry seeded in the same order. Mixed versions are inherent: rows carry
their own topology versions, never one workspace version.

Packaging runs fully offline: crates.io is source-replaced by the staged
local registry, so an unpublished internal requirement can only resolve from
a previously packaged row. The per-row loop therefore also fails closed on
release-order violations. Nothing is uploaded, no tag is created, and no
 crates.io lookup is reachable from this producer.

Modes (state persists across invocations through --output-root and the
receipt):
  stage   assemble the staged closure tree and its deterministically
          rewritten manifests (atomic: staged into a temporary directory,
          swapped into place only when complete)
  package seed the staged registry with warmed external crates, package one
          row per topology release_order entry, finalize the staged lockfile,
          then re-package every row with --locked and require byte-identical
          digests
  verify  re-check every packaged `.crate` against the staged tree and the
          topology rows: exact name-version identity, recorded digests,
          unpack-and-diff against the staged tree, packaged-manifest identity
          and mixed-version dependency law, complete package set, registry
          index agreement, and staged lockfile membership
  all     stage, package, and verify in one process

The staged bytes are vcs-info-free (the staging tree is intentionally not
git-backed) and are therefore not the publish-path bytes; substituting them
into the publication path is a separate reviewed child. This producer is not
publication authority and claims no installation or runtime behavior.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - CI runs 3.11+
    import tomli as tomllib  # type: ignore[no-redef]

ROOT = Path(__file__).resolve().parent.parent
CANDIDATE_PRODUCER = ROOT / "scripts/exact-candidate-package-candidate.py"

SCHEMA_ID = "cargo-allow.staged-packaging.v1"
SCHEMA_VERSION = 1

CLAIM_BOUNDARY = (
    "Exact topology-selected mixed-version candidate packaging from an "
    "outside-workspace staged closure. Proves selection, mixed-version "
    "manifest identity, release-order packaging against a staged local "
    "registry, and packaged-byte agreement with the staged tree. Does not "
    "prove installation, runtime behavior, publication, or a verification "
    "build of the real closure (the default packaging pass is --no-verify; "
    "the fixture characterization exercises the verification build)."
)
LIMITATIONS = [
    "no upload occurs and crates.io is source-replaced by the staged local registry",
    "the real-closure packaging pass runs --no-verify; sibling lanes own build verification",
    "staged bytes are vcs-info-free and are not the git-backed publish-path bytes",
    "no tag, publication, token, or repository setting is touched",
]

_SPEC = importlib.util.spec_from_file_location(
    "exact_candidate_package_candidate", CANDIDATE_PRODUCER
)
if _SPEC is None or _SPEC.loader is None:  # pragma: no cover - structural
    raise SystemExit("could not load exact-candidate-package-candidate.py")
CANDIDATE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(CANDIDATE)


def fail(message: str) -> None:
    raise SystemExit(f"exact-candidate-staged-packaging: error: {message}")


def sha256_file(path: Path) -> str:
    return CANDIDATE.sha256_file(path)


def write_atomic_json(path: Path, document: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    handle = tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", dir=path.parent, delete=False
    )
    try:
        json.dump(document, handle, indent=2, sort_keys=True)
        handle.write("\n")
        handle.close()
        Path(handle.name).replace(path)
    finally:
        Path(handle.name).unlink(missing_ok=True)


# ---------------------------------------------------------------------------
# Deterministic TOML emission for the rewritten staged manifests.
# ---------------------------------------------------------------------------


def _toml_string(value: str) -> str:
    return json.dumps(value)


def _toml_key(key: str) -> str:
    if key and all(character.isalnum() or character in "_-" for character in key):
        return key
    if "'" not in key:
        return f"'{key}'"
    return _toml_string(key)


def _toml_inline(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        return _toml_string(value)
    if isinstance(value, list):
        return "[" + ", ".join(_toml_inline(item) for item in value) + "]"
    if isinstance(value, dict):
        body = ", ".join(
            f"{_toml_key(key)} = {_toml_inline(item)}" for key, item in value.items()
        )
        return "{" + (body + " " if body else "") + "}"
    fail(f"unsupported TOML value type: {type(value)!r}")


def toml_dumps(document: dict[str, Any]) -> str:
    """Serialize a parsed manifest deterministically, preserving key order.

    Arrays of tables are emitted as ``[[header]]`` blocks so packaged
    metadata (for example ``package.metadata.*.channel``) survives a
    round-trip through tomllib unchanged.
    """
    lines: list[str] = []

    def emit_plain(node: dict[str, Any]) -> None:
        for key, item in node.items():
            if isinstance(item, (dict,)):
                continue
            if isinstance(item, list) and item and all(
                isinstance(entry, dict) for entry in item
            ):
                continue  # array of tables: emitted as [[header]] blocks
            lines.append(f"{_toml_key(key)} = {_toml_inline(item)}")

    def walk(prefix: list[str], node: dict[str, Any]) -> None:
        emit_plain(node)
        for key, item in node.items():
            child_prefix = [*prefix, key]
            header = ".".join(_toml_key(part) for part in child_prefix)
            if isinstance(item, dict):
                lines.append("")
                lines.append(f"[{header}]")
                walk(child_prefix, item)
            elif isinstance(item, list) and item and all(
                isinstance(entry, dict) for entry in item
            ):
                for entry in item:
                    lines.append("")
                    lines.append(f"[[{header}]]")
                    walk(child_prefix, entry)

    walk([], document)
    return "\n".join(lines) + "\n"


# ---------------------------------------------------------------------------
# Manifest analysis and rewriting.
# ---------------------------------------------------------------------------


def parse_toml(path: Path) -> dict[str, Any]:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def resolve_inherited_package_table(
    manifest: dict[str, Any], workspace_document: dict[str, Any]
) -> dict[str, Any]:
    """Return the [package] table with `key.workspace = true` resolved.

    The staged manifests must be standalone: every inherited key is replaced
    by the workspace value it resolves to. Version agreement with the
    topology row is enforced separately by the caller from the derived
    payload.
    """
    package = manifest.get("package")
    if not isinstance(package, dict):
        fail("staged manifest has no [package] table")
    workspace_package = (workspace_document.get("workspace") or {}).get("package") or {}
    resolved: dict[str, Any] = {}
    for key, value in package.items():
        if isinstance(value, dict) and value.get("workspace") is True:
            if key not in workspace_package:
                fail(f"package key {key!r} inherits from an absent workspace value")
            resolved[key] = workspace_package[key]
        else:
            resolved[key] = value
    return resolved


def merge_workspace_dependency_spec(
    name: str, spec: Any, workspace_document: dict[str, Any]
) -> Any:
    """Resolve a `dep.workspace = true` shorthand to its concrete spec.

    Cargo semantics: features from the local shorthand extend the workspace
    dependency's features; every other present key overrides. Non-workspace
    specs pass through unchanged.
    """
    if not isinstance(spec, dict) or spec.get("workspace") is not True:
        return spec
    workspace_deps = (workspace_document.get("workspace") or {}).get("dependencies") or {}
    if name not in workspace_deps:
        fail(f"workspace dependency {name!r} is absent from [workspace.dependencies]")
    base = workspace_deps[name]
    if isinstance(base, str):
        merged: dict[str, Any] = {"version": base}
    elif isinstance(base, dict):
        merged = dict(base)
    else:
        fail(f"workspace dependency {name!r} has an unsupported spec")
    for key, value in spec.items():
        if key == "workspace":
            continue
        if key == "features":
            combined = [str(feature) for feature in merged.get("features", [])]
            for feature in value:
                if feature not in combined:
                    combined.append(str(feature))
            merged["features"] = combined
            continue
        merged[key] = value
    return merged


def version_requirement_matches(requirement: Any, expected: str, name: str) -> bool:
    if not isinstance(requirement, str):
        return False
    return requirement.lstrip("=") == expected


def rewrite_dependency_table(
    table: dict[str, Any],
    *,
    section: str,
    workspace_document: dict[str, Any],
    selected_by_name: dict[str, dict[str, Any]],
    staged_target_for: dict[str, str],
    crate_staged_dir: str,
    dev_only_source: bool = False,
) -> dict[str, Any]:
    """Rewrite one dependency table into staged standalone form.

    Normal and build dependencies on selected rows become exact version
    requirements (path dropped). Dev dependencies on selected rows with a
    known version also become version requirements, mirroring what cargo
    freezes into the packaged manifest on the ambient path. Everything that
    would otherwise remain path-based is repointed into the staged layout;
    only dev dependencies may stay path-based, because cargo strips
    path-only dev dependencies from packaged manifests and an unpublished
    package can never satisfy a registry requirement from crates.io.

    Dev-only sources are never packaged, so all of their internal
    dependencies (selected or not, any section) stay path-based: staged
    resolution then never needs a registry row for them and no packaging
    identity is asserted for dev-only sources.
    """
    rewritten: dict[str, Any] = {}
    for name, spec in table.items():
        resolved = merge_workspace_dependency_spec(name, spec, workspace_document)
        if isinstance(resolved, str):
            if name in selected_by_name:
                fail(
                    f"dependency {name!r} on selected row {name} must be a table "
                    "carrying a version requirement"
                )
            rewritten[name] = resolved
            continue
        if not isinstance(resolved, dict):
            fail(f"dependency {name!r} has an unsupported spec")
        internal = name in selected_by_name
        if dev_only_source and ("path" in resolved or internal):
            target = staged_target_for.get(name)
            if target is None:
                fail(f"dev-only dependency {name!r} has no staged target")
            relative = os.path.relpath(target, crate_staged_dir).replace("\\", "/")
            rewritten[name] = {"path": relative}
            continue
        if internal and section in ("dependencies", "build-dependencies"):
            requirement = resolved.get("version")
            if not isinstance(requirement, str) or not requirement.strip():
                fail(
                    f"normal dependency {name!r} on selected row has no registry requirement"
                )
            expected = selected_by_name[name]["cargo_package_version"]
            if not version_requirement_matches(requirement, expected, name):
                fail(
                    f"dependency {name!r} requirement {requirement!r} disagrees with "
                    f"topology version {expected!r}"
                )
            narrowed: dict[str, Any] = {"version": requirement}
            for key in ("features", "default-features", "optional"):
                if key in resolved:
                    narrowed[key] = resolved[key]
            rewritten[name] = narrowed
            continue
        if internal and section == "dev-dependencies" and "version" in resolved:
            requirement = resolved.get("version")
            if not isinstance(requirement, str) or not requirement.strip():
                fail(f"dev dependency {name!r} on selected row has an empty version")
            narrowed = {"version": requirement}
            for key in ("features", "default-features", "optional"):
                if key in resolved:
                    narrowed[key] = resolved[key]
            rewritten[name] = narrowed
            continue
        if "path" in resolved:
            if section != "dev-dependencies":
                fail(
                    f"path dependency {name!r} in {section} cannot enter the staged closure"
                )
            target = staged_target_for.get(name)
            if target is None:
                fail(f"path dependency {name!r} has no staged target")
            relative = os.path.relpath(target, crate_staged_dir).replace("\\", "/")
            rewritten[name] = {"path": relative}
            continue
        if internal:
            fail(
                f"dev dependency {name!r} on selected row has no path or version to rewrite"
            )
        rewritten[name] = resolved
    return rewritten


def rewrite_target_tables(
    target: Any,
    *,
    workspace_document: dict[str, Any],
    selected_by_name: dict[str, dict[str, Any]],
    staged_target_for: dict[str, str],
    crate_staged_dir: str,
    dev_only_source: bool = False,
) -> Any:
    if not isinstance(target, dict):
        return target
    rewritten: dict[str, Any] = {}
    for cfg, tables in target.items():
        if not isinstance(tables, dict):
            rewritten[cfg] = tables
            continue
        rewritten[cfg] = {}
        for section, table in tables.items():
            if section in (
                "dependencies",
                "dev-dependencies",
                "build-dependencies",
            ) and isinstance(table, dict):
                rewritten[cfg][section] = rewrite_dependency_table(
                    table,
                    section=section,
                    workspace_document=workspace_document,
                    selected_by_name=selected_by_name,
                    staged_target_for=staged_target_for,
                    crate_staged_dir=crate_staged_dir,
                    dev_only_source=dev_only_source,
                )
            else:
                rewritten[cfg][section] = table
    return rewritten


def rewrite_manifest_for_staging(
    manifest: dict[str, Any],
    *,
    workspace_document: dict[str, Any],
    selected_by_name: dict[str, dict[str, Any]],
    staged_target_for: dict[str, str],
    crate_staged_dir: str,
    ambient_root: Path,
    staged_crate_root: Path,
    dev_only_source: bool = False,
) -> dict[str, Any]:
    """Rewrite a crate manifest into standalone staged form.

    The result keeps the parsed key order of the source manifest, resolves
    workspace inheritance, expresses internal dependencies per the topology,
    and drops workspace-specific sections. `[lints] workspace = true` is
    kept: the staged workspace root carries the same `[workspace.lints]`
    tables as the ambient root.
    """
    package = resolve_inherited_package_table(manifest, workspace_document)

    def inherit_outside_file(key: str) -> None:
        value = package.get(key)
        if not isinstance(value, str) or not value.strip():
            return
        local = staged_crate_root / value
        if local.is_file():
            return
        ambient = ambient_root / value
        if ambient.is_file():
            staged_crate_root.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ambient, staged_crate_root / Path(value).name)
            package[key] = Path(value).name

    inherit_outside_file("readme")
    inherit_outside_file("license-file")

    rewritten: dict[str, Any] = {}
    for key, value in manifest.items():
        if key == "package":
            rewritten["package"] = package
        elif key == "workspace":
            continue  # replaced by the staged workspace root
        elif key in ("dependencies", "dev-dependencies", "build-dependencies"):
            if isinstance(value, dict):
                rewritten[key] = rewrite_dependency_table(
                    value,
                    section=key,
                    workspace_document=workspace_document,
                    selected_by_name=selected_by_name,
                    staged_target_for=staged_target_for,
                    crate_staged_dir=crate_staged_dir,
                    dev_only_source=dev_only_source,
                )
            else:
                rewritten[key] = value
        elif key == "target":
            rewritten["target"] = rewrite_target_tables(
                value,
                workspace_document=workspace_document,
                selected_by_name=selected_by_name,
                staged_target_for=staged_target_for,
                crate_staged_dir=crate_staged_dir,
                dev_only_source=dev_only_source,
            )
        else:
            rewritten[key] = value
    return rewritten


def internal_dependency_names(
    manifest: dict[str, Any], workspace_document: dict[str, Any] | None = None
) -> set[str]:
    """Internal names referenced from any dependency table of a manifest.

    A name counts as internal-referencing when its spec carries an explicit
    `path`, or when it inherits from `[workspace.dependencies]` and that
    workspace entry carries a `path`. Version-only workspace entries are
    external registry dependencies.
    """
    workspace_deps = (workspace_document or {}).get("workspace", {}).get("dependencies") or {}

    def is_path_dependent(name: str, spec: Any) -> bool:
        if isinstance(spec, dict) and "path" in spec:
            return True
        if isinstance(spec, dict) and spec.get("workspace") is True:
            base = workspace_deps.get(name)
            return isinstance(base, dict) and "path" in base
        return False

    names: set[str] = set()

    def scan(table: Any) -> None:
        if not isinstance(table, dict):
            return
        for name, spec in table.items():
            if is_path_dependent(name, spec):
                names.add(name)

    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        scan(manifest.get(section))
    target = manifest.get("target")
    if isinstance(target, dict):
        for tables in target.values():
            if isinstance(tables, dict):
                for section in ("dependencies", "dev-dependencies", "build-dependencies"):
                    scan(tables.get(section))
    return names


def find_crate_manifest(root: Path, name: str) -> Path | None:
    """Locate a crate directory by package name using the ambient workspace."""
    for member in workspace_member_dirs(root):
        manifest = parse_toml(root / member / "Cargo.toml")
        package = manifest.get("package") or {}
        if package.get("name") == name:
            return root / member / "Cargo.toml"
    return None


def workspace_member_dirs(root: Path) -> list[str]:
    manifest = parse_toml(root / "Cargo.toml")
    workspace = manifest.get("workspace") or {}
    members = workspace.get("members") or []
    return [str(member) for member in members]


def unselected_dev_closure(
    rows: list[dict[str, Any]], root: Path
) -> dict[str, Path]:
    """Return ambient crate dirs for unselected internal dev-dependency sources.

    Selected rows may carry dev dependencies on crates outside the candidate
    (cargo-allow does). Those crates are never packaged, but the staged
    resolution needs them as path sources, so they are staged under
    `dev-only/` with the same standalone rewrite rules.
    """
    selected_by_name = {row["cargo_package_name"]: row for row in rows}
    workspace_document = parse_toml(root / "Cargo.toml")
    staged: dict[str, Path] = {}
    queue: list[str] = []
    for row in rows:
        manifest = parse_toml(root / row["workspace_source_path"] / "Cargo.toml")
        for name in sorted(internal_dependency_names(manifest, workspace_document)):
            if name not in selected_by_name and name not in staged:
                queue.append(name)
    while queue:
        name = queue.pop(0)
        if name in selected_by_name or name in staged:
            continue
        manifest_path = find_crate_manifest(root, name)
        if manifest_path is None:
            fail(f"unselected dev dependency {name!r} has no crate directory")
        manifest = parse_toml(manifest_path)
        staged[name] = manifest_path.parent
        for inner in sorted(internal_dependency_names(manifest, workspace_document)):
            if inner not in selected_by_name and inner not in staged:
                queue.append(inner)
    return staged


# ---------------------------------------------------------------------------
# Staging.
# ---------------------------------------------------------------------------


def stage_closure(
    root: Path,
    payload: dict[str, Any],
    output_root: Path,
) -> dict[str, Any]:
    """Assemble the staged closure tree atomically.

    Each selected crate is copied into `<output-root>/workspace/crates/` and
    its manifest is rewritten to standalone form. Unselected internal
    dev-dependency sources are copied under `dev-only/` and are never
    packaged. The staged workspace root declares exactly the selected
    members plus the ambient `[workspace.lints]` tables.
    """
    rows = payload["rows"]
    selected_by_name = {row["cargo_package_name"]: row for row in rows}
    workspace_document = parse_toml(root / "Cargo.toml")

    dev_only_sources = unselected_dev_closure(rows, root)
    staged_target_for: dict[str, str] = {}
    for row in rows:
        staged_target_for[row["cargo_package_name"]] = f"crates/{row['cargo_package_name']}"
    for name in dev_only_sources:
        staged_target_for[name] = f"dev-only/{name}"

    output_root.mkdir(parents=True, exist_ok=True)
    build_root = output_root / ".staging"
    if build_root.exists():
        shutil.rmtree(build_root)
    build_root.mkdir(parents=True)
    staged_root = build_root / "workspace"

    staged_members: list[str] = []
    staged_manifest_digests: dict[str, str] = {}
    try:
        for row in rows:
            name = row["cargo_package_name"]
            source_dir = root / row["workspace_source_path"]
            target_dir = staged_root / "crates" / name
            shutil.copytree(
                source_dir,
                target_dir,
                ignore=shutil.ignore_patterns("target", ".git", "*.pyc", "__pycache__"),
            )
            manifest_path = target_dir / "Cargo.toml"
            manifest = parse_toml(manifest_path)
            rewritten = rewrite_manifest_for_staging(
                manifest,
                workspace_document=workspace_document,
                selected_by_name=selected_by_name,
                staged_target_for=staged_target_for,
                crate_staged_dir=f"crates/{name}",
                ambient_root=root,
                staged_crate_root=target_dir,
            )
            package_table = rewritten.get("package") or {}
            if package_table.get("version") != row["cargo_package_version"]:
                fail(
                    f"staged manifest version {package_table.get('version')!r} disagrees "
                    f"with topology version {row['cargo_package_version']!r} for {name}"
                )
            manifest_path.write_text(
                toml_dumps(rewritten), encoding="utf-8", newline="\n"
            )
            staged_members.append(f"crates/{name}")
            staged_manifest_digests[name] = sha256_file(manifest_path)

        for name, ambient_dir in sorted(dev_only_sources.items()):
            target_dir = staged_root / "dev-only" / name
            shutil.copytree(
                ambient_dir,
                target_dir,
                ignore=shutil.ignore_patterns("target", ".git", "*.pyc", "__pycache__"),
            )
            manifest_path = target_dir / "Cargo.toml"
            manifest = parse_toml(manifest_path)
            rewritten = rewrite_manifest_for_staging(
                manifest,
                workspace_document=workspace_document,
                selected_by_name=selected_by_name,
                staged_target_for=staged_target_for,
                crate_staged_dir=f"dev-only/{name}",
                ambient_root=root,
                staged_crate_root=target_dir,
                dev_only_source=True,
            )
            # Dev-only sources are never packaged; their internal dependencies
            # stay path-based so staged resolution never needs a registry row
            # for them and no packaging identity is asserted for them.
            manifest_path.write_text(
                toml_dumps(rewritten), encoding="utf-8", newline="\n"
            )

        ambient_workspace = workspace_document.get("workspace") or {}
        workspace_table: dict[str, Any] = {}
        if "resolver" in ambient_workspace:
            workspace_table["resolver"] = ambient_workspace["resolver"]
        workspace_table["members"] = staged_members
        if isinstance(ambient_workspace.get("lints"), dict):
            workspace_table["lints"] = ambient_workspace["lints"]
        (staged_root / "Cargo.toml").write_text(
            toml_dumps({"workspace": workspace_table}),
            encoding="utf-8",
            newline="\n",
        )

        final_root = output_root / "workspace"
        if final_root.exists():
            shutil.rmtree(final_root)
        staged_root.rename(final_root)
    finally:
        if build_root.exists():
            shutil.rmtree(build_root, ignore_errors=True)

    registry_root = output_root / "staged-registry"
    if registry_root.exists():
        shutil.rmtree(registry_root)
    (registry_root / "index").mkdir(parents=True)

    return {
        "workspace_root": str((output_root / "workspace").resolve()),
        "registry_root": str(registry_root.resolve()),
        "staged_members": staged_members,
        "dev_only_staged": sorted(dev_only_sources),
        "staged_manifest_digests": staged_manifest_digests,
    }


# ---------------------------------------------------------------------------
# Staged local registry.
# ---------------------------------------------------------------------------


def index_rel_path(name: str) -> Path:
    lowered = name.lower()
    if len(lowered) == 1:
        return Path("1") / lowered
    if len(lowered) == 2:
        return Path("2") / lowered
    if len(lowered) == 3:
        return Path("3") / lowered[0] / lowered
    return Path(lowered[:2]) / lowered[2:4] / lowered


def _index_dep(
    name: str, spec: dict[str, Any], kind: str, target: str | None
) -> dict[str, Any]:
    version = spec.get("version", "*")
    if not isinstance(version, str):
        fail(f"dependency {name!r} has a non-string version requirement")
    package = spec.get("package")
    return {
        "name": name,
        "req": version if version.strip() else "*",
        "features": [str(feature) for feature in spec.get("features", [])],
        "optional": bool(spec.get("optional", False)),
        "default_features": bool(spec.get("default-features", True)),
        "target": target,
        "kind": kind,
        "registry": None,
        # The index `package` key carries the real crate name when the
        # manifest renames a dependency (`common = { package = "crypto-common" }`).
        "package": str(package) if isinstance(package, str) and package else None,
    }


def registry_index_entry(
    crate_path: Path, staged_manifest: dict[str, Any]
) -> dict[str, Any]:
    package = staged_manifest.get("package") or {}
    name = package.get("name")
    version = package.get("version")
    if not isinstance(name, str) or not isinstance(version, str):
        fail(f"staged manifest for {crate_path.name} lacks name/version")
    deps: list[dict[str, Any]] = []

    def add_entries(table: Any, kind: str, target: str | None = None) -> None:
        if not isinstance(table, dict):
            return
        for dep_name, spec in table.items():
            if isinstance(spec, str):
                deps.append(_index_dep(dep_name, {"version": spec}, kind, target))
                continue
            if not isinstance(spec, dict):
                continue
            if "path" in spec:
                if kind == "dev":
                    # cargo strips path-only dev dependencies from packaged
                    # manifests, so the registry index omits them too.
                    continue
                fail(
                    f"path dependency {dep_name!r} cannot enter the staged registry index"
                )
            deps.append(_index_dep(dep_name, spec, kind, target))

    add_entries(staged_manifest.get("dependencies"), "normal")
    add_entries(staged_manifest.get("build-dependencies"), "build")
    add_entries(staged_manifest.get("dev-dependencies"), "dev")
    target_table = staged_manifest.get("target")
    if isinstance(target_table, dict):
        for cfg, tables in target_table.items():
            if not isinstance(tables, dict):
                continue
            for section, kind in (
                ("dependencies", "normal"),
                ("build-dependencies", "build"),
                ("dev-dependencies", "dev"),
            ):
                add_entries(tables.get(section), kind, target=str(cfg))
    features = staged_manifest.get("features") or {}
    entry: dict[str, Any] = {
        "name": name,
        "vers": version,
        "deps": deps,
        # Registry index checksums are bare lowercase hex digests.
        "cksum": hashlib.sha256(crate_path.read_bytes()).hexdigest(),
        "features": features,
        "yanked": False,
    }
    links = package.get("links")
    if links:
        entry["links"] = links
    rust_version = package.get("rust-version")
    if rust_version:
        entry["rust_version"] = str(rust_version)
    return entry


def write_registry_entry(
    registry_root: Path, crate_path: Path, staged_manifest: dict[str, Any]
) -> str:
    """Inject one `.crate` into the staged registry: bytes plus index line.

    The registry is the complete mirror of a real one: the archive bytes are
    required for verification builds (cargo unpacks dependencies from the
    registry), and the index line carries the same checksum.
    """
    destination = registry_root / crate_path.name
    if crate_path != destination:
        shutil.copy2(crate_path, destination)
    entry = registry_index_entry(destination, staged_manifest)
    name = str(entry["name"])
    path = registry_root / "index" / index_rel_path(name)
    path.parent.mkdir(parents=True, exist_ok=True)
    existing: list[dict[str, Any]] = []
    if path.is_file():
        for line in path.read_text(encoding="utf-8").splitlines():
            stripped = line.strip()
            if stripped:
                existing.append(json.loads(stripped))
    version = str(entry["vers"])
    existing = [row for row in existing if str(row.get("vers")) != version]
    existing.append(entry)
    existing.sort(key=lambda row: str(row.get("vers")))
    path.write_text(
        "".join(json.dumps(row, separators=(",", ":")) + "\n" for row in existing),
        encoding="utf-8",
        newline="\n",
    )
    return path.relative_to(registry_root).as_posix()


def write_registry_config(staged_root: Path, registry_root: Path) -> None:
    """Source-replace crates.io with the staged local registry."""
    relative = os.path.relpath(registry_root, staged_root).replace("\\", "/")
    config_dir = staged_root / ".cargo"
    config_dir.mkdir(parents=True, exist_ok=True)
    config = (
        "[source.crates-io]\n"
        'replace-with = "staged-candidate-registry"\n'
        "\n"
        "[source.staged-candidate-registry]\n"
        f'local-registry = "{relative}"\n'
    )
    (config_dir / "config.toml").write_text(config, encoding="utf-8", newline="\n")


# ---------------------------------------------------------------------------
# Cargo invocation boundary.
# ---------------------------------------------------------------------------


def select_cargo() -> str:
    found = shutil.which("cargo")
    if found is None:
        fail("cargo is required on PATH for staged packaging")
    return str(Path(found).absolute())


def cargo_environment() -> dict[str, str]:
    environment = dict(os.environ)
    environment.pop("CARGO_REGISTRY_TOKEN", None)
    environment["CARGO_NET_OFFLINE"] = "true"
    return environment


def run_cargo(argv: list[str], *, cwd: Path) -> str:
    """Run the only two cargo forms this producer is allowed to run.

    The invocation boundary is deliberately narrow: offline packaging of one
    selected row, and offline lockfile generation. Nothing else (publish,
    upload, metadata) is selectable here.
    """
    if argv[:1] == ["package"]:
        if "--offline" not in argv or "-p" not in argv:
            fail(f"cargo package argv is not selected for staged packaging: {argv}")
    elif argv[:1] == ["generate-lockfile"]:
        if "--offline" not in argv:
            fail("staged lockfile generation must run offline")
    else:
        fail(f"cargo argv is not selected for staged packaging: {argv[:2]}")
    cargo = select_cargo()
    result = subprocess.run(
        [cargo, *argv],
        cwd=cwd,
        env=cargo_environment(),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    if result.returncode != 0:
        sys.stderr.write(result.stdout)
        fail(f"cargo command failed ({result.returncode}): {' '.join(argv)}")
    return result.stdout


# ---------------------------------------------------------------------------
# Packaging.
# ---------------------------------------------------------------------------


def crate_file_name(name: str, version: str) -> str:
    return f"{name}-{version}.crate"


def package_target_dir(output_root: Path) -> Path:
    return output_root / "cargo-target"


def read_manifest_from_crate(crate_path: Path) -> dict[str, Any]:
    with tarfile.open(crate_path, "r:gz") as archive:
        for member in archive.getmembers():
            name = member.name.replace("\\", "/")
            if name.count("/") == 1 and name.endswith("/Cargo.toml"):
                extracted = archive.extractfile(member)
                if extracted is None:
                    fail(f"could not read Cargo.toml from {crate_path.name}")
                return tomllib.loads(extracted.read().decode("utf-8"))
    fail(f"no top-level Cargo.toml in {crate_path.name}")


def warm_external_registry(
    registry_root: Path, ambient_lockfile: Path, cargo_home: Path
) -> list[str]:
    """Copy every external lockfile package into the staged registry.

    External requirements are identical on the ambient and staged paths (the
    rewrite preserves them), so the ambient lockfile is the authoritative
    warm set. A cache miss fails closed instead of falling back to the
    network.
    """
    text = ambient_lockfile.read_text(encoding="utf-8")
    warmed: list[str] = []
    for block in re.split(r"(?m)^\[\[package\]\]\s*$", text)[1:]:
        name_match = re.search(r'(?m)^name\s*=\s*"([^"]+)"\s*$', block)
        version_match = re.search(r'(?m)^version\s*=\s*"([^"]+)"\s*$', block)
        source_match = re.search(r'(?m)^source\s*=\s*"([^"]+)"\s*$', block)
        checksum_match = re.search(r'(?m)^checksum\s*=\s*"([^"]+)"\s*$', block)
        if not name_match or not version_match or not source_match:
            continue
        source = source_match.group(1)
        if "crates.io-index" not in source and not source.startswith("registry+"):
            continue
        name = name_match.group(1)
        version = version_match.group(1)
        cache = cargo_home / "registry" / "cache"
        matches = sorted(cache.glob(f"*/{name}-{version}.crate")) if cache.is_dir() else []
        if not matches:
            fail(
                f"external crate {name}-{version} is not in the local cargo cache; "
                "warm it (for example `cargo fetch --locked` in the workspace) before "
                "staged packaging; the staged registry never contacts crates.io"
            )
        crate_path = matches[0]
        digest = hashlib.sha256(crate_path.read_bytes()).hexdigest()
        if checksum_match and checksum_match.group(1) != digest:
            fail(
                f"cargo cache copy of {name}-{version} does not match the lockfile "
                f"checksum: {digest} != {checksum_match.group(1)}"
            )
        write_registry_entry(
            registry_root, crate_path, read_manifest_from_crate(crate_path)
        )
        warmed.append(f"{name}-{version}")
    warmed.sort()
    return warmed


def package_rows(
    root: Path,
    payload: dict[str, Any],
    output_root: Path,
    *,
    verify_build: bool,
) -> dict[str, Any]:
    """Package every selected row in release order against the staged registry.

    The per-row loop is itself the order proof: a row whose internal
    requirements resolve to rows that are not yet packaged fails here. After
    the last row the staged lockfile is finalized offline, and every row is
    re-packaged with --locked; the second pass must freeze byte-identical
    archives.
    """
    staged_root = output_root / "workspace"
    registry_root = output_root / "staged-registry"
    if not staged_root.is_dir() or not registry_root.is_dir():
        fail("staged closure is absent; run --mode stage first")
    rows = payload["rows"]
    write_registry_config(staged_root, registry_root)
    target_dir = package_target_dir(output_root)
    package_dir = target_dir / "package"
    if package_dir.exists():
        shutil.rmtree(package_dir)

    ambient_lockfile = root / "Cargo.lock"
    if not ambient_lockfile.is_file():
        fail(f"ambient lockfile is absent at {ambient_lockfile}")
    cargo_home = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
    warmed = warm_external_registry(registry_root, ambient_lockfile, cargo_home)

    rows_state: list[dict[str, Any]] = []
    for row in rows:
        name = row["cargo_package_name"]
        version = row["cargo_package_version"]
        argv = ["package", "--offline", "--target-dir", str(target_dir), "-p", name]
        if not verify_build:
            argv.insert(2, "--no-verify")
        run_cargo(argv, cwd=staged_root)
        crate_path = package_dir / crate_file_name(name, version)
        if not crate_path.is_file():
            fail(f"staged packaging produced no {crate_path.name}")
        staged_manifest = parse_toml(staged_root / "crates" / name / "Cargo.toml")
        index_path = write_registry_entry(registry_root, crate_path, staged_manifest)
        rows_state.append(
            {
                "logical_id": row["logical_id"],
                "name": name,
                "version": version,
                "release_order": row["release_order"],
                "crate": str(crate_path.resolve()),
                "crate_digest": sha256_file(crate_path),
                "crate_size_bytes": crate_path.stat().st_size,
                "registry_index": index_path,
                "state": "packaged",
            }
        )

    run_cargo(["generate-lockfile", "--offline"], cwd=staged_root)
    staged_lockfile = staged_root / "Cargo.lock"
    if not staged_lockfile.is_file():
        fail("staged lockfile was not created")
    staged_lock_digest = sha256_file(staged_lockfile)

    for row_state in rows_state:
        name = row_state["name"]
        argv = [
            "package",
            "--offline",
            "--locked",
            "--target-dir",
            str(target_dir),
            "-p",
            name,
        ]
        if not verify_build:
            argv.insert(3, "--no-verify")
        run_cargo(argv, cwd=staged_root)
        crate_path = Path(row_state["crate"])
        locked_digest = sha256_file(crate_path)
        if locked_digest != row_state["crate_digest"]:
            fail(
                f"--locked re-package changed bytes for {name}: "
                f"{row_state['crate_digest']} != {locked_digest}"
            )
        row_state["state"] = "packaged_locked_reverified"

    return {
        "warmed_registry_packages": warmed,
        "staged_lock_digest": staged_lock_digest,
        "staged_lock": str(staged_lockfile.resolve()),
        "rows": rows_state,
    }


# ---------------------------------------------------------------------------
# Verification.
# ---------------------------------------------------------------------------

CARGO_ADDED_FILES = {"Cargo.toml", "Cargo.toml.orig", "Cargo.lock"}
FORBIDDEN_PACKAGED_FILES = {".cargo_vcs_info.json"}


def _dependency_rows(manifest: dict[str, Any], internal_names: set[str]) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for section in ("dependencies", "build-dependencies"):
        table = manifest.get(section) or {}
        if not isinstance(table, dict):
            continue
        for dep_name, spec in table.items():
            if isinstance(spec, str):
                requirement: Any = spec
            elif isinstance(spec, dict):
                requirement = spec.get("version", "")
            else:
                requirement = ""
            kind = "internal" if dep_name in internal_names else "external"
            rows.append(
                {
                    "package_name": dep_name,
                    "package_version": str(requirement).lstrip("="),
                    "dependency_kind": kind,
                }
            )
    rows.sort(key=lambda row: (row["package_name"], row["package_version"]))
    return rows


def verify_staged(
    root: Path,
    payload: dict[str, Any],
    output_root: Path,
) -> dict[str, Any]:
    staged_root = output_root / "workspace"
    registry_root = output_root / "staged-registry"
    package_dir = package_target_dir(output_root) / "package"
    if not staged_root.is_dir():
        fail("staged closure is absent; run --mode stage first")
    rows = payload["rows"]
    selected_by_name = {row["cargo_package_name"]: row for row in rows}
    topology_versions = {
        row["cargo_package_name"]: row["cargo_package_version"] for row in rows
    }
    internal_names = set(selected_by_name)

    # V0: the staged workspace members are exactly the selected rows.
    staged_root_manifest = parse_toml(staged_root / "Cargo.toml")
    members = (staged_root_manifest.get("workspace") or {}).get("members") or []
    expected_members = sorted(f"crates/{row['cargo_package_name']}" for row in rows)
    if sorted(members) != expected_members:
        fail(
            "staged workspace membership differs from the topology selection: "
            f"{sorted(members)!r} != {expected_members!r}"
        )

    # V6: the packaged set is exactly the selected rows.
    observed = sorted(path.name for path in package_dir.glob("*.crate"))
    expected = sorted(
        crate_file_name(row["cargo_package_name"], row["cargo_package_version"])
        for row in rows
    )
    if observed != expected:
        fail(
            "packaged crate set differs from the topology selection: "
            f"{observed!r} != {expected!r}"
        )

    checks: list[dict[str, Any]] = []
    for row in rows:
        name = row["cargo_package_name"]
        version = row["cargo_package_version"]
        crate_path = package_dir / crate_file_name(name, version)

        # V1/V2: exact identity and recorded digest agreement.
        if not crate_path.is_file():
            fail(f"packaged crate missing for {name}: {crate_path.name}")
        digest = sha256_file(crate_path)
        if digest != row.get("crate_digest"):
            fail(
                f"digest mismatch for {name}: receipt {row.get('crate_digest')} "
                f"!= observed {digest}"
            )
        expected_size = row.get("crate_size_bytes")
        if (
            isinstance(expected_size, int)
            and crate_path.stat().st_size != expected_size
        ):
            fail(
                f"size mismatch for {name}: receipt {expected_size} != observed "
                f"{crate_path.stat().st_size}"
            )

        with tempfile.TemporaryDirectory() as directory:
            inspection = Path(directory)
            with tarfile.open(crate_path, "r:gz") as archive:
                archive.extractall(inspection, filter="data")
            unpacked = inspection / f"{name}-{version}"
            if not unpacked.is_dir():
                fail(
                    f"crate {crate_path.name} did not unpack to its exact name-version root"
                )

            for forbidden in FORBIDDEN_PACKAGED_FILES:
                if (unpacked / forbidden).exists():
                    fail(f"packaged crate {name} contains forbidden {forbidden}")

            # V4: unpack-and-diff against the staged tree. Cargo.toml is
            # verified semantically below; Cargo.toml.orig and Cargo.lock are
            # cargo additions. Everything else must be byte-identical to the
            # staged tree.
            staged_crate = staged_root / "crates" / name
            for packaged_file in sorted(unpacked.rglob("*")):
                if not packaged_file.is_file():
                    continue
                relative = packaged_file.relative_to(unpacked)
                if relative.as_posix() in CARGO_ADDED_FILES:
                    continue
                staged_file = staged_crate / relative
                if not staged_file.is_file():
                    fail(
                        f"packaged file {relative.as_posix()} of {name} is absent "
                        "from the staged tree"
                    )
                if packaged_file.read_bytes() != staged_file.read_bytes():
                    fail(
                        f"packaged file {relative.as_posix()} of {name} differs "
                        "from the staged tree"
                    )

            packaged_manifest_path = unpacked / "Cargo.toml"
            packaged_manifest = parse_toml(packaged_manifest_path)
            package_table = packaged_manifest.get("package") or {}
            if (
                package_table.get("name") != name
                or package_table.get("version") != version
            ):
                fail(
                    f"packaged manifest identity {package_table.get('name')}:"
                    f"{package_table.get('version')} disagrees with topology "
                    f"{name}:{version}"
                )

            leak = CANDIDATE.packaged_path_dependency(packaged_manifest_path)
            if leak is not None:
                fail(f"packaged manifest for {name} leaks a path dependency on {leak!r}")

            readme = package_table.get("readme")
            if isinstance(readme, str) and readme.strip():
                if not (unpacked / readme).is_file():
                    fail(
                        f"packaged crate {name} is missing its declared readme {readme!r}"
                    )

            # V5: mixed-version dependency law. Every normal or build
            # dependency on a selected row must require exactly that row's
            # topology version, and the full dependency-row set must equal
            # the staged manifest's own rows (the staged tree is the byte
            # authority for this candidate).
            observed_deps = _dependency_rows(packaged_manifest, internal_names)
            for entry in observed_deps:
                if entry["dependency_kind"] != "internal":
                    continue
                dep_name = entry["package_name"]
                packaged_spec = (packaged_manifest.get("dependencies") or {}).get(dep_name)
                packaged_requirement = (
                    packaged_spec.get("version")
                    if isinstance(packaged_spec, dict)
                    else packaged_spec
                )
                if not version_requirement_matches(
                    packaged_requirement, topology_versions[dep_name], dep_name
                ):
                    fail(
                        f"packaged manifest requires {dep_name} {packaged_requirement!r} "
                        f"but the topology selects {dep_name} "
                        f"{topology_versions[dep_name]!r}"
                    )
            staged_manifest = parse_toml(staged_crate / "Cargo.toml")
            staged_deps = _dependency_rows(staged_manifest, internal_names)
            if observed_deps != staged_deps:
                fail(
                    f"packaged dependency rows for {name} differ from the staged "
                    f"manifest: {observed_deps!r} != {staged_deps!r}"
                )

        checks.append(
            {
                "name": name,
                "version": version,
                "digest": digest,
                "unpacked_diff": "staged-tree-identical",
                "dependency_law": "topology-exact",
            }
        )

    # V7: registry index agreement for every selected row.
    for row in rows:
        name = row["cargo_package_name"]
        version = row["cargo_package_version"]
        index_path = registry_root / "index" / index_rel_path(name)
        if not index_path.is_file():
            fail(f"staged registry has no index entry for {name}")
        found = None
        for line in index_path.read_text(encoding="utf-8").splitlines():
            stripped = line.strip()
            if not stripped:
                continue
            entry = json.loads(stripped)
            if str(entry.get("vers")) == version:
                found = entry
                break
        if found is None:
            fail(f"staged registry index lacks {name} {version}")
        crate_path = package_dir / crate_file_name(name, version)
        observed_cksum = hashlib.sha256(crate_path.read_bytes()).hexdigest()
        if str(found.get("cksum")) != observed_cksum:
            fail(
                f"staged registry checksum disagrees with packaged bytes for "
                f"{name} {version}"
            )

    # V8: the staged lockfile contains every selected row at its version.
    staged_lockfile = staged_root / "Cargo.lock"
    if not staged_lockfile.is_file():
        fail("staged lockfile is absent")
    lock_text = staged_lockfile.read_text(encoding="utf-8")
    lock_packages: set[tuple[str, str]] = set()
    for block in re.split(r"(?m)^\[\[package\]\]\s*$", lock_text)[1:]:
        name_match = re.search(r'(?m)^name\s*=\s*"([^"]+)"\s*$', block)
        version_match = re.search(r'(?m)^version\s*=\s*"([^"]+)"\s*$', block)
        if name_match and version_match:
            lock_packages.add((name_match.group(1), version_match.group(1)))
    for row in rows:
        key = (row["cargo_package_name"], row["cargo_package_version"])
        if key not in lock_packages:
            fail(f"staged lockfile lacks selected row {key[0]} {key[1]}")

    return {"checks": checks, "lock_membership": "complete"}


# ---------------------------------------------------------------------------
# Entry point.
# ---------------------------------------------------------------------------


def load_payload(root: Path, topology: Path, identities: Path) -> dict[str, Any]:
    topology_path = topology if topology.is_absolute() else root / topology
    identities_path = (
        identities if identities.is_absolute() else root / identities
    )
    return CANDIDATE.derive_payload(root, topology_path, identities_path)


def output_root_for(args: argparse.Namespace) -> Path:
    if args.output_root is not None:
        path = Path(args.output_root)
        return path if path.is_absolute() else Path.cwd() / path
    return Path(tempfile.gettempdir()) / "cargo-allow-exact-candidate-staged"


def merge_receipt(path: Path, **updates: Any) -> dict[str, Any]:
    if path.is_file():
        receipt = json.loads(path.read_text(encoding="utf-8"))
    else:
        receipt = {}
    receipt.update(updates)
    write_atomic_json(path, receipt)
    return receipt


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--mode", choices=("stage", "package", "verify", "all"), required=True
    )
    parser.add_argument("--workspace-root", type=Path, default=Path.cwd())
    parser.add_argument(
        "--topology", type=Path, default=Path("policy/product-package-topology-v2.toml")
    )
    parser.add_argument(
        "--identities", type=Path, default=Path("policy/product-crates-v2.toml")
    )
    parser.add_argument("--output-root", type=Path, default=None)
    parser.add_argument(
        "--receipt",
        type=Path,
        default=Path("target/cargo-allow/staged-packaging.receipt.json"),
    )
    parser.add_argument(
        "--verify-build",
        action="store_true",
        help="run the full verification build per packaged crate (the fixture ceiling)",
    )
    args = parser.parse_args(argv)

    root = args.workspace_root.resolve()
    output_root = output_root_for(args)
    receipt_path = args.receipt if args.receipt.is_absolute() else root / args.receipt
    modes = (
        ("stage", "package", "verify") if args.mode == "all" else (args.mode,)
    )

    for mode in modes:
        if mode == "stage":
            payload = load_payload(root, args.topology, args.identities)
            staged_state = stage_closure(root, payload, output_root)
            merge_receipt(
                receipt_path,
                schema_id=SCHEMA_ID,
                schema_version=SCHEMA_VERSION,
                mode=args.mode,
                topology_id=payload.get("topology_id"),
                topology_sha256=payload.get("topology_digest"),
                repository_commit=payload.get("repository_commit"),
                repository_tree=payload.get("repository_tree"),
                ambient_cargo_lock_digest=payload.get("cargo_lock_digest"),
                output_root=str(output_root.resolve()),
                rows=[],
                complete=False,
                incident_state="none",
                claim_boundary=CLAIM_BOUNDARY,
                limitations=list(LIMITATIONS),
                staged=staged_state,
            )
            print(
                f"staged {len(staged_state['staged_members'])} selected rows -> {output_root}"
            )
        elif mode == "package":
            payload = load_payload(root, args.topology, args.identities)
            packaged = package_rows(
                root, payload, output_root, verify_build=args.verify_build
            )
            merge_receipt(
                receipt_path,
                rows=packaged["rows"],
                staged_lock=packaged["staged_lock"],
                staged_lock_digest=packaged["staged_lock_digest"],
                warmed_registry_packages=packaged["warmed_registry_packages"],
                verify_build=args.verify_build,
            )
            print(f"packaged {len(packaged['rows'])} selected rows in release order")
        elif mode == "verify":
            if not receipt_path.is_file():
                fail("receipt is absent; run --mode stage and --mode package first")
            receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
            prior_rows = receipt.get("rows") or []
            if not prior_rows:
                fail("receipt has no packaged rows; run --mode package first")
            payload = load_payload(root, args.topology, args.identities)
            for row, prior in zip(payload["rows"], prior_rows):
                row["crate_digest"] = prior.get("crate_digest")
                row["crate_size_bytes"] = prior.get("crate_size_bytes")
            verification = verify_staged(root, payload, output_root)
            merge_receipt(
                receipt_path,
                verification=verification,
                complete=True,
                incident_state="none",
            )
            print(
                f"verified {len(verification['checks'])} packaged rows against the staged tree"
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
