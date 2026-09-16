#!/usr/bin/env python3
"""Project a direct-floor proof onto only the selected product workspace."""

from __future__ import annotations

import hashlib
import json
import posixpath
from pathlib import Path
import sys
import tomllib
from typing import Any


SCHEMA_ID = "cargo-allow.direct-floor-product-workspace.v1"


def _sha256(raw: bytes) -> str:
    return "sha256:v1:" + hashlib.sha256(raw).hexdigest()


def _member_index(root: Path, workspace: dict[str, Any]) -> tuple[dict[str, str], dict[str, str]]:
    name_to_path: dict[str, str] = {}
    path_to_name: dict[str, str] = {}
    for raw_member in workspace.get("members", []):
        member = posixpath.normpath(str(raw_member).replace("\\", "/"))
        manifest_path = root / member / "Cargo.toml"
        manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        name = str(manifest["package"]["name"])
        if name in name_to_path:
            raise ValueError(f"duplicate workspace package name {name}")
        if member in path_to_name:
            raise ValueError(f"duplicate workspace member path {member}")
        name_to_path[name] = member
        path_to_name[member] = name
    return name_to_path, path_to_name


def _resolved_spec(dependency: str, spec: object, workspace_dependencies: dict[str, Any]) -> tuple[object, bool]:
    inherited = isinstance(spec, dict) and bool(spec.get("workspace"))
    if inherited:
        if dependency not in workspace_dependencies:
            raise ValueError(f"workspace dependency {dependency} is inherited but not declared")
        return workspace_dependencies[dependency], True
    return spec, False


def _path_target(owner_path: str, spec: object, *, inherited: bool) -> str | None:
    if not isinstance(spec, dict) or "path" not in spec:
        return None
    raw = str(spec["path"]).replace("\\", "/")
    return posixpath.normpath(raw if inherited else posixpath.join(owner_path, raw))


def _execution_members(
    root: Path,
    selected_paths: list[str],
    path_to_name: dict[str, str],
    workspace_dependencies: dict[str, Any],
) -> list[str]:
    """Include workspace path dependencies causally reachable from the selection.

    Development dependencies enter the execution workspace because the release-set
    proof runs tests. Their external requirements are not added to certified floor
    rows.
    """

    selected = set(selected_paths)
    pending = list(selected_paths)
    while pending:
        owner_path = pending.pop()
        manifest = tomllib.loads((root / owner_path / "Cargo.toml").read_text(encoding="utf-8"))
        for table in ("dependencies", "build-dependencies", "dev-dependencies"):
            for dependency, authored_spec in manifest.get(table, {}).items():
                spec, inherited = _resolved_spec(dependency, authored_spec, workspace_dependencies)
                target = _path_target(owner_path, spec, inherited=inherited)
                if target is None:
                    continue
                if target not in path_to_name:
                    raise ValueError(
                        f"{path_to_name[owner_path]} path dependency {dependency} "
                        f"leaves the workspace ({target})"
                    )
                if target not in selected:
                    selected.add(target)
                    pending.append(target)
    return sorted(selected)


def _replace_array_assignment(text: str, *, section: str, key: str, values: list[str]) -> str:
    lines = text.splitlines(keepends=True)
    section_header = f"[{section}]"
    try:
        section_start = next(index for index, line in enumerate(lines) if line.strip() == section_header)
    except StopIteration as error:
        raise ValueError(f"missing [{section}] table") from error

    section_end = len(lines)
    for index in range(section_start + 1, len(lines)):
        stripped = lines[index].strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            section_end = index
            break

    assignment_start: int | None = None
    assignment_end: int | None = None
    for index in range(section_start + 1, section_end):
        stripped = lines[index].lstrip()
        if not stripped.startswith(key):
            continue
        if not stripped[len(key) :].lstrip().startswith("="):
            continue
        assignment_start = index
        balance = lines[index].count("[") - lines[index].count("]")
        assignment_end = index + 1
        while balance > 0 and assignment_end < section_end:
            balance += lines[assignment_end].count("[")
            balance -= lines[assignment_end].count("]")
            assignment_end += 1
        if balance != 0:
            raise ValueError(f"unterminated {key} array in [{section}]")
        break

    rendered = [f"{key} = [\n"]
    rendered.extend(f'  "{value}",\n' for value in values)
    rendered.append("]\n")
    if assignment_start is None:
        lines[section_end:section_end] = rendered
    else:
        lines[assignment_start:assignment_end] = rendered
    return "".join(lines)


def project_workspace(manifest_path: Path, selection_path: Path, identity_path: Path) -> dict[str, Any]:
    manifest_path = manifest_path.resolve()
    root = manifest_path.parent
    source_raw = manifest_path.read_bytes()
    source_text = source_raw.decode("utf-8")
    document = tomllib.loads(source_text)
    workspace = document.get("workspace")
    if not isinstance(workspace, dict):
        raise ValueError("root manifest does not define [workspace]")

    selection = json.loads(selection_path.read_text(encoding="utf-8"))
    closure = selection.get("closure")
    roots = selection.get("roots")
    if not isinstance(closure, list) or not closure:
        raise ValueError("selection has no non-empty closure")
    if not isinstance(roots, list) or not roots:
        raise ValueError("selection has no non-empty roots")

    name_to_path, path_to_name = _member_index(root, workspace)
    unknown = sorted(set(map(str, closure)) - set(name_to_path))
    if unknown:
        raise ValueError("selection names packages outside the workspace: " + ", ".join(unknown))
    missing_roots = sorted(set(map(str, roots)) - set(map(str, closure)))
    if missing_roots:
        raise ValueError("selection roots are absent from its closure: " + ", ".join(missing_roots))

    source_member_paths = [
        posixpath.normpath(str(path).replace("\\", "/")) for path in workspace.get("members", [])
    ]
    selected_names = sorted(set(map(str, closure)))
    selected_paths = [path for path in source_member_paths if path_to_name[path] in selected_names]
    workspace_dependencies = workspace.get("dependencies", {})
    if not isinstance(workspace_dependencies, dict):
        raise ValueError("[workspace.dependencies] must be a table")

    execution_path_set = set(
        _execution_members(root, selected_paths, path_to_name, workspace_dependencies)
    )
    execution_paths = [path for path in source_member_paths if path in execution_path_set]
    execution_names = [path_to_name[path] for path in execution_paths]
    omitted_paths = [path for path in source_member_paths if path not in execution_path_set]

    projected = _replace_array_assignment(
        source_text, section="workspace", key="members", values=execution_paths
    )
    projected = _replace_array_assignment(
        projected, section="workspace", key="default-members", values=selected_paths
    )
    projected_raw = projected.encode("utf-8")
    reparsed = tomllib.loads(projected)
    if reparsed["workspace"].get("members") != execution_paths:
        raise ValueError("projected members did not round-trip")
    if reparsed["workspace"].get("default-members") != selected_paths:
        raise ValueError("projected default-members did not round-trip")

    manifest_path.write_bytes(projected_raw)
    identity = {
        "schema_id": SCHEMA_ID,
        "source_manifest_digest": _sha256(source_raw.replace(b"\r", b"")),
        "projected_manifest_digest": _sha256(projected_raw.replace(b"\r", b"")),
        "certified_packages": selected_names,
        "certified_member_paths": selected_paths,
        "execution_packages": execution_names,
        "execution_member_paths": execution_paths,
        "omitted_member_paths": omitted_paths,
        "claim_boundary": (
            "Deterministic root-workspace membership projection for one selected "
            "direct-floor proof. It changes no package manifest or dependency "
            "requirement and grants no support or publication authority."
        ),
    }
    identity_path.parent.mkdir(parents=True, exist_ok=True)
    identity_path.write_text(json.dumps(identity, indent=1) + "\n", encoding="utf-8", newline="\n")
    return identity


def main(argv: list[str]) -> int:
    if len(argv) != 4:
        print(
            "usage: floor_product_workspace.py "
            "<Cargo.toml> <floors-selection.json> <identity.json>",
            file=sys.stderr,
        )
        return 2
    try:
        identity = project_workspace(Path(argv[1]), Path(argv[2]), Path(argv[3]))
    except (OSError, ValueError, KeyError, TypeError, tomllib.TOMLDecodeError) as error:
        print(f"floor-product-workspace: {error}", file=sys.stderr)
        return 1
    print(json.dumps(identity, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
