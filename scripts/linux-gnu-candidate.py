#!/usr/bin/env python3
"""Inspect GNU ABI needs or export the already-qualified #2925 candidate (#3151).

No Cargo build, installation, attestation or publication is performed here.
The JSON bundle manifest is CI evidence, not a new cargo-allow CLI artifact.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parent.parent
TARGET = "x86_64-unknown-linux-gnu"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def digest_bytes(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def version(value: str) -> tuple[int, int, int]:
    require(re.fullmatch(r"[0-9]+\.[0-9]+(?:\.[0-9]+)?", value) is not None,
            "unsupported GLIBC version label")
    parts = [int(part) for part in value.split(".")]
    return tuple((parts + [0])[:3])


def glibc_needs(output: str) -> list[str]:
    sections = re.split(r"(?m)^(?=Version (?:symbols|definition|needs) section )", output)
    needs = [part for part in sections if part.startswith("Version needs section ")]
    require(len(needs) == 1, "ELF inspection has no unique version-needs section")
    section = needs[0]
    header = re.match(r"Version needs section .+ contains ([0-9]+) entr(?:y|ies):", section)
    files = re.findall(r"File:\s+\S+\s+Cnt:\s+([0-9]+)", section)
    names = re.findall(r"Name:\s+(\S+)\s+Flags:", section)
    require(header is not None and int(header[1]) == len(files) and bool(files),
            "malformed ELF version-needs file count")
    require(sum(int(count) for count in files) == len(names),
            "malformed ELF version-needs name count")
    glibc = {name.removeprefix("GLIBC_") for name in names if name.startswith("GLIBC_")}
    require(bool(glibc), "ELF has no GLIBC version requirements")
    return sorted(glibc, key=version)


def inspect_abi(binary: Path, maximum: str) -> dict:
    require(binary.is_file() and not binary.is_symlink(), "binary must be a regular non-symlink file")
    require(binary.stat().st_size <= 32 * 1024 * 1024, "binary exceeds the 32 MiB inspection bound")
    data = binary.read_bytes()
    require(len(data) >= 64 and data[:7] == b"\x7fELF\x02\x01\x01"
            and data[7] in (0, 3), "expected a little-endian ELF64 GNU executable")
    elf_type, machine, generation = struct.unpack_from("<HHI", data, 16)
    require(elf_type in (2, 3) and machine == 62 and generation == 1,
            "expected an x86_64 ELF executable")
    result = subprocess.run(["readelf", "--wide", "--version-info", str(binary)],
                            capture_output=True, text=True, check=False,
                            env={**os.environ, "LC_ALL": "C"}, timeout=30)
    require(result.returncode == 0 and not result.stderr.strip(),
            "readelf inspection failed or emitted diagnostics")
    requirements = glibc_needs(result.stdout)
    require(all(version(item) <= version(maximum) for item in requirements),
            f"binary requires GLIBC_{requirements[-1]}, above the GLIBC_{maximum} baseline")
    binary_digest = digest_bytes(data)
    require(digest_bytes(binary.read_bytes()) == binary_digest, "binary changed during ABI inspection")
    return {"target_triple": TARGET, "max_glibc": maximum,
            "required_glibc": requirements, "executable_sha256": binary_digest}


def validate_predecessors(payloads: list[dict], digests: list[str],
                          commit: str, tree: str, binary_digest: str) -> str:
    candidate, install, qualification = payloads
    for payload, name in zip(payloads, ("package-candidate", "isolated-install", "exact-candidate")):
        require(payload.get("schema_id") == f"cargo-allow.{name}.v2"
                and payload.get("schema_version") == 2, f"unsupported {name} generation")
        require(payload.get("repository_commit") == commit and payload.get("repository_tree") == tree,
                f"stale {name} source identity")
    require(install.get("candidate_artifact_digest") == digests[0]
            and qualification.get("candidate_artifact_digest") == digests[0]
            and qualification.get("isolated_install_receipt_digest") == digests[1],
            "predecessor receipt digest mismatch")
    expected_version = "cargo-allow " + candidate["root_package_version"]
    for payload in (install, qualification):
        require(payload.get("installed_executable_digest") == binary_digest,
                "installed binary digest mismatch")
        require(payload.get("installed_version_output") == expected_version
                and payload.get("platform") == TARGET, "installed version or platform mismatch")
    require(install.get("source_checkout_denied") is True, "source-isolated installation not proven")
    graph = install.get("graph_comparison") or {}
    require(graph.get("expected_packages", 0) > 0
            and graph.get("matched_packages") == graph.get("expected_packages")
            and not any(graph.get(key) for key in
                        ("unexpected_packages", "missing_packages", "version_mismatches", "path_sources")),
            "isolated installation graph is not Complete")
    steps = qualification.get("journey_steps") or []
    require(bool(steps) and all(type(step.get("exit_code")) is int and step["exit_code"] == 0 for step in steps)
            and qualification.get("scanner_completeness") == "complete",
            "qualified journey is not Complete")
    # Lock digests intentionally retain their owners' different input meanings:
    # normalized workspace, packaged root lock, and raw workspace respectively.
    return candidate["root_package_version"]


def command(argv: list[str], **kwargs) -> str:
    return subprocess.run(argv, cwd=ROOT, check=True, text=True,
                          stdout=subprocess.PIPE, **kwargs).stdout.strip()


def package(output: Path) -> None:
    import tomllib  # Only the hosted producer needs Python >=3.11; inspect works on 3.10.

    baseline = tomllib.loads((ROOT / "docs/support-matrix.toml").read_text())["candidate_linux_binary"]["glibc_baseline"]
    host_libc = command(["getconf", "GNU_LIBC_VERSION"])
    require(host_libc == f"glibc {baseline}", "producer must run on the declared GLIBC baseline")
    require(not output.exists(), "output already exists; choose a fresh candidate directory")
    require(not command(["git", "status", "--porcelain", "--untracked-files=no"]),
            "candidate export requires an unchanged source checkout")
    commit = command(["git", "rev-parse", "HEAD"])
    tree = command(["git", "rev-parse", "HEAD^{tree}"])
    binary = ROOT / "target/exact-candidate-isolated-install/install/bin/cargo-allow"
    paths = [ROOT / "target/exact-candidate-package-candidate/package-candidate-v2.json",
             ROOT / "target/exact-candidate-isolated-install/isolated-install.receipt.json",
             ROOT / "target/exact-candidate-qualification/exact-candidate.receipt.v2.json"]
    contents = [path.read_bytes() for path in paths]
    payloads = [json.loads(data) for data in contents]
    digests = [digest_bytes(data) for data in contents]
    abi = inspect_abi(binary, baseline)  # No candidate execution precedes this preflight.
    candidate_version = validate_predecessors(payloads, digests, commit, tree, abi["executable_sha256"])
    classification = command([sys.executable, "scripts/exact-candidate-isolated-install.py",
                              "--mode", "classify", "--candidate-artifact", str(paths[0]),
                              "--input-receipt", str(paths[1])])
    require(classification == "Complete", "isolated-install classifier did not return Complete")
    output.mkdir(parents=True)
    environment = {**os.environ, "CARGO_ALLOW_BIN": str(binary), "RELEASE_TAG": "",
                   "RELEASE_COMMIT": commit, "RELEASE_TREE": tree, "ATTESTATION_VERIFIED": "false"}
    command(["bash", "scripts/package-release-binary.sh", "--candidate", "--version", candidate_version,
             "--output-dir", str(output)], env=environment)
    archive = output / f"cargo-allow-v{candidate_version}-{TARGET}.tar.gz"
    package_receipt = json.loads((output / "release-binary.receipt.json").read_bytes())
    require(package_receipt["executable_sha256"] == abi["executable_sha256"]
            and package_receipt["archive_sha256"] == digest_bytes(archive.read_bytes()),
            "packaged candidate digest mismatch")
    with tarfile.open(archive, "r:gz") as bundle:
        members = [member for member in bundle.getmembers() if member.name.endswith("/cargo-allow")]
        require(len(members) == 1 and members[0].isfile() and members[0].size == binary.stat().st_size,
                "archive executable is not the inspected regular file")
        with bundle.extractfile(members[0]) as source:
            require(digest_bytes(source.read()) == abi["executable_sha256"],
                    "archive executable digest mismatch before execution")
    smoke_path = output / "release-binary-install.receipt.json"
    command(["bash", "scripts/verify-release-binary.sh", "--version", candidate_version,
             "--receipt", str(smoke_path), str(archive)], env=environment)
    smoke = json.loads(smoke_path.read_bytes())
    require(smoke["result"] == "pass" and smoke["attestation_verified"] is False
            and smoke["executable_sha256"] == abi["executable_sha256"]
            and smoke["archive_sha256"] == package_receipt["archive_sha256"]
            and smoke["commit"] == commit and smoke["tree"] == tree and smoke["tag"] == "",
            "archive smoke does not bind this unpublished candidate")
    require(digest_bytes(binary.read_bytes()) == abi["executable_sha256"],
            "installed binary changed during packaging")
    for path, data in zip(paths, contents):
        require(path.read_bytes() == data, "predecessor changed during packaging")
        (output / path.name).write_bytes(data)
    manifest = {"format_version": 1, "result": "qualified-source-candidate", **abi,
                "repository_commit": commit, "repository_tree": tree,
                "version": candidate_version, "producer_glibc": host_libc,
                "rust_toolchain": payloads[1]["toolchain"],
                "archive_name": archive.name, "archive_bytes": archive.stat().st_size,
                "executable_bytes": binary.stat().st_size,
                "archive_sha256": package_receipt["archive_sha256"],
                "predecessors": {path.name: digest for path, digest in zip(paths, digests)},
                "archive_smoke_sha256": digest_bytes(smoke_path.read_bytes()),
                "publication": "none", "attestation": "not-claimed",
                "claim_boundary": "Exact unpublished installed candidate and archive exercised on the declared GNU libc baseline. No published channel, universal Linux compatibility, external adoption or final release experience is claimed."}
    (output / "linux-gnu-candidate.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    print(json.dumps(manifest, indent=2, sort_keys=True))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="command", required=True)
    inspect = subcommands.add_parser("inspect")
    inspect.add_argument("binary", type=Path)
    inspect.add_argument("--max-glibc", required=True)
    export = subcommands.add_parser("package")
    export.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "inspect":
            print(json.dumps(inspect_abi(args.binary, args.max_glibc), indent=2, sort_keys=True))
        else:
            package(args.output.resolve())
    except (ValueError, KeyError, OSError, subprocess.SubprocessError) as error:
        print(f"linux-gnu-candidate: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
