#!/usr/bin/env python3
"""Bind ordinary CI evidence to GitHub's actual merge group (#4602).

This observes the checkout/event identity and required job outcomes. It does not
authorize H, enqueue a PR, publish a status, or prove a later main tree.
"""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib


class InvalidSubject(ValueError):
    """Required merge-group evidence is missing or inconsistent."""


def git(*args):
    result = subprocess.run(
        ["git", *args], check=False, capture_output=True, text=True
    )
    if result.returncode:
        raise InvalidSubject(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def commit(value, label):
    if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{40}", value):
        raise InvalidSubject(f"{label} must be a complete commit SHA")
    if git("rev-parse", "--verify", f"{value}^{{commit}}") != value:
        raise InvalidSubject(f"{label} does not identify the fetched commit")
    return value


def subject(event, environ):
    if environ.get("GITHUB_EVENT_NAME") != "merge_group":
        raise InvalidSubject("ordinary qualification requires a merge_group event")
    if event.get("action") != "checks_requested":
        raise InvalidSubject("merge_group action must be checks_requested")
    group = event.get("merge_group")
    repository = event.get("repository")
    if not isinstance(group, dict) or not isinstance(repository, dict):
        raise InvalidSubject("missing merge group or repository")
    repo = repository.get("full_name")
    if not isinstance(repo, str) or not repo or repo != environ.get("GITHUB_REPOSITORY"):
        raise InvalidSubject("event repository does not match the workflow repository")
    if group.get("base_ref") != "refs/heads/main":
        raise InvalidSubject("ordinary queue qualification targets main")
    head_ref = group.get("head_ref")
    if (
        not isinstance(head_ref, str)
        or not head_ref.startswith("refs/heads/gh-readonly-queue/main/")
        or head_ref != environ.get("GITHUB_REF")
    ):
        raise InvalidSubject("event and workflow must identify the same native queue ref")
    head = commit(group.get("head_sha"), "merge-group head")
    base = commit(group.get("base_sha"), "merge-group base")
    if head != environ.get("GITHUB_SHA") or git("rev-parse", "HEAD") != head:
        raise InvalidSubject("checkout, workflow SHA and actual merge-group G differ")
    if base == head:
        raise InvalidSubject("merge-group base cannot be its candidate head")
    git("merge-base", "--is-ancestor", base, head)
    git("diff", "--exit-code", "HEAD", "--")
    run_id = environ.get("GITHUB_RUN_ID", "")
    attempt = environ.get("GITHUB_RUN_ATTEMPT", "")
    if not re.fullmatch(r"[1-9][0-9]*", run_id) or not re.fullmatch(r"[1-9][0-9]*", attempt):
        raise InvalidSubject("run and attempt identity are required")
    return {
        "schema_id": "cargo-allow.merge-group-subject.v1",
        "schema_version": 1,
        "repository": repo,
        "base_ref": group["base_ref"],
        "base_sha": base,
        "head_ref": head_ref,
        "head_sha": head,
        "tree_sha": git("rev-parse", f"{head}^{{tree}}"),
        "parents": git("show", "-s", "--format=%P", head).split(),
        "run_id": run_id,
        "run_attempt": attempt,
        "workflow_ref": environ.get("GITHUB_WORKFLOW_REF", ""),
        "claim_boundary": "Observed G identity; not H authorization or M reconciliation.",
    }


def required_results(needs, manifest):
    # Keep #3358's declared mandatory product portfolio as the authority.
    # Informational sibling failures must not become cargo-allow gates.
    required = {row["id"] for row in manifest["lane"] if row["posture"] == "required"}
    required = (required | {"queue-subject", "pregate"}) - {"queue-qualification"}
    if not isinstance(needs, dict) or set(needs) != required:
        raise InvalidSubject("aggregate dependencies differ from the mandatory lane portfolio")
    results = {}
    for lane in sorted(required):
        record = needs[lane]
        if not isinstance(record, dict) or record.get("result") != "success":
            raise InvalidSubject(f"required lane {lane} did not complete successfully")
        results[lane] = record["result"]
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("subject", "result"))
    parser.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args()
    try:
        event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text(encoding="utf-8"))
        if not isinstance(event, dict):
            raise InvalidSubject("event must be an object")
        receipt = subject(event, os.environ)
        if args.mode == "result":
            manifest = tomllib.loads(Path("docs/ci-lanes.toml").read_text(encoding="utf-8"))
            receipt["required_job_results"] = required_results(
                json.loads(os.environ["QUEUE_NEEDS_JSON"]), manifest
            )
            receipt["schema_id"] = "cargo-allow.merge-group-result.v1"
            receipt["claim_boundary"] = (
                "Required ci.yml jobs succeeded on G. Independent queue guards, "
                "H authorization, native protected landing and M readback remain separate."
            )
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"{args.mode}: {receipt['head_sha']} tree {receipt['tree_sha']}")
        return 0
    except (InvalidSubject, OSError, ValueError, KeyError, TypeError) as exc:
        print(f"merge-group {args.mode}: NOT_PROVEN: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
