#!/usr/bin/env python3
"""Bind #2925's checksum control to an actually resolved registry dependency."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def select_dependency(metadata: dict, rows: list[dict]) -> dict:
    """Choose one exact candidate reachable from the metadata root, never a path row."""
    candidates = {}
    for row in rows:
        key = (row["cargo_package_name"], row["cargo_package_version"])
        if key in candidates:
            raise ValueError("duplicate consumed candidate identity")
        candidates[key] = row
    resolve = metadata["resolve"]
    nodes = {node["id"]: node for node in resolve["nodes"]}
    if len(nodes) != len(resolve["nodes"]) or resolve["root"] not in nodes:
        raise ValueError("ambiguous or missing resolution root/nodes")
    reachable, pending = set(), [resolve["root"]]
    while pending:
        identity = pending.pop()
        if identity not in reachable:
            reachable.add(identity)
            pending.extend(nodes[identity]["dependencies"])
    eligible = {}
    for package in metadata["packages"]:
        key = (package["name"], package["version"])
        if (package["id"] == resolve["root"] or package["id"] not in reachable
                or not str(package.get("source") or "").startswith("registry+")
                or key not in candidates):
            continue
        if key in eligible:
            raise ValueError("ambiguous resolved candidate identity")
        eligible[key] = {"package_name": key[0], "package_version": key[1],
                         "package_id": package["id"], "source": package["source"],
                         "crate_digest": candidates[key]["crate_digest"],
                         "index_checksum": candidates[key]["index_checksum"]}
    if not eligible:
        raise ValueError("no consumed registry candidate dependency")
    return eligible[min(eligible)]


def mutate_index(files: dict[str, bytes], selected: dict, archive: bytes) -> tuple[str, bytes, dict]:
    """Change exactly one matching index line after binding its original archive bytes."""
    actual = digest(archive)
    checksum = selected["index_checksum"]
    if (re.fullmatch(r"[0-9a-f]{64}", checksum) is None
            or actual != selected["crate_digest"] or actual != "sha256:" + checksum):
        raise ValueError("consumed candidate/index/archive checksum disagreement")
    matches = []
    for path, data in sorted(files.items()):
        lines = data.splitlines(keepends=True)
        for number, line in enumerate(lines):
            if not line.strip():
                continue
            entry = json.loads(line)
            if (entry["name"], entry["vers"]) == (selected["package_name"], selected["package_version"]):
                matches.append((path, lines, number, entry))
    if len(matches) != 1:
        raise ValueError("expected exactly one selected registry index row")
    path, lines, number, entry = matches[0]
    if entry["cksum"] != checksum:
        raise ValueError("selected registry index checksum drift")
    changed = ("1" if checksum[0] == "0" else "0") + checksum[1:]
    entry["cksum"] = changed
    lines[number] = (json.dumps(entry, separators=(",", ":")) + "\n").encode()
    mutated = b"".join(lines)
    return path, mutated, {**selected, "index_path": path,
                          "original_checksum": checksum, "mutated_checksum": changed,
                          "original_index_digest": digest(files[path]),
                          "mutated_index_digest": digest(mutated)}


def checksum_refused(exit_code: int, stderr: str, selected: dict) -> bool:
    # A path containing 'checksum' or an unrelated Cargo error is not the oracle.
    refusal = re.search(r"(?im)^(?:error:\s*)?(?:checksum for |failed to verify the checksum of )`([^`\n]+)`", stderr)
    identity = selected["package_name"] + " v" + selected["package_version"]
    return (type(exit_code) is int and exit_code != 0 and refusal is not None
            and refusal.group(1) == identity)


def registry_inputs(root: Path) -> dict[str, str]:
    paths = sorted(root.glob("*.crate")) + sorted((root / "index").rglob("*"))
    result = {}
    for path in paths:
        if path.is_symlink():
            raise ValueError("registry input must not be a symlink")
        if path.is_file():
            result[path.relative_to(root).as_posix()] = digest(path.read_bytes())
    if not result:
        raise ValueError("empty registry input")
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("registry", "metadata", "consumed-rows", "manifest", "work-dir", "output"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--repository-commit", required=True)
    args = parser.parse_args()
    if re.fullmatch(r"[0-9a-f]{40}", args.repository_commit) is None:
        raise ValueError("expected exact repository commit")
    args.work_dir.mkdir()  # fresh child of the caller's already-owned directory
    args.output.mkdir(parents=True, exist_ok=False)
    record = {"schema_id": "cargo-allow.index-checksum-control.v1", "schema_version": 1,
              "repository_commit": args.repository_commit, "result": "failed",
              "commands": [], "error": None}
    try:
        metadata_bytes = args.metadata.read_bytes()
        rows_bytes = args.consumed_rows.read_bytes()
        selected = select_dependency(json.loads(metadata_bytes), json.loads(rows_bytes))
        inputs = registry_inputs(args.registry)
        index = {name: (args.registry / name).read_bytes() for name in inputs if name.startswith("index/")}
        archive_name = f"{selected['package_name']}-{selected['package_version']}.crate"
        path, mutated, mutation = mutate_index(index, selected, (args.registry / archive_name).read_bytes())
        record.update({"metadata_digest": digest(metadata_bytes), "consumed_rows_digest": digest(rows_bytes),
                       "manifest_digest": digest(args.manifest.read_bytes()), "mutation": mutation,
                       "registry_inputs": inputs})
        # Copy only index/archive inputs. Never inherit unpacked source or Cargo cache state.
        for label in ("positive", "negative"):
            registry = args.work_dir / label
            for name in inputs:
                destination = registry / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(args.registry / name, destination)
            if registry_inputs(registry) != inputs:
                raise ValueError("registry copy differs from original inputs")
            if label == "negative":
                (registry / path).write_bytes(mutated)
            cargo_home = args.work_dir / (label + "-cargo-home")
            cargo_home.mkdir()
            argv = ["cargo", "metadata", "--format-version", "1", "--offline", "--locked",
                    "--manifest-path", str(args.manifest),
                    "--config", 'source.crates-io.replace-with="candidate-local-registry"',
                    "--config", "source.candidate-local-registry.local-registry=" + json.dumps(str(registry))]
            environment = {**os.environ, "CARGO_HOME": str(cargo_home)}
            with (args.output / (label + ".stdout")).open("wb") as stdout, (args.output / (label + ".stderr")).open("wb") as stderr:
                run = subprocess.run(argv, cwd=args.manifest.parent, env=environment, stdout=stdout, stderr=stderr, check=False)
            stderr_bytes = (args.output / (label + ".stderr")).read_bytes()
            record["commands"].append({"label": label, "argv": argv, "cwd": str(args.manifest.parent), "cargo_home": str(cargo_home),
                                       "exit_code": run.returncode, "stdout_digest": digest((args.output / (label + ".stdout")).read_bytes()),
                                       "stderr_digest": digest(stderr_bytes)})
            expected = dict(inputs)
            if label == "negative":
                expected[path] = digest(mutated)
            if registry_inputs(registry) != expected or registry_inputs(args.registry) != inputs:
                raise ValueError("registry inputs changed outside the selected checksum mutation")
            if label == "positive" and run.returncode != 0:
                raise ValueError("unmutated registry control failed")
            if label == "positive" and select_dependency(json.loads((args.output / "positive.stdout").read_bytes()), json.loads(rows_bytes)) != selected:
                raise ValueError("unmutated control resolved a different selected dependency")
            if label == "negative" and not checksum_refused(run.returncode, stderr_bytes.decode("utf-8", errors="replace"), selected):
                raise ValueError("selected index mutation did not produce checksum-specific refusal")
        record["result"] = "passed"
        return 0
    except (OSError, ValueError, KeyError, TypeError) as error:
        record["error"] = str(error)
        print(f"index-checksum control failed: {error}", flush=True)
        return 1
    finally:
        (args.output / "control.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(record, sort_keys=True), flush=True)


if __name__ == "__main__":
    raise SystemExit(main())
