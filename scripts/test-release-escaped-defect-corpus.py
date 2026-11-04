#!/usr/bin/env python3
"""RC.1 escaped-defect corpus fixtures (#3793).

One zero-upload fixture per historical RC.1 escape whose production seam has
landed on main. Every fixture re-establishes the old false-green input and
drives the actual production adapter the release workflow uses, asserting the
exact semantic rejection (or exact exactness acceptance) and that downstream
reachability is blocked. No fixture compiles code, reads a registry, touches a
token value, or mutates the checkout: provider and process functions are
replaced with scenario senders inside this process only.

Rows whose production seams have not landed yet (#2498 action inventory,
#3761/#2496 incident adapters, #3726 draft adapter, #3785 evidence
denominator) are intentionally absent here; the rehearsal harness records
them as pending with their owners.
"""

from __future__ import annotations

import argparse
import contextlib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parent.parent
CORPUS_SCHEMA = "cargo-allow.release-rehearsal-escaped-defect-corpus.v1"

PUBLISHER_PATH = ROOT / "scripts/release-topology-publisher.py"
CLOSEOUT_PATH = ROOT / "scripts/verify-campaign-issue-closeout.py"


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise SystemExit(f"could not load production module {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


PUBLISHER = _load("corpus_release_topology_publisher", PUBLISHER_PATH)
CLOSEOUT = _load("corpus_campaign_issue_closeout", CLOSEOUT_PATH)

TOPOLOGY_PATH = ROOT / "policy/product-package-topology-v2.toml"
DIGEST_A = "sha256:" + "a" * 64
DIGEST_B = "sha256:" + "b" * 64
DIGEST_C = "sha256:" + "c" * 64
DIGEST_E = "sha256:" + "e" * 64
COMMIT = "a" * 40
TREE = "b" * 40
AUTHORIZATION = "sha256:" + "c" * 64


@contextlib.contextmanager
def patched(module, **replacements):
    """Replace module attributes for one scenario and always restore them."""
    saved = {name: getattr(module, name) for name in replacements}
    for name, value in replacements.items():
        setattr(module, name, value)
    try:
        yield
    finally:
        for name, value in saved.items():
            setattr(module, name, value)


class Rejected(Exception):
    """One fixture could not prove its historical rejection."""


def expect_system_exit(operation, fragment: str) -> str:
    """Run one production operation that must fail with the exact law message."""
    try:
        operation()
    except SystemExit as exit:
        message = exit.code if isinstance(exit.code, str) else str(exit.code)
        if fragment not in message:
            raise Rejected(
                f"production rejection message {message!r} does not name {fragment!r}"
            ) from None
        return message
    raise Rejected(f"production accepted the escaped input; expected {fragment!r}")


def selected_rows():
    topology, rows = PUBLISHER.load_rows(TOPOLOGY_PATH, "cargo-allow")
    return topology, rows


def synthetic_packages(rows):
    return {
        row["cargo_package_name"]: {
            "name": row["cargo_package_name"],
            "version": row["package_version"],
            "dependencies": [],
        }
        for row in rows
    }


def fake_crate(row):
    """A never-created archive path inside the ignored target tree."""
    name = row["cargo_package_name"]
    version = row["package_version"]
    return ROOT / "target/package" / f"{name}-{version}.crate"


def drive_publisher(args):
    """Run the real publisher selection under one selected invocation scope."""
    with PUBLISHER._invocation_scope(args) as invocation:
        return PUBLISHER._main_selected(args, invocation)


def fixture_shared_repackage_registry_retained() -> str:
    """RC1-ESCAPE-003: a repackaged local archive differs from the retained
    namespace bytes while the registry checksum still equals the topology's
    retained expectation. The production preflight must decide on
    registry-versus-retained evidence and admit every shared row exactly."""
    topology, rows = selected_rows()
    shared = [row for row in rows if row["product_family"] == "shared"]
    if len(shared) != 3:
        raise Rejected(f"topology selected {len(shared)} shared rows, expected 3")
    expected_by_name = {}
    for row in shared:
        expected = row.get("expected_registry_checksum")
        if not isinstance(expected, str) or not expected.startswith("sha256:"):
            raise Rejected(f"shared row {row['cargo_package_name']} lacks retained evidence")
        if expected == DIGEST_A:
            raise Rejected("retained evidence collides with the synthetic repackage digest")
        expected_by_name[row["cargo_package_name"]] = expected

    def registry(name, version):
        return expected_by_name[name]

    receipt: dict = {}
    with tempfile.TemporaryDirectory(prefix="corpus-003-") as directory:
        receipt_path = Path(directory) / "preflight-receipt.json"
        with patched(
            PUBLISHER,
            package_crate=lambda name, version: (fake_crate(
                {"cargo_package_name": name, "package_version": version}), DIGEST_A),
            registry_checksum=registry,
        ):
            PUBLISHER.shared_registry_preflight(
                rows, publish=True, receipt=receipt, receipt_path=receipt_path
            )
    evidence = receipt.get("shared_registry_preflight", [])
    if len(evidence) != 3 or any(
        row["state"] != "already_published_exact" for row in evidence
    ):
        raise Rejected(f"preflight did not admit the exact shared rows: {evidence}")
    if receipt.get("shared_registry_preflight_complete") is not True:
        raise Rejected("preflight did not complete after exact shared evidence")
    return "3 shared rows admitted exactly despite local repackage drift"


def fixture_shared_checksum_conflict() -> str:
    """RC1-ESCAPE-004: a shared row's registry checksum conflicts with the
    retained expectation. The production preflight must record the conflict
    and block a publishing run before the first upload."""
    topology, rows = selected_rows()

    def registry(name, version):
        return DIGEST_E

    receipt: dict = {}
    with tempfile.TemporaryDirectory(prefix="corpus-004-") as directory:
        receipt_path = Path(directory) / "preflight-receipt.json"
        with patched(
            PUBLISHER,
            package_crate=lambda name, version: (fake_crate(
                {"cargo_package_name": name, "package_version": version}), DIGEST_A),
            registry_checksum=registry,
        ):
            message = expect_system_exit(
                lambda: PUBLISHER.shared_registry_preflight(
                    rows, publish=True, receipt=receipt, receipt_path=receipt_path
                ),
                "shared registry preflight blocked before upload",
            )
    evidence = receipt.get("shared_registry_preflight", [])
    if not evidence or any(row["state"] != "checksum_conflict" for row in evidence):
        raise Rejected(f"preflight did not record the conflict: {evidence}")
    if receipt.get("incident_state") != "release_incident":
        raise Rejected("conflicting shared evidence did not raise a release incident")
    return f"conflict blocked before upload: {message[:80]}"


def _publish_loop_scenario(*, publish, shared_answer, cargo_answers):
    """Drive the real publisher selection with one synthetic provider script.

    The real selection first runs the three shared-row preflight observations
    (``shared_answer`` supplies each retained-expectation comparison), then
    walks the cargo-allow rows where ``cargo_answers`` supplies each scenario
    observation in order. ``events`` records the process calls the publisher
    selected, so the fixture can prove no upload path escaped the scenario
    and that the token only ever reached the synthetic sender.
    """
    _, rows = selected_rows()
    packages = synthetic_packages(rows)
    events: list = []
    consumed: list = []
    directory = tempfile.TemporaryDirectory(prefix="corpus-publish-")
    receipt_path = Path(directory.name) / "publish-receipt.json"

    def package(selected, selected_packages):
        events.append(("package", sorted(selected)))

    def crate(name, version):
        row = next(row for row in rows if row["cargo_package_name"] == name)
        return fake_crate(row), DIGEST_A

    by_name = {row["cargo_package_name"]: row for row in rows}

    def registry(name, version):
        row = by_name[name]
        if row["product_family"] == "shared":
            return shared_answer(row)
        consumed.append(name)
        return cargo_answers[len(consumed) - 1]

    def run(command, *, env=None):
        if command[:2] == ["cargo", "publish"] and "--dry-run" not in command:
            if env is None:
                raise Rejected("upload attempted without an explicit child environment")
            events.append(("upload", command[3], bool(env.get("CARGO_REGISTRY_TOKEN"))))
        else:
            events.append(("dry-run", tuple(command[:2])))
        return ""

    args = SimpleNamespace(
        topology=TOPOLOGY_PATH, receipt=receipt_path, mode="cargo-allow",
        list=False, authorization=AUTHORIZATION if publish else "", publish=publish,
        package_only=False, registry_preflight=False, recovery_receipt=None,
        publication_receipt=None,
    )
    synthetic_token = "corpus-synthetic-token"
    with patched(
        PUBLISHER,
        cargo_packages=lambda: packages,
        package_workspace=package,
        package_crate=crate,
        registry_checksum=registry,
        git_identity=lambda kind: {"commit": COMMIT, "tree": TREE}[kind],
        run=run,
        time=SimpleNamespace(sleep=lambda seconds: events.append(("sleep", seconds))),
    ), patched(os, environ={**os.environ, "CARGO_REGISTRY_TOKEN": synthetic_token}):
        try:
            drive_publisher(args)
        except SystemExit as exit:
            message = exit.code if isinstance(exit.code, str) else str(exit.code)
            events.append(("exit", message))
    receipt_text = receipt_path.read_text(encoding="utf-8")
    directory.cleanup()
    receipt = json.loads(receipt_text)
    if synthetic_token in receipt_text:
        raise Rejected("the synthetic fixture token leaked into the receipt")
    return receipt, events


def fixture_existing_row_checksum_drift() -> str:
    """RC1-ESCAPE-005: an existing cargo-allow registry row carries a wrong
    checksum. The production selection must classify checksum_conflict, raise
    a release incident, and stop before any upload for that row."""
    receipt, events = _publish_loop_scenario(
        publish=False, shared_answer=lambda row: DIGEST_B,
        cargo_answers=[DIGEST_E],
    )
    rows = receipt.get("rows", [])
    if len(rows) != 1 or rows[0]["state"] != "checksum_conflict":
        raise Rejected(f"existing-row drift was not classified: {rows}")
    if receipt.get("incident_state") != "release_incident":
        raise Rejected("existing-row drift did not raise a release incident")
    if any(event[0] in {"upload", "dry-run"} for event in events):
        raise Rejected(f"an upload path ran before the conflict stopped the row: {events}")
    if not any(event[0] == "exit" and "existing registry checksum conflict" in event[1]
               for event in events):
        raise Rejected(f"the exact existing-row rejection was not recorded: {events}")
    return "existing row drift blocked before any upload"


def fixture_post_upload_checksum_conflict() -> str:
    """RC1-ESCAPE-006: a freshly uploaded row becomes visible with a
    conflicting checksum. The production visibility/equality loop must
    classify the conflict, retain the incident as partial, and stop before
    the next row's upload."""
    def retained(row):
        expected = row.get("expected_registry_checksum")
        if not isinstance(expected, str) or not expected.startswith("sha256:"):
            raise Rejected(f"shared row {row['cargo_package_name']} lacks retained evidence")
        return expected

    receipt, events = _publish_loop_scenario(
        publish=True, shared_answer=retained, cargo_answers=[None, DIGEST_E],
    )
    rows = receipt.get("rows", [])
    if len(rows) != 1 or rows[0]["state"] != "checksum_conflict":
        raise Rejected(f"post-upload conflict was not classified: {rows}")
    if receipt.get("incident_state") != "partial":
        raise Rejected("post-upload conflict did not retain a partial incident")
    if receipt.get("first_irreversible_row") is None:
        raise Rejected("the irreversible boundary was not retained for recovery custody")
    uploads = [event for event in events if event[0] == "upload"]
    if len(uploads) != 1 or uploads[0][2] is not True:
        raise Rejected(f"the upload scenario did not observe one tokened sender: {uploads}")
    if not any(event[0] == "exit" and "newly published registry checksum conflict" in event[1]
               for event in events):
        raise Rejected(f"the exact post-upload rejection was not recorded: {events}")
    return "post-upload conflict retained a partial incident and stopped row 2+"


def _recovery_receipt_file(directory: Path, **overrides):
    topology, _ = selected_rows()
    receipt = {
        "schema_id": "cargo-allow.topology-publish-receipt.v1",
        "schema_version": 1,
        "mode": "cargo-allow",
        "publish": True,
        "authorization": AUTHORIZATION,
        "topology_id": topology["topology_id"],
        "topology_sha256": PUBLISHER.sha256_text(TOPOLOGY_PATH),
        "cargo_lock_sha256": PUBLISHER.sha256_text(ROOT / "Cargo.lock"),
        "commit": COMMIT,
        "tree": TREE,
        "complete": False,
        "incident_state": "partial",
        "rows": [{
            "name": "allow-core", "version": "0.2.0",
            "local_checksum": DIGEST_A, "registry_checksum": DIGEST_B,
            "state": "published_verified",
        }],
    }
    receipt.update(overrides)
    path = directory / "recovery-receipt.json"
    path.write_text(json.dumps(receipt), encoding="utf-8")
    return path


def fixture_recovery_custody_exact_candidate() -> str:
    """RC1-ESCAPE-007: a partial run must only resume against the exact
    original candidate. The production recovery loader must admit the exact
    receipt and reject commit (moved main), tree, authorization, posture,
    schema, and missing-incident drift with the exact law messages."""
    topology, _ = selected_rows()

    def load(path, **overrides):
        return PUBLISHER.load_recovery_receipt(
            path, mode="cargo-allow", topology=topology, topology_path=TOPOLOGY_PATH,
            authorization=AUTHORIZATION,
        )

    with patched(PUBLISHER, git_identity=lambda kind: {"commit": COMMIT, "tree": TREE}[kind]):
        with tempfile.TemporaryDirectory(prefix="corpus-007-") as directory:
            exact = _recovery_receipt_file(Path(directory))
            if load(exact).get("incident_state") != "partial":
                raise Rejected("the exact incident receipt was not admitted")
            moving_main = _recovery_receipt_file(Path(directory), commit="c" * 40)
            expect_system_exit(
                lambda: load(moving_main),
                "recovery receipt commit differs from the exact candidate",
            )
            rebuilt_tree = _recovery_receipt_file(Path(directory), tree="d" * 40)
            expect_system_exit(
                lambda: load(rebuilt_tree),
                "recovery receipt tree differs from the exact candidate",
            )
            other_authority = _recovery_receipt_file(
                Path(directory), authorization="sha256:" + "d" * 64,
            )
            expect_system_exit(
                lambda: load(other_authority),
                "recovery receipt authorization differs from the selected authorization",
            )
            finished = _recovery_receipt_file(Path(directory), complete=True)
            expect_system_exit(
                lambda: load(finished),
                "recovery requires an incomplete incident receipt",
            )
            clean = _recovery_receipt_file(Path(directory), incident_state="none")
            expect_system_exit(
                lambda: load(clean),
                "recovery receipt does not preserve a publish incident",
            )
            foreign = _recovery_receipt_file(
                Path(directory), schema_id="cargo-allow.other-receipt.v1",
            )
            expect_system_exit(
                lambda: load(foreign),
                "recovery receipt has an unexpected schema",
            )
    return "recovery admitted only the exact original candidate receipt"


def fixture_canonical_checksum_encoding() -> str:
    """RC1-ESCAPE-010: topology receipts must carry the canonical
    sha256:<64 lowercase hex> encoding. The production canonical-form
    authority must reject uppercase, short, prefixed-wrong, and non-hex
    encodings and accept only the canonical form."""
    canonical = PUBLISHER.canonical_receipt_checksum(DIGEST_C, "fixture checksum")
    if canonical != DIGEST_C:
        raise Rejected("the canonical form was not preserved verbatim")
    for malformed in (
        "SHA256:" + "c" * 64, "c" * 64, "sha999:" + "c" * 64,
    ):
        expect_system_exit(
            lambda value=malformed: PUBLISHER.canonical_receipt_checksum(
                value, "fixture checksum"),
            "canonical sha256:",
        )
    for malformed_digest in (
        "sha256:" + "C" * 64, "sha256:" + "c" * 63, "sha256:" + "g" * 64,
        DIGEST_C + " ",
    ):
        expect_system_exit(
            lambda value=malformed_digest: PUBLISHER.canonical_receipt_checksum(
                value, "fixture checksum"),
            "exactly 64 lowercase hexadecimal",
        )
    return "noncanonical encodings rejected at the production checksum authority"


def fixture_issue_closed_from_narration() -> str:
    """RC1-ESCAPE-016: a campaign issue must not close on narration. The
    production closeout checker must reject a missing closeout payload and a
    Complete closeout whose acceptance backing carries no sufficient evidence
    class, while admitting a typed-evidence closeout."""
    if CLOSEOUT.validate_closeout(None, 3793, {"Complete"}) != ["missing_closeout"]:
        raise Rejected("a narration-only close was not rejected")
    if CLOSEOUT.closeout_from_body("narration without any closeout marker") is not None:
        raise Rejected("an unmarked body produced a closeout payload")
    payload = {
        "schema_id": CLOSEOUT.SCHEMA, "issue": 3793, "result": "Complete",
        "merged_pr": 3946, "closeout_id": "corpus-narration-control",
        "evidence_surfaces": ["narration-only-surface"],
    }
    surfaces = {
        "narration-only-surface": "LexicalProjectionOnly",
        "typed-surface": "TypedModelValidation",
    }
    narration_only = CLOSEOUT.verify_acceptance_evidence(payload, surfaces)
    if narration_only != ["insufficient_evidence_class"]:
        raise Rejected(f"lexical-only acceptance backing was not rejected: {narration_only}")
    accepted = CLOSEOUT.validate_closeout(payload, 3793, {"Complete"})
    if accepted:
        raise Rejected(f"the well-formed closeout was rejected: {accepted}")
    typed_payload = dict(payload, evidence_surfaces=["typed-surface"])
    typed = CLOSEOUT.verify_acceptance_evidence(typed_payload, surfaces)
    if typed:
        raise Rejected(f"typed acceptance backing was rejected: {typed}")
    return "narration closeout rejected; typed evidence admitted"


FIXTURES = (
    ("RC1-ESCAPE-003", fixture_shared_repackage_registry_retained),
    ("RC1-ESCAPE-004", fixture_shared_checksum_conflict),
    ("RC1-ESCAPE-005", fixture_existing_row_checksum_drift),
    ("RC1-ESCAPE-006", fixture_post_upload_checksum_conflict),
    ("RC1-ESCAPE-007", fixture_recovery_custody_exact_candidate),
    ("RC1-ESCAPE-010", fixture_canonical_checksum_encoding),
    ("RC1-ESCAPE-016", fixture_issue_closed_from_narration),
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--receipt", type=Path,
                       help="write the corpus result JSON here instead of stdout only")
    args = parser.parse_args()
    rows = []
    for incident_id, fixture in FIXTURES:
        try:
            detail = fixture()
        except Rejected as rejected:
            rows.append({
                "incident_id": incident_id, "result": "rejected",
                "detail": str(rejected)[:300],
            })
        except (OSError, ValueError, KeyError, json.JSONDecodeError) as error:
            rows.append({
                "incident_id": incident_id, "result": "rejected",
                "detail": f"fixture instrument failed: {type(error).__name__}",
            })
        else:
            rows.append({
                "incident_id": incident_id, "result": "proven",
                "detail": detail[:300],
            })
    result = {
        "schema": CORPUS_SCHEMA,
        "fixture": "scripts/test-release-escaped-defect-corpus.py",
        "rows": rows,
    }
    text = json.dumps(result, indent=2, sort_keys=True)
    if args.receipt is not None:
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(text + "\n", encoding="utf-8")
    else:
        print(text)
    return 0 if all(row["result"] == "proven" for row in rows) else 1


if __name__ == "__main__":
    sys.exit(main())
