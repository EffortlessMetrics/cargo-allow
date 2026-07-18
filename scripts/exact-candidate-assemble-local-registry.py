#!/usr/bin/env python3
"""Assemble a classic Cargo local-registry (.crate + index) for ExactCandidate.

Reads a warm CARGO_HOME (after patched `cargo fetch`) plus candidate `.crate`
files, copies every lockfile registry package into a local-registry tree, and
writes crates.io-format index entries.

Usage:
  python3 scripts/exact-candidate-assemble-local-registry.py \\
    --lockfile PATH \\
    --cargo-home PATH \\
    --packages-dir PATH \\
    --output PATH \\
    --candidate NAME=VERSION [--candidate NAME=VERSION ...]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import sys
import tarfile
import tomllib
from pathlib import Path


def fail(message: str) -> None:
    raise SystemExit(f"exact-candidate-assemble-local-registry: {message}")


def index_rel_path(name: str) -> Path:
    n = name.lower()
    if len(n) == 1:
        return Path("1") / n
    if len(n) == 2:
        return Path("2") / n
    if len(n) == 3:
        return Path("3") / n[0] / n
    return Path(n[:2]) / n[2:4] / n


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def normalize_req(req: str) -> str:
    text = req.strip()
    if not text:
        return "*"
    if text[0] in "=~<>^*":
        return text
    return f"^{text}"


def dep_entries(table: dict | None, kind: str, target: str | None = None) -> list[dict]:
    deps: list[dict] = []
    if not table:
        return deps
    for name, spec in table.items():
        if isinstance(spec, str):
            deps.append(
                {
                    "name": name,
                    "req": normalize_req(spec),
                    "features": [],
                    "optional": False,
                    "default_features": True,
                    "target": target,
                    "kind": kind,
                    "registry": None,
                    "package": None,
                }
            )
            continue
        if not isinstance(spec, dict):
            continue
        if "path" in spec and "version" not in spec:
            fail(f"path-only dependency {name} cannot enter local-registry index")
        # Index format: `name` is the Cargo.toml key (alias); `package` is the
        # registry package name when renamed (see Cargo registry-index docs).
        package = spec.get("package")
        deps.append(
            {
                "name": name,
                "req": normalize_req(str(spec.get("version", "*"))),
                "features": list(spec.get("features") or []),
                "optional": bool(spec.get("optional", False)),
                "default_features": bool(
                    spec.get("default-features", spec.get("default_features", True))
                ),
                "target": target,
                "kind": kind,
                "registry": None,
                "package": None if package is None else str(package),
            }
        )
    return deps


def read_manifest_from_crate(crate_path: Path) -> dict:
    with tarfile.open(crate_path, "r:gz") as archive:
        manifest_member = None
        for member in archive.getmembers():
            name = member.name.replace("\\", "/")
            if name.count("/") == 1 and name.endswith("/Cargo.toml"):
                manifest_member = member
                break
        if manifest_member is None:
            fail(f"no top-level Cargo.toml in {crate_path.name}")
        extracted = archive.extractfile(manifest_member)
        if extracted is None:
            fail(f"could not read Cargo.toml from {crate_path.name}")
        return tomllib.loads(extracted.read().decode("utf-8"))


def index_entry_from_crate(crate_path: Path) -> dict:
    manifest = read_manifest_from_crate(crate_path)
    package = manifest.get("package") or {}
    name = package.get("name")
    vers = package.get("version")
    if not name or not vers:
        fail(f"missing package name/version in {crate_path.name}")
    deps: list[dict] = []
    deps.extend(dep_entries(manifest.get("dependencies"), "normal"))
    deps.extend(dep_entries(manifest.get("dev-dependencies"), "dev"))
    deps.extend(dep_entries(manifest.get("build-dependencies"), "build"))
    for section, table in manifest.items():
        if not section.startswith("target.") or not isinstance(table, dict):
            continue
        # section like 'target."cfg(unix)".dependencies'
        match = re.match(r'^target\.(.+)\.(dependencies|dev-dependencies|build-dependencies)$', section)
        if match is None:
            # TOML nested tables appear as nested dicts under target.<cfg>
            continue
        target = match.group(1).strip('"')
        kind = {
            "dependencies": "normal",
            "dev-dependencies": "dev",
            "build-dependencies": "build",
        }[match.group(2)]
        deps.extend(dep_entries(table, kind, target=target))
    # Nested target tables: target.<cfg>.dependencies
    target_root = manifest.get("target")
    if isinstance(target_root, dict):
        for target_name, target_table in target_root.items():
            if not isinstance(target_table, dict):
                continue
            deps.extend(
                dep_entries(target_table.get("dependencies"), "normal", target=target_name)
            )
            deps.extend(
                dep_entries(
                    target_table.get("dev-dependencies"), "dev", target=target_name
                )
            )
            deps.extend(
                dep_entries(
                    target_table.get("build-dependencies"), "build", target=target_name
                )
            )
    features = manifest.get("features") or {}
    if not isinstance(features, dict):
        features = {}
    entry = {
        "name": name,
        "vers": str(vers),
        "deps": deps,
        "cksum": sha256_file(crate_path),
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


def parse_lockfile_registry_packages(lockfile: Path) -> list[tuple[str, str, str]]:
    text = lockfile.read_text(encoding="utf-8")
    packages: list[tuple[str, str, str]] = []
    blocks = re.split(r"(?m)^\[\[package\]\]\s*$", text)
    for block in blocks[1:]:
        name_m = re.search(r'(?m)^name\s*=\s*"([^"]+)"\s*$', block)
        vers_m = re.search(r'(?m)^version\s*=\s*"([^"]+)"\s*$', block)
        source_m = re.search(r'(?m)^source\s*=\s*"([^"]+)"\s*$', block)
        cksum_m = re.search(r'(?m)^checksum\s*=\s*"([^"]+)"\s*$', block)
        if not name_m or not vers_m or not source_m or not cksum_m:
            continue
        source = source_m.group(1)
        if "crates.io-index" not in source and not source.startswith("registry+"):
            continue
        packages.append((name_m.group(1), vers_m.group(1), cksum_m.group(1)))
    return packages


def find_cache_crate(cargo_home: Path, name: str, version: str) -> Path | None:
    cache_root = cargo_home / "registry" / "cache"
    if not cache_root.is_dir():
        return None
    expected = f"{name}-{version}.crate"
    matches = list(cache_root.glob(f"*/{expected}"))
    if not matches:
        return None
    return matches[0]


def find_sparse_index_line(
    cargo_home: Path, name: str, version: str
) -> str | None:
    index_root = cargo_home / "registry" / "index"
    if not index_root.is_dir():
        return None
    rel = index_rel_path(name)
    for base in index_root.iterdir():
        if not base.is_dir():
            continue
        path = base / rel
        if not path.is_file():
            # sparse index may use lowercase; also try as-is
            continue
        for line in path.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line:
                continue
            try:
                data = json.loads(line)
            except json.JSONDecodeError:
                continue
            if str(data.get("vers")) == version:
                return line
    return None


def write_index_line(registry_root: Path, entry_line: str) -> None:
    data = json.loads(entry_line)
    name = data["name"]
    path = registry_root / "index" / index_rel_path(name)
    path.parent.mkdir(parents=True, exist_ok=True)
    existing: list[dict] = []
    if path.is_file():
        for line in path.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line:
                continue
            existing.append(json.loads(line))
    vers = str(data["vers"])
    existing = [row for row in existing if str(row.get("vers")) != vers]
    existing.append(data)
    existing.sort(key=lambda row: str(row.get("vers")))
    path.write_text(
        "".join(json.dumps(row, separators=(",", ":")) + "\n" for row in existing),
        encoding="utf-8",
    )


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--lockfile", type=Path, required=True)
    parser.add_argument("--cargo-home", type=Path, required=True)
    parser.add_argument("--packages-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--candidate",
        action="append",
        default=[],
        metavar="NAME=VERSION",
        help="Candidate crate to inject from --packages-dir (repeatable)",
    )
    args = parser.parse_args(argv)

    if not args.lockfile.is_file():
        fail(f"missing lockfile {args.lockfile}")
    if not args.packages_dir.is_dir():
        fail(f"missing packages dir {args.packages_dir}")

    candidates: dict[str, str] = {}
    for item in args.candidate:
        if "=" not in item:
            fail(f"candidate must be NAME=VERSION, got {item!r}")
        name, version = item.split("=", 1)
        candidates[name] = version

    if args.output.exists():
        shutil.rmtree(args.output)
    args.output.mkdir(parents=True)
    (args.output / "index").mkdir()

    registry_packages = parse_lockfile_registry_packages(args.lockfile)
    if not registry_packages:
        fail("no registry packages found in lockfile")

    # Inject / override candidate crates first so lockfile checksums win.
    candidate_files: dict[str, Path] = {}
    for name, version in candidates.items():
        crate_path = args.packages_dir / f"{name}-{version}.crate"
        if not crate_path.is_file():
            fail(f"missing candidate crate {crate_path}")
        dest = args.output / crate_path.name
        shutil.copy2(crate_path, dest)
        candidate_files[name] = dest
        entry = index_entry_from_crate(dest)
        # Prefer lockfile checksum when present so --locked install matches.
        for lock_name, lock_vers, lock_cksum in registry_packages:
            if lock_name == name and lock_vers == version:
                actual = sha256_file(dest)
                if actual != lock_cksum:
                    fail(
                        f"candidate {name}-{version} sha256 {actual} != lockfile {lock_cksum}"
                    )
                entry["cksum"] = lock_cksum
                break
        write_index_line(args.output, json.dumps(entry, separators=(",", ":")))
        print(f"injected_candidate={name}-{version}")

    copied = 0
    for name, version, cksum in registry_packages:
        dest_name = f"{name}-{version}.crate"
        dest = args.output / dest_name
        if name in candidates and candidates[name] == version:
            # Already injected from packages-dir.
            continue
        src = find_cache_crate(args.cargo_home, name, version)
        if src is None:
            fail(f"missing cached crate for {name}-{version} under {args.cargo_home}")
        actual = sha256_file(src)
        if actual != cksum:
            fail(f"cache checksum mismatch for {name}-{version}: {actual} != {cksum}")
        shutil.copy2(src, dest)
        line = find_sparse_index_line(args.cargo_home, name, version)
        if line is None:
            # Fall back to synthesizing from the .crate when sparse index is absent.
            entry = index_entry_from_crate(dest)
            entry["cksum"] = cksum
            line = json.dumps(entry, separators=(",", ":"))
        else:
            data = json.loads(line)
            data["cksum"] = cksum
            line = json.dumps(data, separators=(",", ":"))
        write_index_line(args.output, line)
        copied += 1
        print(f"copied_registry={name}-{version}")

    print(f"local_registry_ok candidates={len(candidates)} externals={copied}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
