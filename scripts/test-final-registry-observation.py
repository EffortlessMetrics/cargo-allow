#!/usr/bin/env python3
"""Deterministic contract tests for the #3850 credential-free observer.

Every test stubs the transport: no test contacts crates.io, reads a
credential, or performs a release mutation. The matrix proves that 404s,
malformed responses, rate limits, timeouts, and propagation-delay hints stay
distinct, that surplus observations are preserved in order, and that the
adapter is structurally incapable of credential use.
"""

from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import socket
import sys
from contextlib import contextmanager, redirect_stderr
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


def version_payload(
    name: str, version: str, checksum: str = "a" * 64, yanked: bool = False
) -> bytes:
    return json.dumps(
        {"version": {"num": version, "checksum": checksum, "yanked": yanked}}
    ).encode("utf-8")


@contextmanager
def stub_urlopen(handler: Callable[[Any], Any]):
    original = ADAPTER.urlopen
    ADAPTER.urlopen = handler
    try:
        yield handler
    finally:
        ADAPTER.urlopen = original


@contextmanager
def capture_stderr():
    buffer = io.StringIO()
    with redirect_stderr(buffer):
        yield buffer


def denominator_row(name: str, version: str) -> dict[str, Any]:
    return {"cargo_package_name": name, "package_version": version}


def test_denominator_matches_final_selection() -> None:
    rows = ADAPTER.load_denominator(ROOT / "policy/product-package-topology-v2.toml")
    check(len(rows) == 13, "denominator must hold exactly 13 rows")
    check(
        [(r["cargo_package_name"], r["package_version"]) for r in rows]
        == [(p, v) for (_, p, v, _) in ADAPTER.SELECTION],
        "denominator order/identity differs from final selection",
    )
    shared = [r for r in rows if r["product_family"] == "shared"]
    check(len(shared) == 3, "denominator must hold three shared prerequisites")
    check(
        all(ADAPTER.check_canonical_digest(r["expected_registry_checksum"]) for r in shared),
        "shared rows must retain canonical expected checksums",
    )


def test_denominator_rejects_wrong_count(tmp: Path) -> None:
    bad = tmp / "topology.toml"
    bad.write_text(
        'topology_id = "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001"\n', encoding="utf-8"
    )
    expect_failure(
        lambda: ADAPTER.load_denominator(bad),
        "denominator with zero rows was accepted",
    )


def test_found_missing_and_malformed_stay_distinct() -> None:
    calls: list[str] = []

    def handler(request: Any, **_: Any) -> Any:
        url = request.full_url
        calls.append(url)
        if url.endswith("/allow-core/0.2.0"):
            return FakeResponse(version_payload("allow-core", "0.2.0"))
        if url.endswith("/allow-policy/0.2.0"):
            raise HTTPError(url, 404, "not found", None, None)
        return FakeResponse(b"not json{")

    with stub_urlopen(handler):
        with capture_stderr():
            kind, _body, attempts = ADAPTER.fetch_raw("allow-core", "0.2.0")
            check(kind == "found", f"200 with exact version must be found, got {kind}")
            check(attempts == 1, "clean fetch must not retry")
            kind, _body, attempts = ADAPTER.fetch_raw("allow-policy", "0.2.0")
            check(kind == "missing", f"404 must be missing, got {kind}")
            check(attempts == 1, "a 404 must never be retried into another verdict")
            kind, _body, _ = ADAPTER.fetch_raw("allow-files", "0.2.0")
            check(kind == "malformed_response", f"bad JSON must be malformed, got {kind}")
    check(len(calls) == 3, "unexpected request count for terminal outcomes")


def test_substituted_and_malformed_bodies_are_malformed() -> None:
    cases = [
        version_payload("allow-core", "0.2.0-rc.1"),  # RC substitution
        version_payload("allow-core", "0.2.0", checksum="ZZZ"),  # bad checksum
        version_payload("allow-core", "0.2.0", checksum="A" * 64),  # non-lowercase
        json.dumps({"version": {"num": "0.2.0"}}).encode(),  # missing checksum
        json.dumps(
            {"version": {"num": "0.2.0", "checksum": "b" * 64, "yanked": "no"}}
        ).encode(),  # non-bool yanked
        json.dumps({"crate": {}}).encode(),  # missing version object
    ]
    for index, body in enumerate(cases):
        verdict = ADAPTER.classify_body("allow-core", "0.2.0", body)
        check(
            verdict == {"status": "malformed_response"},
            f"case {index} must be malformed_response: {verdict}",
        )


def test_transient_failures_retry_bounded_then_stay_distinct() -> None:
    attempts: list[str] = []

    def handler(request: Any, **_: Any) -> Any:
        attempts.append(request.full_url)
        raise HTTPError(request.full_url, 429, "limited", None, None)

    with stub_urlopen(handler):
        with capture_stderr():
            ADAPTER.fetch_raw("allow-rust", "0.2.0")
    check(len(attempts) == ADAPTER.MAX_ATTEMPTS, "rate limits must exhaust retries")
    with stub_urlopen(handler):
        kind, _, _ = ADAPTER.fetch_raw("allow-rust", "0.2.0")
    check(kind == "rate_limited", f"429 must stay rate_limited, got {kind}")

    def timeout_handler(request: Any, **_: Any) -> Any:
        raise socket.timeout("timed out")

    with stub_urlopen(timeout_handler):
        kind, _, made = ADAPTER.fetch_raw("allow-rust", "0.2.0")
    check(kind == "timeout", f"timeouts must stay timeout, got {kind}")
    check(made == ADAPTER.MAX_ATTEMPTS, "timeouts must exhaust retries")
    check(kind != "missing", "a visibility timeout must never become clean absence")

    def down_handler(request: Any, **_: Any) -> Any:
        raise URLError("connection refused")

    with stub_urlopen(down_handler):
        kind, _, _ = ADAPTER.fetch_raw("allow-rust", "0.2.0")
    check(kind == "provider_unavailable", f"refusals must stay unavailable, got {kind}")

    def server_handler(request: Any, **_: Any) -> Any:
        raise HTTPError(request.full_url, 503, "down", None, None)

    with stub_urlopen(server_handler):
        kind, _, _ = ADAPTER.fetch_raw("allow-rust", "0.2.0")
    check(kind == "provider_unavailable", f"5xx must stay unavailable, got {kind}")


def test_observation_shape_and_independent_provenance() -> None:
    url = ADAPTER.version_url("allow-match", "0.2.0")
    body = version_payload("allow-match", "0.2.0", checksum="c" * 64, yanked=True)
    observation = ADAPTER.observe_row(
        denominator_row("allow-match", "0.2.0"),
        ("found", body, 1),
        owner_state="permission_not_proven",
        authority_state="not_proven",
        mark_missing_as_visibility_pending=False,
        observed_at=1_700_000_000,
    )
    check(
        observation["version"]
        == {"status": "found", "checksum": "sha256:" + "c" * 64, "yanked": True},
        "found bodies must retain checksum and yank state",
    )
    check(observation["owner"] == "permission_not_proven", "owner must default unproven")
    check(
        observation["publish_authority"] == "not_proven", "authority must default unproven"
    )
    for dimension in ("version_provenance", "owner_provenance", "authority_provenance"):
        provenance = observation[dimension]
        check(provenance["origin"] == "external_provider", "provenance must name its origin")
        check(
            ADAPTER.check_canonical_digest(provenance["evidence_digest"]),
            "provenance digests must be canonical",
        )
        check(
            provenance["observed_at_unix_seconds"] == 1_700_000_000,
            "provenance must carry observation time",
        )
    check(
        observation["version_provenance"]["source"] == url,
        "version provenance must identify the exact query URL",
    )
    check(
        observation["version_provenance"]["provider"] == "crates.io",
        "version provenance must name the provider",
    )
    expected_digest = "sha256:" + hashlib.sha256(body).hexdigest()
    check(
        observation["version_provenance"]["evidence_digest"] == expected_digest,
        "evidence digest must cover the retained bytes",
    )


def test_visibility_pending_requires_explicit_hint() -> None:
    row = denominator_row("allow-diff", "0.2.0")
    plain = ADAPTER.observe_row(
        row, ("missing", b"", 1), owner_state="permission_not_proven",
        authority_state="not_proven",
        mark_missing_as_visibility_pending=False, observed_at=10,
    )
    check(plain["version"] == {"status": "missing"}, "404 defaults to missing")
    hinted = ADAPTER.observe_row(
        row, ("missing", b"", 1), owner_state="permission_not_proven",
        authority_state="not_proven",
        mark_missing_as_visibility_pending=True, observed_at=10,
    )
    check(
        hinted["version"] == {"status": "visibility_pending"},
        "explicit upload-state hints become visibility_pending",
    )


def test_surplus_observations_preserved_in_order(tmp: Path) -> None:
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
    extra_file = tmp / "extras.json"
    extra_file.write_text(json.dumps(extras), encoding="utf-8")
    observations_out = tmp / "observations.json"
    seen = ADAPTER.validate_extra_observations(json.loads(extra_file.read_text()))
    check(seen == extras, "extra observations must validate unchanged")

    original_fetch = ADAPTER.fetch_raw
    ADAPTER.fetch_raw = lambda name, version: ("missing", b"", 1)  # type: ignore[assignment]
    try:
        code = ADAPTER.main(
            [
                "--observations-out", str(observations_out),
                "--extra-observation-file", str(extra_file),
            ]
        )
    finally:
        ADAPTER.fetch_raw = original_fetch
    check(code == 0, "denominator-only run must succeed")
    emitted = json.loads(observations_out.read_text(encoding="utf-8"))
    check(len(emitted) == 15, f"surplus must be appended, got {len(emitted)}")
    check(emitted[13:] == extras, "surplus must keep input order without normalization")


def test_retained_body_is_bounded() -> None:
    big = version_payload("allow-core", "0.2.0") + b"x" * 65536
    retained = big[: ADAPTER.MAX_RETAINED_BODY_BYTES]
    observation = ADAPTER.observe_row(
        denominator_row("allow-core", "0.2.0"),
        ("found", retained, 1), owner_state="permission_not_proven",
        authority_state="not_proven",
        mark_missing_as_visibility_pending=False, observed_at=10,
    )
    expected_digest = "sha256:" + hashlib.sha256(retained).hexdigest()
    check(
        observation["version_provenance"]["evidence_digest"] == expected_digest,
        "evidence digests must cover only the bounded retained prefix",
    )
    rendered = json.dumps(observation).encode("utf-8")
    check(len(big) > len(rendered), "unbounded response content must not reach artifacts")


def test_adapter_cannot_use_credentials() -> None:
    source = ADAPTER_PATH.read_text(encoding="utf-8")
    check("import os" not in source, "adapter must not inspect the environment")
    check("os.environ" not in source, "adapter must never read environment values")
    check("os.getenv" not in source, "adapter must never read environment values")
    check("CARGO_REGISTRY_TOKEN" not in source, "adapter must never name a token lookup")
    check("credentials" not in source.lower(), "adapter must never touch credential files")

    seen_requests: list[Any] = []
    opened: list[str] = []

    def handler(request: Any, **_: Any) -> Any:
        seen_requests.append(request)
        return FakeResponse(version_payload("allow-core", "0.2.0"))

    import builtins

    original_open = builtins.open

    def tracking_open(path: Any, *args: Any, **kwargs: Any) -> Any:
        opened.append(str(path))
        return original_open(path, *args, **kwargs)

    builtins.open = tracking_open  # type: ignore[assignment]
    try:
        with stub_urlopen(handler):
            ADAPTER.fetch_raw("allow-core", "0.2.0")
    finally:
        builtins.open = original_open
    check(seen_requests, "expected at least one stubbed request")
    for request in seen_requests:
        header_names = [name.lower() for name, _ in request.header_items()]
        check("authorization" not in header_names, "requests must carry no auth header")
        check(
            request.get_header("User-agent") == ADAPTER.USER_AGENT,
            "requests must carry only the fixed User-Agent",
        )
    statuses: set[str] = set()
    for response in (
        ("found", b"{}", 1),
        ("missing", b"", 1),
        ("timeout", b"", 3),
        ("rate_limited", b"", 3),
        ("provider_unavailable", b"", 3),
        ("malformed_response", b"bad", 1),
    ):
        observation = ADAPTER.observe_row(
            denominator_row("allow-core", "0.2.0"), response,
            owner_state="permission_not_proven", authority_state="not_proven",
            mark_missing_as_visibility_pending=False, observed_at=10,
        )
        statuses.add(observation["version"]["status"])
    check(
        statuses <= set(ADAPTER.VERSION_STATES),
        f"adapter emitted unknown version states: {statuses}",
    )
    check(
        "upload_failed" not in json.dumps(sorted(statuses)),
        "adapter cannot conclude upload failure from any outcome",
    )
    check(
        not any("credential" in path.lower() or ".cargo" in path for path in opened),
        f"adapter opened credential-adjacent paths: {opened}",
    )


def test_candidate_merge_replaces_observations_in_order(tmp: Path) -> None:
    stub_observation = {
        "package_name": "stub",
        "package_version": "0.0.0",
        "version": {"status": "missing"},
    }
    base = {
        "schema_id": "cargo-allow.final-registry-preflight.v1",
        "schema_version": 1,
        "candidate": {},
        "shared_authorities": [{}, {}, {}],
        "observed_context": {},
        "current_context": {},
        "evaluated_at_unix_seconds": 1,
        "maximum_age_seconds": 60,
        "observations": [dict(stub_observation) for _ in range(13)],
    }
    base_file = tmp / "base.json"
    base_file.write_text(json.dumps(base), encoding="utf-8")
    observations_out = tmp / "observations.json"
    input_out = tmp / "input.json"
    original_fetch = ADAPTER.fetch_raw
    ADAPTER.fetch_raw = lambda name, version: ("missing", b"", 1)  # type: ignore[assignment]
    try:
        with capture_stderr():
            code = ADAPTER.main(
                [
                    "--observations-out", str(observations_out),
                    "--candidate-input", str(base_file),
                    "--input-out", str(input_out),
                ]
            )
    finally:
        ADAPTER.fetch_raw = original_fetch
    check(code == 0, "candidate merge run must succeed")
    merged = json.loads(input_out.read_text(encoding="utf-8"))
    check(len(merged["observations"]) == 13, "merged input must keep 13 observations")
    check(
        [o["package_name"] for o in merged["observations"]]
        == [p for (_, p, _, _) in ADAPTER.SELECTION],
        "merged observations must follow denominator order with live identities",
    )
    check(
        merged["evaluated_at_unix_seconds"] != 1,
        "merge must refresh evaluation time instead of relabeling",
    )
    check(
        all(o["version_provenance"]["origin"] == "external_provider"
            for o in merged["observations"]),
        "merged observations must carry external provenance",
    )


def test_candidate_merge_rejects_wrong_denominator(tmp: Path) -> None:
    base = tmp / "base.json"
    base.write_text(json.dumps({"observations": []}), encoding="utf-8")
    observations_out = tmp / "observations.json"
    original_fetch = ADAPTER.fetch_raw
    ADAPTER.fetch_raw = lambda name, version: ("missing", b"", 1)  # type: ignore[assignment]
    try:
        expect_failure(
            lambda: ADAPTER.main(
                [
                    "--observations-out", str(observations_out),
                    "--candidate-input", str(base),
                    "--input-out", str(tmp / "input.json"),
                ]
            ),
            "candidate input with a wrong denominator was accepted",
        )
    finally:
        ADAPTER.fetch_raw = original_fetch


def main() -> int:
    tmp = Path(__file__).resolve().parent.parent / "target" / "tmp"
    tmp.mkdir(parents=True, exist_ok=True)
    test_denominator_matches_final_selection()
    test_denominator_rejects_wrong_count(tmp)
    test_found_missing_and_malformed_stay_distinct()
    test_substituted_and_malformed_bodies_are_malformed()
    test_transient_failures_retry_bounded_then_stay_distinct()
    test_observation_shape_and_independent_provenance()
    test_visibility_pending_requires_explicit_hint()
    test_surplus_observations_preserved_in_order(tmp)
    test_candidate_merge_replaces_observations_in_order(tmp)
    test_retained_body_is_bounded()
    test_adapter_cannot_use_credentials()
    test_candidate_merge_rejects_wrong_denominator(tmp)
    if FAILURES:
        print(f"{len(FAILURES)} contract failure(s)", file=sys.stderr)
        return 1
    print("final registry observation adapter: all contract tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
