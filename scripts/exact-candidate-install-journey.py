#!/usr/bin/env python3
"""Run the exact installed journey and publish its receipt after owned cleanup."""

from __future__ import annotations

import argparse
import importlib.util
import os
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def sibling(name: str):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), ROOT / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


LIFECYCLE = sibling("candidate-harness-owned-dir")
VALIDATOR = sibling("validate-exact-candidate-install-journey")


def run(root: Path = ROOT) -> Path:
    target = root / "target"
    LIFECYCLE.validate_target(root, target)
    target.mkdir(exist_ok=True)
    LIFECYCLE.validate_target(root, target)
    package_dir = LIFECYCLE.validate_contained_directory(
        target, Path(os.environ.get("PACKAGE_SET_DIR", target / "exact-candidate-package-set")))
    work = LIFECYCLE.validate_work_directory(
        Path(os.environ.get("WORK_DIR", target / "exact-candidate-install-journey")), root, target)
    if work == package_dir or work in package_dir.parents or package_dir in work.parents:
        VALIDATOR.fail("WORK_DIR and PACKAGE_SET_DIR must not overlap")
    if work.exists():
        VALIDATOR.fail("WORK_DIR must be newly allocated for this run")
    consumer = None
    if os.environ.get("CONSUMER_DIR"):
        consumer = LIFECYCLE.validate_caller_directory(Path(os.environ["CONSUMER_DIR"]), root)
        if consumer.exists():
            VALIDATOR.fail("CONSUMER_DIR must be newly allocated for this run")
    install = LIFECYCLE.validate_contained_directory(package_dir, package_dir / "install")
    binary = install / "bin" / "cargo-allow"
    if binary.with_suffix(".exe").is_file():
        binary = binary.with_suffix(".exe")
    if not binary.is_file() or LIFECYCLE.is_reparse_point(binary) or install not in binary.resolve().parents:
        VALIDATOR.fail("exact installed binary must be a regular file below the package install root")
    if os.environ.get("CARGO_ALLOW_BIN") and Path(os.environ["CARGO_ALLOW_BIN"]).resolve() != binary.resolve():
        VALIDATOR.fail("CARGO_ALLOW_BIN must identify the exact package install binary")
    head = LIFECYCLE.run_git(["-C", str(root), "rev-parse", "HEAD"],
                             check=True, capture_output=True, text=True).stdout.strip()
    schemas = root / "docs/dogfood/fixtures/release"
    args = argparse.Namespace(
        receipt=work / "exact-candidate-install-journey.receipt.json",
        schema=schemas / "exact-candidate-install-journey.v1.schema.json",
        fixture=schemas / "candidate-crate-set.toml",
        topology=root / "policy/product-package-topology-v2.toml",
        package_receipt=package_dir / "exact-candidate-package-set.receipt.json",
        package_schema=schemas / "exact-candidate-package-set.v1.schema.json",
        journey_receipt=work / "source-candidate-smoke.receipt.json",
        journey_schema=schemas / "source-candidate-smoke-receipt.v1.schema.json",
        binary=binary,
        current_head=head,
    )
    package = VALIDATOR.validate_package_receipt(
        args.package_receipt, args.package_schema, args.fixture, args.topology)
    if package["candidate"]["git_head"] != head or package["install"]["binary_sha256"] != VALIDATOR.sha256_file(binary):
        VALIDATOR.fail("package receipt does not bind the current head and installed bytes")
    # Exclusive creation leaves any pre-existing caller content untouched.
    work.mkdir()
    LIFECYCLE.claim(work, "exact-candidate-install-journey")
    source_dir, source_token = LIFECYCLE.allocate(work, "exact-candidate-source-journey", False)
    environment = {**os.environ, "CARGO_ALLOW_BIN": str(binary), "WORK_DIR": str(source_dir)}
    # A failed child may have refused cleanup. Do not recursively remove its
    # parent, retry the refusal, or publish a Passed receipt on this path.
    subprocess.run(["bash", str(root / "scripts/source-candidate-smoke.sh")],
                   cwd=root, env=environment, check=True)
    source_path = source_dir / "source-candidate-smoke.receipt.json"
    source = VALIDATOR.validate_journey_receipt(source_path, args.journey_schema)
    if consumer is not None and (consumer.exists() or consumer.is_symlink()):
        VALIDATOR.fail("source journey left its temporary consumer behind")
    expected_leaves = {LIFECYCLE.MARKER, source_path.name}
    if {path.name for path in source_dir.iterdir()} != expected_leaves:
        VALIDATOR.fail("source journey left temporary state; refusing parent cleanup")
    args.journey_receipt.write_bytes(source_path.read_bytes())
    candidate = VALIDATOR.assemble_receipt(package, source, args)
    VALIDATOR.validate_binding(package, source, candidate, args)
    LIFECYCLE.remove(work, source_dir, "exact-candidate-source-journey", source_token)
    if source_dir.exists() or source_dir.is_symlink():
        VALIDATOR.fail("temporary source journey artifacts remain")
    VALIDATOR.publish_receipt(package, source, args)
    return args.receipt


if __name__ == "__main__":
    try:
        print(f"ExactCandidateInstallJourneyV1 passed; receipt: {run()}")
    except (OSError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"exact-candidate-install-journey: {error}") from error
