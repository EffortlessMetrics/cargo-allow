#!/usr/bin/env python3
"""#2267 release publication gate over cross-platform SourceCandidateSmoke.

Two modes over the same binding law:

aggregate
    Consumes one downloaded per-platform receipt directory produced by the
    release.yml ``source-candidate-smoke`` matrix plus the raw ``needs``
    result of that matrix job, verifies every required row (exact release
    identity, Passed #2266 harness receipt, exact packaged crate graph,
    binary digest), refuses unexpected rows, and emits the aggregate receipt
    that publication consumes.

verify
    Re-verifies a downloaded aggregate receipt inside the ``publish`` job
    against the live preflight identity, so a stale, foreign-run, or
    non-Passed aggregate can never reach the upload steps.

The selected platform/toolchain matrix follows #3737: Linux x86_64 and
Windows x86_64 on Rust 1.95 are required; macOS and the Rust 1.85 lane are
explicitly NotIncluded (an explicitly not-included row is not inherited
failure, and this script is the single place that classification lives).

This gate parses receipts and bindings only. It never executes project
code, never reads a registry token, and never performs an external action.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import tomllib
from pathlib import Path
from typing import Any

SCHEMA_ID = "cargo-allow.release-source-candidate-smoke.v1"
HARNESS_SCHEMA_ID = "cargo-allow.source-candidate-smoke-receipt.v1"
HARNESS_RESULT_PASSED = "Passed"
MATRIX_AUTHORITY = "#3737"

# The selected candidate matrix (#3737). Rows are required individually; a
# capability selected only on a subset of platforms is encoded here, not
# inferred from conditional YAML elsewhere in the workflow.
REQUIRED_ROWS: tuple[dict[str, str], ...] = (
    {
        "platform": "linux",
        "target": "x86_64-unknown-linux-gnu",
        "toolchain": "1.95.0",
    },
    {
        "platform": "windows",
        "target": "x86_64-pc-windows-msvc",
        "toolchain": "1.95.0",
    },
)

# Explicitly not-included rows. Their classification is recorded in every
# aggregate receipt so an absent job can never quietly widen or shrink the
# claimed platform set.
NOT_INCLUDED_ROWS: tuple[dict[str, str], ...] = (
    {
        "platform": "macos",
        "classification": "NotIncluded",
        "reason": (
            "#3737 selection: macOS is NotIncluded for the candidate matrix; "
            "an explicitly NotIncluded platform is not inherited failure and "
            "proves nothing"
        ),
    },
    {
        "platform": "rust-1.85",
        "classification": "NotIncluded",
        "reason": (
            "Rust 1.85 is the 0.1.11 rollback-baseline toolchain guidance "
            "(#2485), not a candidate build row"
        ),
    },
)

CLAIM_BOUNDARY = (
    "This receipt gates publication on exact cross-platform "
    "source-candidate smoke: every required #3737 platform row re-ran the "
    "landed #2256/#2266 harness against the exact release commit/tree and "
    "passed the packaged-crate, path-install, and first-hour journeys. It "
    "does not observe crates.io state, prove post-publication registry "
    "installation, authorize the operation, or claim macOS/Rust-1.85 "
    "support for rows explicitly marked NotIncluded."
)

ROW_FIELDS = (
    "repository",
    "ref",
    "candidate_commit",
    "candidate_tree",
    "candidate_version",
    "tag",
    "platform",
    "target",
    "toolchain",
    "required",
    "result",
    "packaged_crates",
    "harness_receipt_sha256",
    "workflow_run_id",
    "workflow_run_attempt",
)

HEX64 = re.compile(r"^[0-9a-f]{64}$")


class GateRefusal(SystemExit):
    """A fail-closed gate refusal with a stable, pinnable message."""

    def __init__(self, message: str) -> None:
        super().__init__(1)
        self.message = message


def refuse(message: str) -> None:
    print(f"::error::{message}", file=sys.stderr)
    raise GateRefusal(message)


def sha256_file(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def version_token(observed: object, prefix: str) -> str | None:
    """Return the exact version token of a `rustc --version` style string.

    The token is compared for equality, so a suffix such as `-nightly` is a
    different toolchain rather than a prefix match on the declared version.
    """
    if not isinstance(observed, str):
        return None
    text = observed.strip()
    if text.startswith(prefix):
        text = text[len(prefix):].strip()
    token = text.split()[0] if text else ""
    return token or None


def load_json(path: Path) -> dict[str, Any]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError) as error:
        refuse(f"{path} is unreadable: {error}")
    except json.JSONDecodeError as error:
        refuse(f"{path} is not valid JSON: {error}")
    if not isinstance(document, dict):
        refuse(f"{path} is not a JSON object")
    return document


def load_fixture_crates(path: Path) -> list[str]:
    try:
        document = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        refuse(f"candidate crate fixture {path} is unreadable: {error}")
    crates = document.get("crates")
    if (
        not isinstance(crates, list)
        or not crates
        or not all(isinstance(crate, str) and crate for crate in crates)
    ):
        refuse(f"candidate crate fixture {path} has no usable crates list")
    if "cargo-allow" not in crates:
        refuse(f"candidate crate fixture {path} omits the cargo-allow binary crate")
    return crates


def expected_bindings(args: argparse.Namespace) -> dict[str, str]:
    return {
        "repository": args.expected_repository,
        "ref": args.expected_ref,
        "candidate_commit": args.expected_commit,
        "candidate_tree": args.expected_tree,
        "candidate_version": args.expected_version,
        "tag": args.expected_tag,
    }


def check_binding(where: str, document: dict[str, Any], expected: dict[str, str]) -> None:
    for field, value in expected.items():
        observed = document.get(field)
        if observed != value:
            refuse(
                f"{where} binds a different {field}: expected {value!r}, "
                f"observed {observed!r}"
            )


def check_run_identity(where: str, document: dict[str, Any], args: argparse.Namespace) -> None:
    if str(document.get("workflow_run_id")) != str(args.expected_run_id):
        refuse(
            f"{where} belongs to workflow run "
            f"{document.get('workflow_run_id')!r}, expected "
            f"{args.expected_run_id!r}"
        )
    if str(document.get("workflow_run_attempt")) != str(args.expected_run_attempt):
        refuse(
            f"{where} belongs to workflow run attempt "
            f"{document.get('workflow_run_attempt')!r}, expected "
            f"{args.expected_run_attempt!r}"
        )


def check_harness_receipt(
    platform: str, harness_path: Path, row: dict[str, Any], args: argparse.Namespace
) -> dict[str, Any]:
    harness = load_json(harness_path)
    if harness.get("schema_id") != HARNESS_SCHEMA_ID:
        refuse(
            f"{platform} harness receipt is not a {HARNESS_SCHEMA_ID} receipt"
        )
    if harness.get("result") != HARNESS_RESULT_PASSED:
        refuse(
            f"{platform} harness receipt result is "
            f"{harness.get('result')!r}, never acceptable for publication"
        )
    candidate = harness.get("candidate")
    if not isinstance(candidate, dict):
        refuse(f"{platform} harness receipt has no candidate object")
    if candidate.get("git_head") != args.expected_commit:
        refuse(
            f"{platform} harness receipt is stale: binds head "
            f"{candidate.get('git_head')!r}, expected commit "
            f"{args.expected_commit!r}"
        )
    if candidate.get("workspace_version") != args.expected_version:
        refuse(
            f"{platform} harness receipt binds workspace version "
            f"{candidate.get('workspace_version')!r}, expected "
            f"{args.expected_version!r}"
        )
    binary = harness.get("installed_binary")
    if not isinstance(binary, dict):
        refuse(f"{platform} harness receipt has no installed_binary object")
    binary_sha256 = binary.get("binary_sha256")
    if not isinstance(binary_sha256, str) or not HEX64.match(binary_sha256):
        refuse(
            f"{platform} harness receipt has no exact installed binary digest"
        )
    declared_toolchain = next(
        entry["toolchain"] for entry in REQUIRED_ROWS if entry["platform"] == platform
    )
    environment = harness.get("environment")
    if not isinstance(environment, dict):
        refuse(f"{platform} harness receipt has no environment object")
    observed_rustc = environment.get("rustc_version")
    rustc_version = version_token(observed_rustc, "rustc ")
    if rustc_version != declared_toolchain:
        refuse(
            f"{platform} harness receipt does not prove Rust {declared_toolchain}: "
            f"environment.rustc_version is {observed_rustc!r}; the row's toolchain "
            "label is a declared value, not an attestation"
        )
    observed_cargo = environment.get("cargo_version")
    cargo_version = version_token(observed_cargo, "cargo ")
    if cargo_version != declared_toolchain:
        refuse(
            f"{platform} harness receipt does not prove Cargo {declared_toolchain}: "
            f"environment.cargo_version is {observed_cargo!r}"
        )
    journey = harness.get("journey")
    if not isinstance(journey, dict):
        refuse(f"{platform} harness receipt has no journey object")
    steps_expected = journey.get("steps_expected")
    steps_executed = journey.get("steps_executed")
    if not isinstance(steps_expected, list) or not isinstance(steps_executed, list):
        refuse(f"{platform} harness receipt journey is malformed")
    if [step["id"] for step in steps_executed] != steps_expected or not steps_executed:
        refuse(
            f"{platform} harness journey is partial: "
            f"{len(steps_executed)} of {len(steps_expected)} steps executed"
        )
    if any(step.get("exit_code") != 0 for step in steps_executed):
        refuse(
            f"{platform} harness journey has a non-zero step exit; a journey "
            "that recorded a failing step cannot satisfy publication"
        )
    if not harness.get("negative_controls"):
        refuse(
            f"{platform} harness receipt ran no negative controls; an "
            "unclaimed smoke profile cannot satisfy publication"
        )
    observed_digest = row.get("harness_receipt_sha256")
    if observed_digest != sha256_file(harness_path):
        refuse(
            f"{platform} row receipt harness digest {observed_digest!r} does "
            "not match the retained harness receipt bytes"
        )
    return harness


def check_row_receipt(
    platform: str,
    row_dir: Path,
    fixture_crates: list[str],
    expected: dict[str, str],
    args: argparse.Namespace,
) -> dict[str, Any]:
    row_path = row_dir / f"release-source-candidate-smoke-{platform}.receipt.json"
    if not row_path.is_file():
        refuse(
            f"required platform row {platform} is missing its receipt "
            f"({row_path}); a missing, skipped, cancelled, or timed-out row "
            "can never satisfy publication"
        )
    row = load_json(row_path)
    missing = [field for field in ROW_FIELDS if field not in row]
    if missing:
        refuse(f"{platform} row receipt is unclaimed (missing {', '.join(missing)})")
    if row.get("result") != HARNESS_RESULT_PASSED:
        refuse(
            f"{platform} row result is {row.get('result')!r}; only a Passed "
            "row satisfies publication"
        )
    if row.get("required") is not True:
        refuse(f"{platform} row is not marked required=True")
    declared = next(
        entry for entry in REQUIRED_ROWS if entry["platform"] == platform
    )
    for field in ("platform", "target", "toolchain"):
        if row.get(field) != declared[field]:
            refuse(
                f"{platform} row binds {field}={row.get(field)!r}, the "
                f"selected matrix declares {declared[field]!r}"
            )
    check_binding(f"{platform} row receipt", row, expected)
    check_run_identity(f"{platform} row receipt", row, args)
    packaged = row.get("packaged_crates")
    if not isinstance(packaged, list) or packaged != fixture_crates:
        refuse(
            f"{platform} row packaged crate graph differs from the candidate "
            "fixture; a partial package graph can never satisfy publication"
        )
    harness_path = row_dir / "source-candidate-smoke.receipt.json"
    if not harness_path.is_file():
        refuse(f"required platform row {platform} is missing its harness receipt")
    harness = check_harness_receipt(platform, harness_path, row, args)
    binary_sha256 = harness["installed_binary"]["binary_sha256"]
    row_copy = dict(row)
    row_copy["binary_sha256"] = binary_sha256
    row_copy["harness_schema_id"] = harness.get("schema_id")
    return row_copy


def run_aggregate(args: argparse.Namespace) -> dict[str, Any]:
    expected = expected_bindings(args)
    if args.needs_result != "success":
        refuse(
            f"smoke matrix did not succeed: needs.source-candidate-smoke "
            f"result is {args.needs_result!r}; failed, cancelled, timed-out, "
            "and skipped matrix results all refuse publication"
        )
    fixture_crates = load_fixture_crates(Path(args.fixture))
    rows_dir = Path(args.rows_dir)
    if not rows_dir.is_dir():
        refuse(
            f"smoke rows directory {rows_dir} is absent; no platform row was "
            "claimed and publication refuses"
        )
    observed_dirs = sorted(
        entry.name for entry in rows_dir.iterdir() if entry.is_dir()
    )
    required_platforms = sorted(row["platform"] for row in REQUIRED_ROWS)
    if observed_dirs != required_platforms:
        refuse(
            "smoke rows directory set "
            f"{observed_dirs!r} differs from the selected matrix "
            f"{required_platforms!r}; unexpected rows may not widen the "
            "claimed platform set"
        )
    rows = [
        check_row_receipt(row["platform"], rows_dir / row["platform"], fixture_crates, expected, args)
        for row in REQUIRED_ROWS
    ]
    aggregate: dict[str, Any] = {
        "schema_id": SCHEMA_ID,
        "matrix_authority": MATRIX_AUTHORITY,
        "result": HARNESS_RESULT_PASSED,
        "repository": args.expected_repository,
        "ref": args.expected_ref,
        "candidate_commit": args.expected_commit,
        "candidate_tree": args.expected_tree,
        "candidate_version": args.expected_version,
        "tag": args.expected_tag,
        "workflow_run_id": int(args.expected_run_id),
        "workflow_run_attempt": int(args.expected_run_attempt),
        "required_rows": rows,
        "not_included_rows": [dict(entry) for entry in NOT_INCLUDED_ROWS],
        "claim_boundary": CLAIM_BOUNDARY,
        "limitations": [
            "does_not_observe_registry",
            "does_not_authorize_operation",
            "does_not_prove_post_publication_installation",
            "does_not_claim_not_included_platforms",
        ],
    }
    return aggregate


def check_aggregate(
    aggregate: dict[str, Any], expected: dict[str, str], args: argparse.Namespace
) -> None:
    if aggregate.get("schema_id") != SCHEMA_ID:
        refuse(f"aggregate is not a {SCHEMA_ID} receipt")
    if aggregate.get("result") != HARNESS_RESULT_PASSED:
        refuse(
            f"aggregate is not Passed (result {aggregate.get('result')!r}); "
            "publication refuses"
        )
    check_binding("aggregate", aggregate, expected)
    check_run_identity("aggregate", aggregate, args)
    rows = aggregate.get("required_rows")
    if not isinstance(rows, list):
        refuse("aggregate has no required_rows table")
    observed_platforms = []
    for row in rows:
        if not isinstance(row, dict):
            refuse("aggregate required_rows entries must be objects")
        platform = row.get("platform")
        observed_platforms.append(platform)
        declared = next(
            (entry for entry in REQUIRED_ROWS if entry["platform"] == platform),
            None,
        )
        if declared is None:
            refuse(
                f"aggregate required row {platform!r} is not in the selected "
                "matrix"
            )
        if row.get("result") != HARNESS_RESULT_PASSED:
            refuse(
                f"aggregate required row {platform} result is "
                f"{row.get('result')!r}; publication refuses"
            )
        for field in ("target", "toolchain"):
            if row.get(field) != declared[field]:
                refuse(
                    f"aggregate required row {platform} binds {field}="
                    f"{row.get(field)!r}, the selected matrix declares "
                    f"{declared[field]!r}"
                )
        if not isinstance(row.get("binary_sha256"), str) or not HEX64.match(
            row["binary_sha256"]
        ):
            refuse(
                f"aggregate required row {platform} has no installed binary "
                "digest"
            )
        if not isinstance(row.get("harness_receipt_sha256"), str):
            refuse(
                f"aggregate required row {platform} has no harness receipt "
                "digest"
            )
    required_platforms = sorted(entry["platform"] for entry in REQUIRED_ROWS)
    if sorted(observed_platforms) != required_platforms:
        refuse(
            "aggregate is missing a required platform row: observed "
            f"{sorted(observed_platforms)!r}, selected matrix "
            f"{required_platforms!r}"
        )
    not_included = aggregate.get("not_included_rows")
    if not isinstance(not_included, list):
        refuse("aggregate does not record the NotIncluded row classification")
    declared_classifications = sorted(
        (entry["platform"], entry["classification"]) for entry in NOT_INCLUDED_ROWS
    )
    observed_classifications = sorted(
        (entry.get("platform"), entry.get("classification"))
        for entry in not_included
        if isinstance(entry, dict)
    )
    if observed_classifications != declared_classifications:
        refuse(
            "aggregate NotIncluded classification drifted: observed "
            f"{observed_classifications!r}, declared "
            f"{declared_classifications!r}"
        )


def run_verify(args: argparse.Namespace) -> dict[str, Any]:
    aggregate_path = Path(args.aggregate)
    if not aggregate_path.is_file():
        refuse(
            f"source-candidate smoke aggregate is absent at {aggregate_path}; "
            "publication cannot proceed without the gate receipt"
        )
    aggregate = load_json(aggregate_path)
    check_aggregate(aggregate, expected_bindings(args), args)
    verification = {
        "verification_of": SCHEMA_ID,
        "result": HARNESS_RESULT_PASSED,
        "candidate_commit": args.expected_commit,
        "candidate_tree": args.expected_tree,
        "candidate_version": args.expected_version,
        "workflow_run_id": int(args.expected_run_id),
        "workflow_run_attempt": int(args.expected_run_attempt),
        "verified_platforms": [
            row.get("platform") for row in aggregate.get("required_rows", [])
        ],
    }
    return verification


def write_receipt(path: str, document: dict[str, Any]) -> None:
    destination = Path(path)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(
        (json.dumps(document, indent=2, ensure_ascii=False) + "\n").encode("utf-8")
    )


def add_expected_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--expected-repository", required=True)
    parser.add_argument("--expected-ref", required=True)
    parser.add_argument("--expected-commit", required=True)
    parser.add_argument("--expected-tree", required=True)
    parser.add_argument("--expected-version", required=True)
    parser.add_argument("--expected-tag", default="")
    parser.add_argument("--expected-run-id", required=True)
    parser.add_argument("--expected-run-attempt", required=True)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="mode", required=True)

    aggregate = subparsers.add_parser("aggregate", help="build the gate receipt")
    aggregate.add_argument("--rows-dir", required=True)
    aggregate.add_argument("--fixture", required=True)
    aggregate.add_argument("--needs-result", required=True)
    add_expected_arguments(aggregate)
    aggregate.add_argument("--output", required=True)

    verify = subparsers.add_parser("verify", help="verify a gate receipt")
    verify.add_argument("--aggregate", required=True)
    add_expected_arguments(verify)
    verify.add_argument("--output", required=True)

    args = parser.parse_args(argv)
    if args.mode == "aggregate":
        document = run_aggregate(args)
    else:
        document = run_verify(args)
    write_receipt(args.output, document)
    print(
        f"release-source-candidate-smoke-gate: {args.mode} passed; "
        f"receipt: {args.output}"
    )
    return 0


if __name__ == "__main__":
    # GateRefusal subclasses SystemExit(1): the ::error line is already
    # printed and the nonzero exit propagates unchanged.
    raise SystemExit(main())
