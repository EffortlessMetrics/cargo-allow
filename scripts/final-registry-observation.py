#!/usr/bin/env python3
"""Credential-free crates.io observation adapter for final registry preflight.

The adapter observes the exact cargo-allow 0.2.0 publication denominator
through public, read-only crates.io endpoints. It emits bounded observation and
evidence artifacts, never reads credentials, never starts Cargo or another
subprocess, and never performs a registry or repository mutation.

A successful exact-version response establishes version/checksum/yank state.
An exact-version 404 triggers a second public crate-name query so an absent
version remains distinct from an unavailable crate name. Owner permission and
publication authority are always reported as unproven by this public observer;
positive authority evidence belongs to a separate typed provider.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import socket
import sys
import tempfile
import time
from pathlib import Path
from typing import Any, Callable, NoReturn
from urllib.error import HTTPError, URLError
from urllib.parse import quote
from urllib.request import Request, urlopen

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - CI runs Python 3.11+
    try:
        import tomli as tomllib  # type: ignore[no-redef]
    except ModuleNotFoundError as error:
        raise SystemExit(
            "final-registry-observation requires Python 3.11+ or tomli"
        ) from error

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_TOPOLOGY = ROOT / "policy/product-package-topology-v2.toml"
API_BASE = "https://crates.io/api/v1/crates"
USER_AGENT = "cargo-allow-final-registry-observation/0.2"
TIMEOUT_SECONDS = 15
MAX_ATTEMPTS = 3
MAX_RETAINED_BODY_BYTES = 8192
CHECKSUM_PREFIX = "sha256:"
EVIDENCE_SCHEMA_ID = "cargo-allow.final-registry-observation-evidence.v1"

# Exact final denominator: (logical_id, package_name, version, product_family).
SELECTION: tuple[tuple[str, str, str, str], ...] = (
    ("allow-core", "allow-core", "0.2.0", "cargo-allow"),
    ("allow-policy", "allow-policy", "0.2.0", "cargo-allow"),
    ("allow-inventory", "allow-inventory", "0.2.0", "cargo-allow"),
    ("allow-files", "allow-files", "0.2.0", "cargo-allow"),
    ("allow-rust", "allow-rust", "0.2.0", "cargo-allow"),
    ("allow-match", "allow-match", "0.2.0", "cargo-allow"),
    ("allow-report", "allow-report", "0.2.0", "cargo-allow"),
    ("allow-policy-legacy", "allow-policy-legacy", "0.2.0", "cargo-allow"),
    ("repo-protocol", "effortless-repo-protocol", "0.1.0", "shared"),
    ("repo-snapshot", "effortless-repo-snapshot", "0.1.0", "shared"),
    ("repo-edit", "effortless-repo-edit", "0.1.0", "shared"),
    ("allow-diff", "allow-diff", "0.2.0", "cargo-allow"),
    ("cargo-allow", "cargo-allow", "0.2.0", "cargo-allow"),
)

RELEASE_ORDERS = (10, 20, 30, 40, 50, 60, 70, 75, 80, 85, 90, 95, 100)

VERSION_STATES = (
    "found",
    "missing",
    "name_unavailable",
    "visibility_pending",
    "timeout",
    "rate_limited",
    "provider_unavailable",
    "malformed_response",
)

OWNER_RULE = {
    "rule_id": "public-read-does-not-prove-owner",
    "statement": (
        "The credential-free crates.io version and crate-name endpoints do not "
        "prove current owner permission for the selected publication principal."
    ),
}
AUTHORITY_RULE = {
    "rule_id": "public-read-does-not-prove-publish-authority",
    "statement": (
        "Public registry visibility, prior publication, and package ownership do "
        "not prove that a separately selected credential may publish exact bytes."
    ),
}


def fail(message: str) -> NoReturn:
    raise SystemExit(f"final-registry-observation: error: {message}")


def now_seconds() -> int:
    return int(time.time())


def canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode("utf-8")


def digest_value(value: Any) -> str:
    return f"{CHECKSUM_PREFIX}{hashlib.sha256(canonical_json_bytes(value)).hexdigest()}"


def check_canonical_digest(value: Any) -> bool:
    return (
        isinstance(value, str)
        and value.startswith(CHECKSUM_PREFIX)
        and len(value) == len(CHECKSUM_PREFIX) + 64
        and all(char in "0123456789abcdef" for char in value[len(CHECKSUM_PREFIX) :])
    )


def load_denominator(topology_path: Path) -> list[dict[str, Any]]:
    """Load and fail-closed validate the exact 13-row final denominator."""
    if not topology_path.is_file():
        fail(f"topology file is absent: {topology_path}")
    with topology_path.open("rb") as handle:
        topology = tomllib.load(handle)
    if topology.get("topology_id") != "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001":
        fail("topology is not the selected final V2 authority")

    rows: list[dict[str, Any]] = []
    for raw in topology.get("package", []):
        if raw.get("candidate_inclusion") is not True:
            continue
        for field in (
            "logical_id",
            "cargo_package_name",
            "product_family",
            "package_version",
            "release_order",
        ):
            if field not in raw:
                fail(f"topology row is missing field {field}: {raw}")
        rows.append(dict(raw))

    rows.sort(key=lambda row: (int(row["release_order"]), row["cargo_package_name"]))
    if len(rows) != len(SELECTION):
        fail(f"denominator must hold exactly {len(SELECTION)} rows, found {len(rows)}")
    for row, (logical, package, version, family), expected_order in zip(
        rows, SELECTION, RELEASE_ORDERS
    ):
        if (
            row["logical_id"] != logical
            or row["cargo_package_name"] != package
            or row["package_version"] != version
            or row["product_family"] != family
            or int(row["release_order"]) != expected_order
        ):
            fail(f"denominator row differs from final selection: {row}")
        if family == "shared" and not check_canonical_digest(
            row.get("expected_registry_checksum")
        ):
            fail(f"shared row {package} lacks retained expected checksum evidence")

    orders = [int(row["release_order"]) for row in rows]
    if len(set(orders)) != len(orders):
        fail("denominator holds duplicate release_order values")
    return rows


def version_url(name: str, version: str) -> str:
    return f"{API_BASE}/{quote(name, safe='')}/{quote(version, safe='')}"


def crate_url(name: str) -> str:
    return f"{API_BASE}/{quote(name, safe='')}"


def parse_version_payload(payload: Any, expected_version: str) -> dict[str, Any]:
    if not isinstance(payload, dict) or not isinstance(payload.get("version"), dict):
        raise ValueError("missing_version_object")
    version = payload["version"]
    if version.get("num") != expected_version:
        raise ValueError("unexpected_version")
    checksum = version.get("checksum")
    if (
        not isinstance(checksum, str)
        or len(checksum) != 64
        or any(char not in "0123456789abcdef" for char in checksum)
    ):
        raise ValueError("malformed_checksum")
    yanked = version.get("yanked")
    if not isinstance(yanked, bool):
        raise ValueError("malformed_yank_state")
    return {
        "num": expected_version,
        "checksum": f"{CHECKSUM_PREFIX}{checksum}",
        "yanked": yanked,
    }


def parse_crate_payload(payload: Any, expected_name: str) -> dict[str, Any]:
    if not isinstance(payload, dict) or not isinstance(payload.get("crate"), dict):
        raise ValueError("missing_crate_object")
    crate = payload["crate"]
    observed_name = crate.get("id")
    if observed_name is None:
        observed_name = crate.get("name")
    if observed_name != expected_name:
        raise ValueError("unexpected_crate_name")
    return {"id": expected_name}


def _is_timeout(error: BaseException) -> bool:
    if isinstance(error, (socket.timeout, TimeoutError)):
        return True
    return isinstance(error, URLError) and isinstance(
        error.reason, (socket.timeout, TimeoutError)
    )


def fetch_endpoint(
    url: str,
    project: Callable[[Any], dict[str, Any]],
) -> dict[str, Any]:
    """Perform one bounded public GET and retain only a canonical projection."""
    request = Request(url, headers={"User-Agent": USER_AGENT})
    attempts = 0
    while attempts < MAX_ATTEMPTS:
        attempts += 1
        try:
            with urlopen(request, timeout=TIMEOUT_SECONDS) as response:
                body = response.read(MAX_RETAINED_BODY_BYTES + 1)
        except HTTPError as error:
            if error.code == 404:
                return {
                    "url": url,
                    "outcome": "not_found",
                    "http_status": 404,
                    "attempts": attempts,
                }
            if error.code == 429:
                if attempts == MAX_ATTEMPTS:
                    return {
                        "url": url,
                        "outcome": "rate_limited",
                        "http_status": 429,
                        "attempts": attempts,
                    }
            elif 500 <= error.code <= 599:
                if attempts == MAX_ATTEMPTS:
                    return {
                        "url": url,
                        "outcome": "provider_unavailable",
                        "http_status": error.code,
                        "attempts": attempts,
                    }
            else:
                return {
                    "url": url,
                    "outcome": "provider_unavailable",
                    "http_status": error.code,
                    "attempts": attempts,
                }
            print(
                f"transient crates.io HTTP {error.code}; retrying "
                f"({attempts}/{MAX_ATTEMPTS})",
                file=sys.stderr,
            )
            time.sleep(attempts * 2)
            continue
        except (URLError, socket.timeout, TimeoutError, ConnectionError) as error:
            if attempts == MAX_ATTEMPTS:
                return {
                    "url": url,
                    "outcome": "timeout" if _is_timeout(error) else "provider_unavailable",
                    "http_status": None,
                    "attempts": attempts,
                }
            print(
                f"transient crates.io connection failure; retrying "
                f"({attempts}/{MAX_ATTEMPTS})",
                file=sys.stderr,
            )
            time.sleep(attempts * 2)
            continue

        if len(body) > MAX_RETAINED_BODY_BYTES:
            return {
                "url": url,
                "outcome": "malformed_response",
                "http_status": 200,
                "attempts": attempts,
                "reason": "body_too_large",
            }
        try:
            payload = json.loads(body.decode("utf-8"))
            projection = project(payload)
        except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
            reason = str(error) if isinstance(error, ValueError) else "invalid_json"
            return {
                "url": url,
                "outcome": "malformed_response",
                "http_status": 200,
                "attempts": attempts,
                "reason": reason,
            }
        return {
            "url": url,
            "outcome": "found",
            "http_status": 200,
            "attempts": attempts,
            "projection": projection,
        }
    fail(f"crates.io lookup exhausted retries: {url}")


def observe_version(name: str, version: str) -> tuple[dict[str, Any], dict[str, Any]]:
    """Observe exact version state and return typed response plus evidence row."""
    exact = fetch_endpoint(
        version_url(name, version),
        lambda payload: parse_version_payload(payload, version),
    )
    name_probe: dict[str, Any] | None = None
    if exact["outcome"] == "found":
        projection = exact["projection"]
        response = {
            "status": "found",
            "checksum": projection["checksum"],
            "yanked": projection["yanked"],
        }
    elif exact["outcome"] == "not_found":
        name_probe = fetch_endpoint(
            crate_url(name), lambda payload: parse_crate_payload(payload, name)
        )
        if name_probe["outcome"] == "found":
            response = {"status": "missing"}
        elif name_probe["outcome"] == "not_found":
            response = {"status": "name_unavailable"}
        else:
            response = {"status": name_probe["outcome"]}
    else:
        response = {"status": exact["outcome"]}

    evidence = {
        "package_name": name,
        "package_version": version,
        "exact_version_request": exact,
        "crate_name_request": name_probe,
        "conclusion": response,
    }
    return response, evidence


def provenance(
    provider: str, source: str, evidence: Any, observed_at: int
) -> dict[str, Any]:
    return {
        "origin": "external_provider",
        "provider": provider,
        "source": source,
        "evidence_digest": digest_value(evidence),
        "observed_at_unix_seconds": observed_at,
    }


def rule_provenance(rule: dict[str, str], observed_at: int) -> dict[str, Any]:
    return provenance(
        "cargo-allow-final-registry-observation",
        f"rule:{rule['rule_id']}",
        rule,
        observed_at,
    )


def observe_row(
    row: dict[str, Any], observed_at: int
) -> tuple[dict[str, Any], dict[str, Any]]:
    name = row["cargo_package_name"]
    version = row["package_version"]
    response, version_evidence = observe_version(name, version)
    exact_url = version_evidence["exact_version_request"]["url"]
    name_request = version_evidence["crate_name_request"]
    source = exact_url if name_request is None else f"{exact_url} + {name_request['url']}"
    observation = {
        "package_name": name,
        "package_version": version,
        "version": response,
        "version_provenance": provenance(
            "crates.io-public-api", source, version_evidence, observed_at
        ),
        "owner": "permission_not_proven",
        "owner_provenance": rule_provenance(OWNER_RULE, observed_at),
        "publish_authority": "not_proven",
        "authority_provenance": rule_provenance(AUTHORITY_RULE, observed_at),
    }
    return observation, version_evidence


def validate_extra_observations(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        fail("extra observations must be a JSON array")
    for entry in value:
        if not isinstance(entry, dict):
            fail("extra observations must hold observation objects")
        for field in ("package_name", "package_version", "version"):
            if field not in entry:
                fail(f"extra observation is missing field {field}")
        version = entry["version"]
        if not isinstance(version, dict) or version.get("status") not in VERSION_STATES:
            fail("extra observation carries an unknown version status")
    return value


def _resolved(path: Path) -> Path:
    return path.expanduser().resolve(strict=False)


def prepare_output_paths(output_paths: list[Path], input_paths: list[Path]) -> None:
    """Reject aliases and remove every prior output before observation begins."""
    resolved_outputs = [_resolved(path) for path in output_paths]
    if len(set(resolved_outputs)) != len(resolved_outputs):
        fail("output paths must be distinct")

    resolved_inputs = {_resolved(path) for path in input_paths}
    aliases = [
        path
        for path, resolved in zip(output_paths, resolved_outputs)
        if resolved in resolved_inputs
    ]
    if aliases:
        fail(
            "output path aliases an input: "
            + ", ".join(str(path) for path in aliases)
        )

    failures: list[str] = []
    for path in output_paths:
        try:
            path.unlink(missing_ok=True)
        except OSError as error:
            failures.append(f"{path}: {error}")
    if failures:
        fail("could not clear prior outputs: " + "; ".join(failures))


def clear_outputs(output_paths: list[Path]) -> None:
    """Best-effort cleanup after a staged output publication failure."""
    failures: list[str] = []
    for path in output_paths:
        try:
            path.unlink(missing_ok=True)
        except OSError as error:
            failures.append(f"{path}: {error}")
    if failures:
        print(
            "final-registry-observation: cleanup failure: " + "; ".join(failures),
            file=sys.stderr,
        )


def write_json(path: Path, payload: Any) -> None:
    """Atomically replace one UTF-8 JSON output from a same-directory stage."""
    path.parent.mkdir(parents=True, exist_ok=True)
    rendered = json.dumps(payload, indent=2) + "\n"
    stage: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            newline="\n",
            dir=path.parent,
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as handle:
            stage = Path(handle.name)
            handle.write(rendered)
            handle.flush()
        stage.replace(path)
    except OSError:
        if stage is not None:
            try:
                stage.unlink(missing_ok=True)
            except OSError:
                pass
        raise


def merge_candidate_input(
    candidate_path: Path,
    observations: list[dict[str, Any]],
    observed_at: int,
    provider_state_digest: str,
) -> dict[str, Any]:
    try:
        base = json.loads(candidate_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"could not read candidate input: {error}")
    if not isinstance(base, dict):
        fail("candidate input must be a JSON object")
    base_observations = base.get("observations")
    if not isinstance(base_observations, list) or len(base_observations) != len(SELECTION):
        fail("candidate input must carry exactly the 13 denominator observations")
    for context_name in ("observed_context", "current_context"):
        context = base.get(context_name)
        if not isinstance(context, dict):
            fail(f"candidate input is missing object {context_name}")
        context["provider_state_digest"] = provider_state_digest
    base["observations"] = observations
    base["evaluated_at_unix_seconds"] = observed_at
    return base


def summarize(
    observations: list[dict[str, Any]],
    rows: list[dict[str, Any]],
    provider_state_digest: str,
) -> str:
    lines = ["final registry observation: credential-free crates.io read-only pass"]
    for row, observation in zip(rows, observations):
        version = observation["version"]
        detail = f" yanked={version['yanked']}" if version["status"] == "found" else ""
        lines.append(
            f"  {row['cargo_package_name']} {row['package_version']}"
            f" -> {version['status']}{detail}"
            " owner=permission_not_proven authority=not_proven"
        )
    lines.append(
        f"rows={len(rows)} surplus_preserved={max(0, len(observations) - len(rows))} "
        f"provider_state={provider_state_digest}"
    )
    lines.append(
        "next=refresh-after-freeze/before-authorization/before-publish; "
        "evaluate the emitted input with the separately built Rust helper"
    )
    return "\n".join(lines)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Credential-free crates.io observer for final registry preflight."
    )
    parser.add_argument("--topology", type=Path, default=DEFAULT_TOPOLOGY)
    parser.add_argument("--observations-out", type=Path, required=True)
    parser.add_argument("--evidence-out", type=Path, required=True)
    parser.add_argument("--candidate-input", type=Path, default=None)
    parser.add_argument("--input-out", type=Path, default=None)
    parser.add_argument("--extra-observation-file", type=Path, default=None)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if (args.candidate_input is None) != (args.input_out is None):
        fail("--candidate-input and --input-out must be supplied together")

    output_paths = [args.observations_out, args.evidence_out]
    if args.input_out is not None:
        output_paths.append(args.input_out)
    input_paths = [args.topology]
    if args.candidate_input is not None:
        input_paths.append(args.candidate_input)
    if args.extra_observation_file is not None:
        input_paths.append(args.extra_observation_file)
    prepare_output_paths(output_paths, input_paths)

    rows = load_denominator(args.topology)
    observed_at = now_seconds()
    observations: list[dict[str, Any]] = []
    evidence_rows: list[dict[str, Any]] = []
    for row in rows:
        observation, evidence = observe_row(row, observed_at)
        observations.append(observation)
        evidence_rows.append(evidence)

    extras: list[dict[str, Any]] = []
    if args.extra_observation_file is not None:
        try:
            raw_extras = json.loads(
                args.extra_observation_file.read_text(encoding="utf-8")
            )
        except (OSError, json.JSONDecodeError) as error:
            fail(f"could not read extra observations: {error}")
        extras = validate_extra_observations(raw_extras)
        observations.extend(extras)

    provider_payload = {
        "observed_at_unix_seconds": observed_at,
        "rows": evidence_rows,
        "limitation_rules": [OWNER_RULE, AUTHORITY_RULE],
        "extra_observations_digest": digest_value(extras),
    }
    provider_state_digest = digest_value(provider_payload)
    evidence_artifact = {
        "schema_id": EVIDENCE_SCHEMA_ID,
        "schema_version": 1,
        "provider_state_digest": provider_state_digest,
        **provider_payload,
    }

    merged: dict[str, Any] | None = None
    if args.candidate_input is not None:
        merged = merge_candidate_input(
            args.candidate_input,
            observations,
            observed_at,
            provider_state_digest,
        )

    try:
        write_json(args.evidence_out, evidence_artifact)
        write_json(args.observations_out, observations)
        if args.input_out is not None and merged is not None:
            # The evaluator input is intentionally published last. It cannot
            # exist unless all supporting artifacts were written successfully.
            write_json(args.input_out, merged)
    except BaseException:
        clear_outputs(output_paths)
        raise

    print(summarize(observations, rows, provider_state_digest))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
