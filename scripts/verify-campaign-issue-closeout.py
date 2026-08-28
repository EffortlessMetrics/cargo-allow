#!/usr/bin/env python3
"""Guard active campaign issue close events with bounded GitHub evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import urllib.request
from pathlib import Path
from typing import Any

MARKER = "<!-- cargo-allow:campaign-closeout.v1 -->"
SCHEMA = "cargo-allow.campaign-issue-closeout.v1"
ALLOWED_RESULTS = {"Complete", "NotPlanned", "Duplicate"}
INVENTORY_SCHEMA = "cargo-allow.evidence-surface-inventory.v1"
# Evidence classes that name real evidence strength. A `Complete` closeout
# whose acceptance backing carries none of these classes is rejected
# (criterion 7; negative control 12 names the LexicalProjectionOnly shape).
# Everything else — including classes unknown to this guard — is treated as
# insufficient: an unrecognized class cannot be assumed to prove the named
# authority.
SUFFICIENT_EVIDENCE_CLASSES = {
    "StructuredShapeValidation",
    "TypedModelValidation",
    "ProductionBehaviorValidation",
    "ExternalObservationValidation",
    "LiveControlReadback",
}


class GitHub:
    def __init__(self, api_url: str, repository: str, token: str):
        self.base = f"{api_url.rstrip('/')}/repos/{repository}"
        self.token = token

    def request(self, path: str, method: str = "GET", payload: dict[str, Any] | None = None) -> Any:
        body = None if payload is None else json.dumps(payload).encode()
        request = urllib.request.Request(
            self.base + path,
            data=body,
            method=method,
            headers={
                "Accept": "application/vnd.github+json",
                "Authorization": f"Bearer {self.token}",
                "X-GitHub-Api-Version": "2022-11-28",
                "Content-Type": "application/json",
                "User-Agent": "cargo-allow-campaign-closeout/1",
            },
        )
        with urllib.request.urlopen(request, timeout=20) as response:
            return json.load(response)


def load_membership(path: Path) -> dict[int, tuple[str, set[str]]]:
    import tomllib

    data = tomllib.loads(path.read_text(encoding="utf-8"))
    children = data.get("children", [])
    result: dict[int, tuple[str, set[str]]] = {}
    for child in children:
        issue = child.get("issue")
        required = child.get("required")
        accepted = child.get("accepted", [required])
        if not isinstance(issue, int) or not isinstance(required, str):
            raise ValueError("campaign membership contains an invalid child row")
        if not isinstance(accepted, list) or not all(isinstance(item, str) for item in accepted):
            raise ValueError(f"campaign membership has invalid accepted results for #{issue}")
        if required not in accepted:
            raise ValueError(f"required result is not accepted for #{issue}")
        if issue in result:
            raise ValueError(f"campaign membership duplicates issue #{issue}")
        result[issue] = (required, set(accepted))
    return result


def closeout_from_body(body: str) -> dict[str, Any] | None:
    if MARKER not in body:
        return None
    fenced = re.search(r"```json\s*(\{.*?\})\s*```", body, flags=re.DOTALL)
    if not fenced:
        raise ValueError("closeout marker is present without a JSON object")
    value = json.loads(fenced.group(1))
    if not isinstance(value, dict):
        raise ValueError("closeout payload must be an object")
    return value


def validate_closeout(payload: dict[str, Any] | None, issue_number: int, accepted: set[str]) -> list[str]:
    if payload is None:
        return ["missing_closeout"]
    errors: list[str] = []
    if payload.get("schema_id") != SCHEMA:
        errors.append("schema_mismatch")
    if payload.get("issue") != issue_number:
        errors.append("issue_identity_mismatch")
    result = payload.get("result")
    if result not in ALLOWED_RESULTS or result not in accepted:
        errors.append("result_not_accepted")
    if result == "Complete":
        if not isinstance(payload.get("merged_pr"), int):
            errors.append("merged_pr_missing")
        if not isinstance(payload.get("closeout_id"), str) or not payload["closeout_id"].strip():
            errors.append("closeout_id_missing")
    elif result == "Duplicate":
        if not isinstance(payload.get("replacement_issue"), int):
            errors.append("replacement_issue_missing")
    elif result == "NotPlanned":
        if not isinstance(payload.get("reason"), str) or not payload["reason"].strip():
            errors.append("reason_missing")
    return errors


def load_evidence_surfaces(path: Path) -> dict[str, str]:
    """Load the checked evidence-surface inventory as `id -> evidence_class`."""
    import tomllib

    data = tomllib.loads(path.read_text(encoding="utf-8"))
    if data.get("schema") != INVENTORY_SCHEMA:
        raise ValueError("evidence surface inventory schema mismatch")
    result: dict[str, str] = {}
    for surface in data.get("surfaces", []):
        if not isinstance(surface, dict):
            raise ValueError("evidence surface inventory contains a non-table row")
        surface_id = surface.get("id")
        evidence_class = surface.get("evidence_class")
        if not isinstance(surface_id, str) or not surface_id.strip():
            raise ValueError("evidence surface inventory contains an invalid id")
        if not isinstance(evidence_class, str) or not evidence_class.strip():
            raise ValueError(f"evidence surface {surface_id} has an invalid class")
        if surface_id in result:
            raise ValueError(f"evidence surface inventory duplicates id {surface_id}")
        result[surface_id] = evidence_class
    return result


def verify_acceptance_evidence(payload: dict[str, Any], surfaces: dict[str, str]) -> list[str]:
    """Reject `Complete` when the declared acceptance backing is insufficient.

    The closeout must name the inventory surfaces that back its acceptance
    rows; every named surface must exist, and at least one must carry one of
    the named sufficient evidence classes (#3810 criterion 7). Classes outside
    the sufficient set — including unknown ones, which cannot be assumed to
    prove the named authority — are treated as insufficient.
    """
    declared = payload.get("evidence_surfaces")
    if not isinstance(declared, list) or not declared:
        return ["evidence_surfaces_missing"]
    if not all(isinstance(item, str) and item.strip() for item in declared):
        return ["evidence_surfaces_invalid"]
    if len(set(declared)) != len(declared):
        return ["evidence_surfaces_invalid"]
    if any(item not in surfaces for item in declared):
        return ["evidence_surface_unknown"]
    classes = {surfaces[item] for item in declared}
    if not classes & SUFFICIENT_EVIDENCE_CLASSES:
        return ["insufficient_evidence_class"]
    return []


def verify_complete(api: GitHub, payload: dict[str, Any], base_branch: str) -> list[str]:
    pr = api.request(f"/pulls/{payload['merged_pr']}")
    errors: list[str] = []
    if pr.get("merged_at") is None or pr.get("state") != "closed":
        errors.append("pull_request_not_merged")
    if pr.get("base", {}).get("ref") != base_branch:
        errors.append("pull_request_wrong_base")
    merge_sha = pr.get("merge_commit_sha")
    if not isinstance(merge_sha, str) or not re.fullmatch(r"[0-9a-f]{40}", merge_sha):
        errors.append("merge_commit_missing")
    if errors:
        return errors
    # Put the candidate commit on the base side: a merge commit is reachable
    # when main is identical to it or contains it with later commits.
    comparison = api.request(f"/compare/{merge_sha}...{base_branch}")
    if comparison.get("status") not in {"ahead", "identical"}:
        errors.append("merge_commit_not_reachable_from_main")
    return errors


def bounded_comment(issue_number: int, errors: list[str], payload: dict[str, Any] | None) -> str:
    digest = hashlib.sha256(json.dumps(payload, sort_keys=True).encode()).hexdigest()[:16]
    codes = ", ".join(sorted(set(errors)))
    return (
        f"{MARKER}\n"
        "## Campaign closeout rejected\n"
        f"Issue: #{issue_number}\n"
        f"Result: `InstrumentFailure` / `NotProven`\n"
        f"Codes: `{codes}`\n"
        f"Closeout identity: `{digest}`\n"
        "The issue was reopened because the checked active campaign denominator "
        "does not have current evidence for a valid close. Repair the exact "
        "closeout rows and close it through the reviewed maintainer path.\n\n"
        "Claim boundary: this guard protects issue state; it does not perform "
        "the work, merge code, or execute a release."
    )


def handle(
    event: dict[str, Any],
    api: GitHub,
    membership: dict[int, tuple[str, set[str]]],
    base_branch: str,
    inventory_path: Path,
) -> int:
    if event.get("action") != "closed" or not isinstance(event.get("issue"), dict):
        return 0
    issue = event["issue"]
    number = issue.get("number")
    if not isinstance(number, int) or number not in membership:
        return 0
    _required, accepted = membership[number]
    payload: dict[str, Any] | None = None
    try:
        payload = closeout_from_body(issue.get("body") or "")
        errors = validate_closeout(payload, number, accepted)
        if not errors and payload and payload.get("result") == "Complete":
            errors.extend(verify_complete(api, payload, base_branch))
            errors.extend(verify_acceptance_evidence(payload, load_evidence_surfaces(inventory_path)))
    except (OSError, ValueError, TypeError, KeyError, AttributeError, json.JSONDecodeError):
        errors = ["instrument_failure"]
    if not errors:
        return 0
    comment = bounded_comment(number, errors, payload)
    comments = api.request(f"/issues/{number}/comments?per_page=100")
    if not any(item.get("body") == comment for item in comments if isinstance(item, dict)):
        api.request(f"/issues/{number}/comments", method="POST", payload={"body": comment})
    api.request(f"/issues/{number}", method="PATCH", payload={"state": "open", "state_reason": "reopened"})
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--event", type=Path, default=Path(os.environ["GITHUB_EVENT_PATH"]))
    parser.add_argument("--membership", type=Path, default=Path("policy/campaign-issue-closeout.toml"))
    parser.add_argument("--inventory", type=Path, default=Path("policy/evidence-surface-inventory.toml"))
    args = parser.parse_args()
    event = json.loads(args.event.read_text(encoding="utf-8"))
    membership = load_membership(args.membership)
    api = GitHub(os.environ.get("GITHUB_API_URL", "https://api.github.com"), os.environ["GITHUB_REPOSITORY"], os.environ["GITHUB_TOKEN"])
    return handle(event, api, membership, "main", args.inventory)


if __name__ == "__main__":
    raise SystemExit(main())
