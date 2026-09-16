#!/usr/bin/env python3
"""Credential-free crates.io observation adapter for the final registry preflight.

Issue #3850 consumes the `CargoAllowFinalRegistryPreflightV1` contract from
#3849/#4270. This script is the production observation path: it queries the
exact ten final `0.2.0` upload rows and three shared `0.1.0` prerequisites
through the public read-only crates.io API, normalizes each provider response
into the typed observation shape the production evaluator owns, and preserves
missing, malformed, duplicated, and surplus observations instead of
normalizing unexpected evidence away.

Credential and authority law (enforced by construction, not by convention):

* Only stdlib `urllib` GET requests with a fixed User-Agent are issued. No
  `Authorization` header is ever set, no release token is ever read, no
  credential file is ever opened, and the process environment is never
  inspected (`os` is not imported).
* Public version/checksum observation is credential-free. Owner state
  defaults to `permission_not_proven` and publication authority defaults to
  `not_proven`: a public read cannot prove permission, so residual authority
  risk stays visible to #2501/#3789 instead of becoming clean permission.
* Repository-secret availability is never consulted per package. Prior
  publication or owner membership, when explicitly selected by the caller,
  is recorded as `supporting_evidence_only`, never `proven`.
* Nothing here packages, uploads, yanks, mutates owners, creates tags, reads
  a release token, authorizes publication, or touches GitHub state.

Visibility-delay law: a 404 is `missing`, never an upload failure. Only an
explicit caller-supplied upload-state hint (`--mark-missing-as-visibility-pending`)
may reclassify a missing row as `visibility_pending`, and the adapter can
never conclude `UploadFailed` from a visibility timeout: no such response
variant exists in the contract it emits.

Each observation carries independent version, owner, and authority provenance
(provider, source URL or rule token, SHA-256 of the bounded retained bytes,
observation time, `external_provider` origin) so the #4270 handoff
requirement holds: independent provider provenance is preserved and surplus
observations travel in `surplus_observations` without establishing selected
package authority.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any, NoReturn
from urllib.error import HTTPError, URLError
from urllib.parse import quote
from urllib.request import Request, urlopen

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - CI runs 3.11+
    try:
        import tomli as tomllib  # type: ignore[no-redef]
    except ModuleNotFoundError as error:
        raise SystemExit(
            "final-registry-observation requires Python 3.11+ or the tomli package"
        ) from error

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_TOPOLOGY = ROOT / "policy/product-package-topology-v2.toml"
API_BASE = "https://crates.io/api/v1/crates"
USER_AGENT = "cargo-allow-final-registry-observation/0.2"
TIMEOUT_SECONDS = 15
MAX_ATTEMPTS = 3
MAX_RETAINED_BODY_BYTES = 8192
CHECKSUM_PREFIX = "sha256:"

# The exact final denominator: (logical_id, cargo_package_name, version, family).
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
OWNER_STATES = (
    "owned_by_expected_principal",
    "unexpected_owner",
    "permission_not_proven",
    "provider_unavailable",
)
AUTHORITY_STATES = ("not_proven", "supporting_evidence_only")


def fail(message: str) -> NoReturn:
    raise SystemExit(f"final-registry-observation: error: {message}")


def now_seconds() -> int:
    return int(time.time())


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical_digest(hex_value: str, field: str) -> str:
    if (
        not isinstance(hex_value, str)
        or len(hex_value) != 64
        or any(c not in "0123456789abcdef" for c in hex_value)
    ):
        fail(f"{field} must be exactly 64 lowercase hexadecimal characters")
    return f"{CHECKSUM_PREFIX}{hex_value}"


def check_canonical_digest(value: Any) -> bool:
    return (
        isinstance(value, str)
        and value.startswith(CHECKSUM_PREFIX)
        and len(value) == len(CHECKSUM_PREFIX) + 64
        and all(c in "0123456789abcdef" for c in value[len(CHECKSUM_PREFIX):])
    )


def load_denominator(topology_path: Path) -> list[dict[str, Any]]:
    """Load and fail-closed-validate the exact 13-row final denominator."""
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
    for row, (logical, package, version, family) in zip(rows, SELECTION):
        if (
            row["logical_id"] != logical
            or row["cargo_package_name"] != package
            or row["package_version"] != version
            or row["product_family"] != family
        ):
            fail(f"denominator row differs from final selection: {row}")
        if family == "shared":
            expected = row.get("expected_registry_checksum")
            if not check_canonical_digest(expected):
                fail(f"shared row {package} lacks retained expected checksum evidence")
    orders = [int(row["release_order"]) for row in rows]
    if len(set(orders)) != len(orders):
        fail("denominator holds duplicate release_order values")
    return rows


def version_url(name: str, version: str) -> str:
    return f"{API_BASE}/{quote(name, safe='')}/{quote(version, safe='')}"


def classify_body(name: str, version: str, body: bytes) -> dict[str, Any]:
    """Classify one HTTP 200 body without retaining it beyond evidence digest."""
    try:
        payload = json.loads(body.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return {"status": "malformed_response"}
    if not isinstance(payload, dict):
        return {"status": "malformed_response"}
    version_payload = payload.get("version")
    if not isinstance(version_payload, dict):
        return {"status": "malformed_response"}
    observed_version = version_payload.get("num")
    if observed_version != version:
        # An RC or otherwise substituted version response is never a
        # substitute for the requested exact-version query.
        return {"status": "malformed_response"}
    checksum = version_payload.get("checksum")
    if (
        not isinstance(checksum, str)
        or len(checksum) != 64
        or any(c not in "0123456789abcdef" for c in checksum)
    ):
        return {"status": "malformed_response"}
    yanked = version_payload.get("yanked")
    if not isinstance(yanked, bool):
        return {"status": "malformed_response"}
    return {
        "status": "found",
        "checksum": f"{CHECKSUM_PREFIX}{checksum}",
        "yanked": yanked,
    }


def fetch_raw(name: str, version: str) -> tuple[str, bytes, int]:
    """Bounded credential-free fetch.

    Returns (kind, retained_body, attempts) where kind is one of found,
    missing, rate_limited, provider_unavailable, timeout, malformed_response.
    Only 429/5xx/timeout/connection failures retry; every other outcome is
    terminal on first observation so ordinary propagation delay (404) can
    never be retried into a different verdict.
    """
    url = version_url(name, version)
    request = Request(url, headers={"User-Agent": USER_AGENT})
    attempts = 0
    while attempts < MAX_ATTEMPTS:
        attempts += 1
        try:
            with urlopen(request, timeout=TIMEOUT_SECONDS) as response:
                body = response.read(MAX_RETAINED_BODY_BYTES + 1)
        except HTTPError as error:
            if error.code == 404:
                return ("missing", b"", attempts)
            if error.code == 429:
                if attempts == MAX_ATTEMPTS:
                    return ("rate_limited", b"", attempts)
            elif 500 <= error.code <= 599:
                if attempts == MAX_ATTEMPTS:
                    return ("provider_unavailable", b"", attempts)
            else:
                # Any other HTTP status is provider behavior the contract
                # cannot interpret as version evidence; it must not become
                # clean absence.
                return ("provider_unavailable", b"", attempts)
            print(
                f"transient crates.io HTTP {error.code} for {name} {version}; "
                f"retrying ({attempts}/{MAX_ATTEMPTS})",
                file=sys.stderr,
            )
            time.sleep(attempts * 2)
            continue
        except (URLError, socket.timeout, TimeoutError, ConnectionError) as error:
            if attempts == MAX_ATTEMPTS:
                if isinstance(error, socket.timeout | TimeoutError) or (
                    isinstance(error, URLError)
                    and isinstance(error.reason, socket.timeout | TimeoutError)
                ):
                    return ("timeout", b"", attempts)
                return ("provider_unavailable", b"", attempts)
            print(
                f"transient crates.io lookup failure for {name} {version}; "
                f"retrying ({attempts}/{MAX_ATTEMPTS}): {error}",
                file=sys.stderr,
            )
            time.sleep(attempts * 2)
            continue
        retained = body[:MAX_RETAINED_BODY_BYTES]
        verdict = classify_body(name, version, retained)
        if verdict["status"] == "found":
            return ("found", retained, attempts)
        return (verdict["status"], retained, attempts)
    fail(f"crates.io lookup exhausted retries for {name} {version}")


def provenance(
    provider: str, source: str, evidence: bytes, observed_at: int
) -> dict[str, Any]:
    return {
        "origin": "external_provider",
        "provider": provider,
        "source": source,
        "evidence_digest": f"{CHECKSUM_PREFIX}{sha256_hex(evidence)}",
        "observed_at_unix_seconds": observed_at,
    }


def rule_provenance(rule: str, observed_at: int) -> dict[str, Any]:
    """Provenance for a recorded rule (e.g. permission limits), not a fetch."""
    token = f"cargo-allow-final-registry-observation/v1:{rule}".encode("utf-8")
    return provenance(
        "cargo-allow-final-registry-observation", f"rule:{rule}", token, observed_at
    )


def observe_row(
    row: dict[str, Any],
    raw: tuple[str, bytes, int] | None,
    *,
    owner_state: str,
    authority_state: str,
    mark_missing_as_visibility_pending: bool,
    observed_at: int,
) -> dict[str, Any]:
    """Normalize one fetched outcome into the typed observation shape."""
    name = row["cargo_package_name"]
    version = row["package_version"]
    url = version_url(name, version)
    if raw is None:
        raise SystemExit("observe_row requires an explicit fetch outcome")
    kind, body, _attempts = raw
    if kind == "found":
        verdict = classify_body(name, version, body)
    elif kind == "missing" and mark_missing_as_visibility_pending:
        verdict = {"status": "visibility_pending"}
    elif kind == "missing":
        verdict = {"status": "missing"}
    elif kind in ("timeout", "rate_limited", "provider_unavailable", "malformed_response"):
        verdict = {"status": kind}
    else:  # pragma: no cover - defensive: unknown fetch kinds fail closed
        fail(f"unknown fetch outcome for {name} {version}: {kind}")
    return {
        "package_name": name,
        "package_version": version,
        "version": verdict,
        "version_provenance": provenance("crates.io", url, body, observed_at),
        "owner": owner_state,
        "owner_provenance": rule_provenance(f"owner:{owner_state}", observed_at),
        "publish_authority": authority_state,
        "authority_provenance": rule_provenance(
            f"authority:{authority_state}", observed_at
        ),
    }


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


def write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def summarize(observations: list[dict[str, Any]], rows: list[dict[str, Any]]) -> str:
    lines = ["final registry observation: credential-free crates.io read-only pass"]
    for row, observation in zip(rows, observations):
        version = observation["version"]
        status = version.get("status", "?")
        detail = ""
        if status == "found":
            detail = f" checksum_match_pending yanked={version['yanked']}"
        lines.append(
            f"  {row['cargo_package_name']} {row['package_version']}"
            f" -> {status}{detail}"
            f" owner={observation['owner']}"
            f" authority={observation['publish_authority']}"
        )
    surplus = max(0, len(observations) - len(rows))
    lines.append(
        f"rows={len(rows)} surplus_preserved={surplus} "
        "permission=not-proven-by-public-read "
        "next=refresh-after-freeze/before-authorization/before-publish"
    )
    return "\n".join(lines)


def run_helper(helper_args: list[str]) -> None:
    result = subprocess.run(
        helper_args, cwd=ROOT, text=True, stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT, timeout=300, check=False,
    )
    if result.returncode != 0:
        sys.stderr.write(result.stdout)
        fail(f"evaluator helper failed ({result.returncode})")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Credential-free crates.io observer for the final registry preflight."
    )
    parser.add_argument("--topology", type=Path, default=DEFAULT_TOPOLOGY)
    parser.add_argument("--observations-out", type=Path, required=True)
    parser.add_argument("--candidate-input", type=Path, default=None)
    parser.add_argument("--input-out", type=Path, default=None)
    parser.add_argument("--receipt-out", type=Path, default=None)
    parser.add_argument("--extra-observation-file", type=Path, default=None)
    parser.add_argument(
        "--mark-missing-as-visibility-pending", action="store_true",
        help="Ingest a caller-asserted upload-state hint: missing rows become "
        "visibility_pending. Never concludes upload failure.",
    )
    parser.add_argument(
        "--owner-state", default="permission_not_proven", choices=OWNER_STATES,
        help="Owner dimension outcome. The public surface cannot prove "
        "permission; non-default values record an explicitly selected "
        "bounded observation, never a permission oracle.",
    )
    parser.add_argument(
        "--authority-state", default="not_proven", choices=AUTHORITY_STATES,
        help="Publication-authority dimension. This adapter never emits "
        "proven: residual risk stays visible.",
    )
    args = parser.parse_args(argv)

    rows = load_denominator(args.topology)
    observed_at = now_seconds()
    observations: list[dict[str, Any]] = []
    for row in rows:
        raw = fetch_raw(row["cargo_package_name"], row["package_version"])
        observations.append(
            observe_row(
                row, raw, owner_state=args.owner_state,
                authority_state=args.authority_state,
                mark_missing_as_visibility_pending=(
                    args.mark_missing_as_visibility_pending
                ),
                observed_at=observed_at,
            )
        )
    if args.extra_observation_file is not None:
        extras = validate_extra_observations(
            json.loads(args.extra_observation_file.read_text(encoding="utf-8"))
        )
        # Surplus observations are preserved in input order; they never
        # establish selected package authority downstream.
        observations.extend(extras)

    write_json(args.observations_out, observations)

    if args.input_out is not None or args.receipt_out is not None:
        if args.candidate_input is None:
            fail("--candidate-input is required to emit input/receipt bytes")
        base = json.loads(args.candidate_input.read_text(encoding="utf-8"))
        base_observations = base.get("observations")
        if not isinstance(base_observations, list) or len(base_observations) != len(rows):
            fail("candidate input must carry exactly the 13 denominator observations")
        base["observations"] = observations
        base["evaluated_at_unix_seconds"] = observed_at
        if args.input_out is not None:
            write_json(args.input_out, base)
        if args.receipt_out is not None:
            with tempfile.NamedTemporaryFile(
                suffix=".json", delete=False
            ) as merged_handle:
                merged_path = Path(merged_handle.name)
            try:
                write_json(merged_path, base)
                run_helper(
                    [
                        "cargo", "run", "--locked", "-p", "allow-report",
                        "--example", "evaluate_final_registry_preflight",
                        "--", "--input", str(merged_path),
                        "--receipt-out", str(args.receipt_out),
                    ]
                )
            finally:
                merged_path.unlink(missing_ok=True)

    print(summarize(observations, rows))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
