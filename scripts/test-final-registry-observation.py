#!/usr/bin/env python3
"""Deterministic contract tests for the credential-free registry observer."""

from __future__ import annotations

import ast
import importlib.util
import json
import socket
import sys
import tempfile
from contextlib import contextmanager, redirect_stderr
from io import StringIO
from pathlib import Path
from typing import Any, Callable
from urllib.error import HTTPError, URLError

ROOT = Path(__file__).resolve().parent.parent
ADAPTER_PATH = ROOT / "scripts/final-registry-observation.py"
SPEC = importlib.util.spec_from_file_location("final_registry_observation", ADAPTER_PATH)
if SPEC is None or SPEC.loader is None:
    raise SystemExit("could not load final registry observation adapter")
ADAPTER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ADAPTER)

FAILURES: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        FAILURES.append(message)
        print(f"FAIL: {message}")


def expect_failure(action: Callable[[], Any], message: str) -> None:
    try:
        action()
    except SystemExit:
        return
    FAILURES.append(message)
    print(f"FAIL: {message}")


class FakeResponse:
    def __init__(self, body: bytes) -> None:
        self._body = body

    def read(self, limit: int = -1) -> bytes:
        return self._body if limit < 0 else self._body[:limit]

    def __enter__(self) -> "FakeResponse":
        return self

    def __exit__(self, *args: Any) -> bool:
        return False


@contextmanager
def stub_transport(handler: Callable[..., Any]):
    original_open = ADAPTER.urlopen
    original_sleep = ADAPTER.time.sleep
    ADAPTER.urlopen = handler
    ADAPTER.time.sleep = lambda _seconds: None
    try:
        yield
    finally:
        ADAPTER.urlopen = original_open
        ADAPTER.time.sleep = original_sleep


@contextmanager
def quiet_stderr():
    buffer = StringIO()
    with redirect_stderr(buffer):
        yield buffer


def version_payload(version: str, checksum: str = "a" * 64, yanked: bool = False) -> bytes:
    return json.dumps(
        {"version": {"num": version, "checksum": checksum, "yanked": yanked}}
    ).encode("utf-8")


def crate_payload(name: str) -> bytes:
    return json.dumps({"crate": {"id": name}}).encode("utf-8")


def write_topology(path: Path) -> dict[str, str]:
    orders = ADAPTER.RELEASE_ORDERS
    shared_checksums: dict[str, str] = {}
    lines = ['topology_id = "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001"', ""]
    for index, ((logical, package, version, family), order) in enumerate(
        zip(ADAPTER.SELECTION, orders), start=1
    ):
        lines.extend(
            [
                "[[package]]",
                f'logical_id = "{logical}"',
                f'cargo_package_name = "{package}"',
                f'product_family = "{family}"',
                f'package_version = "{version}"',
                f"release_order = {order}",
                "candidate_inclusion = true",
            ]
        )
        if family == "shared":
            checksum = f"sha256:{index:064x}"
            shared_checksums[package] = checksum
            lines.append(f'expected_registry_checksum = "{checksum}"')
        lines.append("")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines), encoding="utf-8")
    return shared_checksums


def test_denominator_matches_final_selection() -> None:
    rows = ADAPTER.load_denominator(ROOT / "policy/product-package-topology-v2.toml")
    check(len(rows) == 13, "denominator must contain exactly 13 rows")
    check(
        [
            (
                row["logical_id"],
                row["cargo_package_name"],
                row["package_version"],
                row["product_family"],
            )
            for row in rows
        ]
        == list(ADAPTER.SELECTION),
        "topology denominator differs from the selected final rows",
    )
    check(
        [int(row["release_order"]) for row in rows] == list(ADAPTER.RELEASE_ORDERS),
        "topology release order differs from the selected final denominator",
    )
    shared = [row for row in rows if row["product_family"] == "shared"]
    check(len(shared) == 3, "denominator must retain three shared prerequisites")
    check(
        all(ADAPTER.check_canonical_digest(row["expected_registry_checksum"]) for row in shared),
        "shared prerequisite checksums must be canonical",
    )


def test_exact_found_does_not_probe_name() -> None:
    calls: list[str] = []

    def handler(request: Any, **_: Any) -> FakeResponse:
        calls.append(request.full_url)
        return FakeResponse(version_payload("0.2.0", checksum="b" * 64, yanked=True))

    with stub_transport(handler):
        response, evidence = ADAPTER.observe_version("allow-core", "0.2.0")
    check(
        response == {"status": "found", "checksum": "sha256:" + "b" * 64, "yanked": True},
        f"exact response was not retained: {response}",
    )
    check(len(calls) == 1, "an exact version response must not trigger a name probe")
    check(evidence["crate_name_request"] is None, "unexpected name-probe evidence")


def test_version_missing_and_name_unavailable_are_distinct() -> None:
    calls: list[str] = []

    def existing_name(request: Any, **_: Any) -> Any:
        calls.append(request.full_url)
        if request.full_url.endswith("/allow-policy/0.2.0"):
            raise HTTPError(request.full_url, 404, "not found", None, None)
        return FakeResponse(crate_payload("allow-policy"))

    with stub_transport(existing_name):
        response, evidence = ADAPTER.observe_version("allow-policy", "0.2.0")
    check(response == {"status": "missing"}, f"existing name must yield missing: {response}")
    check(len(calls) == 2, "missing exact version must trigger exactly one name probe")
    check(
        evidence["crate_name_request"]["outcome"] == "found",
        "name-existence evidence was not retained",
    )

    def absent_name(request: Any, **_: Any) -> Any:
        raise HTTPError(request.full_url, 404, "not found", None, None)

    with stub_transport(absent_name):
        response, evidence = ADAPTER.observe_version("never-published", "0.2.0")
    check(
        response == {"status": "name_unavailable"},
        f"double 404 must stay name_unavailable: {response}",
    )
    check(
        evidence["crate_name_request"]["outcome"] == "not_found",
        "name 404 evidence was not retained",
    )


def test_name_probe_failures_never_become_clean_absence() -> None:
    scenarios: list[tuple[str, Callable[..., Any], str]] = []

    def rate_limited(request: Any, **_: Any) -> Any:
        if request.full_url.endswith("/0.2.0"):
            raise HTTPError(request.full_url, 404, "not found", None, None)
        raise HTTPError(request.full_url, 429, "limited", None, None)

    scenarios.append(("rate limit", rate_limited, "rate_limited"))

    def timed_out(request: Any, **_: Any) -> Any:
        if request.full_url.endswith("/0.2.0"):
            raise HTTPError(request.full_url, 404, "not found", None, None)
        raise socket.timeout("timed out")

    scenarios.append(("timeout", timed_out, "timeout"))

    def malformed(request: Any, **_: Any) -> Any:
        if request.full_url.endswith("/0.2.0"):
            raise HTTPError(request.full_url, 404, "not found", None, None)
        return FakeResponse(b"not-json")

    scenarios.append(("malformed response", malformed, "malformed_response"))

    for label, handler, expected in scenarios:
        with stub_transport(handler), quiet_stderr():
            response, _evidence = ADAPTER.observe_version("allow-files", "0.2.0")
        check(
            response == {"status": expected},
            f"{label} became clean absence: {response}",
        )


def test_version_body_validation_and_bounding() -> None:
    cases = [
        version_payload("0.2.0-rc.1"),
        version_payload("0.2.0", checksum="A" * 64),
        json.dumps({"version": {"num": "0.2.0"}}).encode("utf-8"),
        json.dumps(
            {"version": {"num": "0.2.0", "checksum": "a" * 64, "yanked": "no"}}
        ).encode("utf-8"),
    ]
    for index, body in enumerate(cases):
        with stub_transport(lambda _request, body=body, **_: FakeResponse(body)):
            result = ADAPTER.fetch_endpoint(
                ADAPTER.version_url("allow-core", "0.2.0"),
                lambda payload: ADAPTER.parse_version_payload(payload, "0.2.0"),
            )
        check(
            result["outcome"] == "malformed_response",
            f"malformed version case {index} was accepted: {result}",
        )

    oversized = b"{" + b"x" * (ADAPTER.MAX_RETAINED_BODY_BYTES + 1)
    with stub_transport(lambda _request, **_: FakeResponse(oversized)):
        result = ADAPTER.fetch_endpoint(
            ADAPTER.version_url("allow-core", "0.2.0"),
            lambda payload: ADAPTER.parse_version_payload(payload, "0.2.0"),
        )
    check(result["outcome"] == "malformed_response", "oversized body was accepted")
    check(result.get("reason") == "body_too_large", "oversized body reason was lost")
    check("projection" not in result, "oversized response content reached retained evidence")


def test_retries_are_bounded_and_typed() -> None:
    calls = 0

    def rate_limited(request: Any, **_: Any) -> Any:
        nonlocal calls
        calls += 1
        raise HTTPError(request.full_url, 429, "limited", None, None)

    with stub_transport(rate_limited), quiet_stderr():
        result = ADAPTER.fetch_endpoint(ADAPTER.crate_url("allow-core"), lambda value: value)
    check(calls == ADAPTER.MAX_ATTEMPTS, "rate limit did not use the bounded retry count")
    check(result["outcome"] == "rate_limited", "rate limit lost its typed outcome")

    def unavailable(_request: Any, **_: Any) -> Any:
        raise URLError("connection refused")

    with stub_transport(unavailable), quiet_stderr():
        result = ADAPTER.fetch_endpoint(ADAPTER.crate_url("allow-core"), lambda value: value)
    check(
        result["outcome"] == "provider_unavailable",
        "connection refusal became another outcome",
    )


def test_observation_fixes_owner_and_authority_as_unproven() -> None:
    row = {"cargo_package_name": "allow-match", "package_version": "0.2.0"}
    with stub_transport(
        lambda _request, **_: FakeResponse(version_payload("0.2.0", checksum="c" * 64))
    ):
        observation, evidence = ADAPTER.observe_row(row, 1_700_000_000)
    check(observation["owner"] == "permission_not_proven", "owner was escalated")
    check(observation["publish_authority"] == "not_proven", "authority was escalated")
    check(
        observation["owner_provenance"]["source"]
        == "rule:public-read-does-not-prove-owner",
        "owner limitation provenance is not explicit",
    )
    check(
        observation["authority_provenance"]["source"]
        == "rule:public-read-does-not-prove-publish-authority",
        "authority limitation provenance is not explicit",
    )
    check(
        observation["version_provenance"]["evidence_digest"]
        == ADAPTER.digest_value(evidence),
        "version provenance does not bind the retained evidence row",
    )


def test_adapter_process_boundary_is_credential_free() -> None:
    source = ADAPTER_PATH.read_text(encoding="utf-8")
    tree = ast.parse(source)
    imported: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            imported.update(alias.name.split(".", 1)[0] for alias in node.names)
        elif isinstance(node, ast.ImportFrom) and node.module:
            imported.add(node.module.split(".", 1)[0])
    check("os" not in imported, "adapter imports process-environment access")
    check("subprocess" not in imported, "adapter starts a credential-inheriting subprocess")
    check("CARGO_REGISTRY_TOKEN" not in source, "adapter names a registry token")
    check("--owner-state" not in source, "caller can assert owner state without evidence")
    check("--authority-state" not in source, "caller can assert authority without evidence")
    check("--receipt-out" not in source, "observer still shells into a receipt evaluator")
    check(
        "--mark-missing-as-visibility-pending" not in source,
        "unbound global visibility hint remains available",
    )

    seen: list[Any] = []

    def handler(request: Any, **_: Any) -> FakeResponse:
        seen.append(request)
        return FakeResponse(version_payload("0.2.0"))

    with stub_transport(handler):
        ADAPTER.observe_version("allow-core", "0.2.0")
    check(bool(seen), "expected one stubbed public request")
    for request in seen:
        names = {name.lower() for name, _value in request.header_items()}
        check(names == {"user-agent"}, f"unexpected request headers: {names}")


def test_main_emits_bound_evidence_and_merged_input() -> None:
    with tempfile.TemporaryDirectory() as raw_tmp:
        tmp = Path(raw_tmp)
        topology = tmp / "topology.toml"
        shared_checksums = write_topology(topology)
        observations_out = tmp / "observations.json"
        evidence_out = tmp / "evidence.json"
        candidate = tmp / "candidate.json"
        merged_out = tmp / "merged.json"
        contexts = {
            "candidate_digest": "sha256:" + "1" * 64,
            "denominator_digest": "sha256:" + "2" * 64,
            "workflow_digest": "sha256:" + "3" * 64,
            "principal": "fixture",
            "environment": "fixture",
            "owner_team_digest": "sha256:" + "4" * 64,
            "release_controls_digest": "sha256:" + "5" * 64,
            "provider_state_digest": "sha256:" + "0" * 64,
        }
        candidate.write_text(
            json.dumps(
                {
                    "observed_context": dict(contexts),
                    "current_context": dict(contexts),
                    "evaluated_at_unix_seconds": 1,
                    "observations": [{} for _ in range(13)],
                }
            ),
            encoding="utf-8",
        )

        def handler(request: Any, **_: Any) -> Any:
            url = request.full_url
            for _logical, package, version, family in ADAPTER.SELECTION:
                if url.endswith(f"/{package}/{version}"):
                    if family == "shared":
                        checksum = shared_checksums[package].removeprefix("sha256:")
                        return FakeResponse(version_payload(version, checksum=checksum))
                    raise HTTPError(url, 404, "not found", None, None)
                if url.endswith(f"/{package}"):
                    return FakeResponse(crate_payload(package))
            raise AssertionError(f"unexpected URL {url}")

        with stub_transport(handler):
            code = ADAPTER.main(
                [
                    "--topology",
                    str(topology),
                    "--observations-out",
                    str(observations_out),
                    "--evidence-out",
                    str(evidence_out),
                    "--candidate-input",
                    str(candidate),
                    "--input-out",
                    str(merged_out),
                ]
            )
        check(code == 0, "main observer run failed")
        observations = json.loads(observations_out.read_text(encoding="utf-8"))
        evidence = json.loads(evidence_out.read_text(encoding="utf-8"))
        merged = json.loads(merged_out.read_text(encoding="utf-8"))
        check(len(observations) == 13, "observer did not emit the exact denominator")
        check(
            [entry["version"]["status"] for entry in observations[:8] + observations[11:]]
            == ["missing"] * 10,
            "unpublished final rows did not remain missing",
        )
        check(
            all(entry["version"]["status"] == "found" for entry in observations[8:11]),
            "shared prerequisites were not observed exactly",
        )
        provider_digest = evidence["provider_state_digest"]
        check(ADAPTER.check_canonical_digest(provider_digest), "provider digest is malformed")
        check(
            merged["observed_context"]["provider_state_digest"] == provider_digest
            and merged["current_context"]["provider_state_digest"] == provider_digest,
            "merged input is not bound to observed provider state",
        )
        check(merged["observations"] == observations, "merged observations differ")
        check(
            len(evidence["rows"]) == 13 and len(evidence["limitation_rules"]) == 2,
            "retained evidence artifact is incomplete",
        )


def test_extra_observations_are_preserved_in_order() -> None:
    extras = [
        {
            "package_name": "unexpected-crate",
            "package_version": "9.9.9",
            "version": {"status": "missing"},
        },
        {
            "package_name": "allow-core",
            "package_version": "0.2.0",
            "version": {"status": "malformed_response"},
        },
    ]
    check(
        ADAPTER.validate_extra_observations(json.loads(json.dumps(extras))) == extras,
        "surplus observations were reordered or normalized",
    )


def test_candidate_pairing_and_shape_fail_closed() -> None:
    with tempfile.TemporaryDirectory() as raw_tmp:
        tmp = Path(raw_tmp)
        candidate = tmp / "candidate.json"
        candidate.write_text(json.dumps({"observations": []}), encoding="utf-8")
        expect_failure(
            lambda: ADAPTER.merge_candidate_input(
                candidate, [], 100, "sha256:" + "1" * 64
            ),
            "candidate with wrong denominator was accepted",
        )
        parser = ADAPTER.build_parser()
        parsed = parser.parse_args(
            [
                "--observations-out",
                str(tmp / "observations.json"),
                "--evidence-out",
                str(tmp / "evidence.json"),
            ]
        )
        check(parsed.candidate_input is None and parsed.input_out is None, "unexpected pairing")


def main() -> int:
    test_denominator_matches_final_selection()
    test_exact_found_does_not_probe_name()
    test_version_missing_and_name_unavailable_are_distinct()
    test_name_probe_failures_never_become_clean_absence()
    test_version_body_validation_and_bounding()
    test_retries_are_bounded_and_typed()
    test_observation_fixes_owner_and_authority_as_unproven()
    test_adapter_process_boundary_is_credential_free()
    test_main_emits_bound_evidence_and_merged_input()
    test_extra_observations_are_preserved_in_order()
    test_candidate_pairing_and_shape_fail_closed()
    if FAILURES:
        print(f"{len(FAILURES)} contract failure(s)", file=sys.stderr)
        return 1
    print("final registry observation adapter: all contract tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
