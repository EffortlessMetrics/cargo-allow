#!/usr/bin/env python3
"""Lane applicability for #2569 and #4332.

This is the one source-controlled plan for which CI lanes execute.
``scripts/feature-config-routing.py`` stays the row selector inside
``feature-configuration-proof`` when that lane runs. Release-impact
classification stays in the release graph. This script does not replace
either of those.

A ``pull_request`` run is narrow only in two cases:

- the successor delta (``before``..head, when ``before`` is a real
  ancestor) is entirely under ``.allow/review-dispositions/`` AND, when
  the effective diff is source-bearing or shared impact, the predecessor
  has exact terminal policy-admissible proof for that same source and base
- there is no such successor, or the successor is not source-bearing, and
  the effective diff (merge-base of the PR base..head) is provably
  docs-only or ledger-only

A docs-only successor on a source-bearing pull request stays wide: docs
routing uses the effective diff, not the last commit. A ledger successor
does not narrow a source-bearing effective diff unless that predecessor
proof exists. Missing, stale, wrong-source, failed, pending, cancelled,
ambiguous, skipped, and base-moved proof stays wide. ``docs/schemas/``
and any ``*.schema.json`` are source, not docs. Markdown is the only
prose under ``docs/`` that may narrow. JSON, TOML, and any other
non-markdown path under ``docs/`` are source, including the
feature-configuration matrix, the lane and support tables, and
direct-floor receipts. An unrecognized docs payload is source, not
prose.

Unknown, empty, rename/copy, malformed, and shared ledger impact stay
wide. ``push``, ``workflow_dispatch``, ``merge_group``, ``schedule``,
``release``, and every other event stay wide, including before the
merge-group trigger exists. Planner failure writes ``narrow=false`` and
exits non-zero so heavy lanes still run. Lookup failure stays wide and
exits zero: absence is not proof.

The ruleset contexts (pregate, test, review-readiness,
frozen-subject-lock), package-smoke, cargo-deny, and the release graph
always execute. Heavy non-required lanes are NotApplicable only when this
plan succeeds with ``narrow=true``; GitHub then skips those jobs. A skip
is not a green execution.
"""

from __future__ import annotations

import io
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from pathlib import Path

SCHEMA_ID = "cargo-allow.ci-proof-plan.v1"
PROOF_SCHEMA_ID = "cargo-allow.ci-predecessor-proof.v1"
OBSERVATION_SCHEMA_ID = "cargo-allow.ci-predecessor-observation.v1"
LEDGER_PREFIX = ".allow/review-dispositions/"
CI_WORKFLOW = ".github/workflows/ci.yml"
REVIEW_WORKFLOW = ".github/workflows/review-readiness.yml"
FROZEN_WORKFLOW = ".github/workflows/frozen-subject-lock.yml"
REVIEW_CHECK = "check:review-readiness"
SHA = re.compile(r"^[0-9a-f]{40}([0-9a-f]{24})?$")
MAX_PATHS = 2000
MAX_FOLLOW = 4
MAX_BODY = 2 * 1024 * 1024
API_VERSION = "2022-11-28"

NARROW_IF_WITH_PREGATE = (
    "${{ !cancelled() && needs.pregate.result == 'success' && "
    "(needs.proof-route.result != 'success' || needs.proof-route.outputs.narrow != 'true') }}"
)
NARROW_IF_STANDALONE = (
    "${{ !cancelled() && (needs.proof-route.result != 'success' || "
    "needs.proof-route.outputs.narrow != 'true') }}"
)

# Jobs in ci.yml that execute on a narrow plan. review-readiness and
# frozen-subject-lock are recorded here and live in their own workflows.
ALWAYS_CI = [
    ("pregate", "ruleset context; current assertions stay on every applicable run"),
    ("test", "ruleset context; no-new and the change-note guard stay unconditional"),
    ("package-smoke", "test restores this lane's cutover receipts"),
    ("cargo-deny", "supply-chain audit stays on every run"),
    ("release-impact", "release qualification stays on every event"),
    ("release-rehearsal", "release qualification stays on every event"),
    (
        "release-rehearsal-required",
        "a skipped release proof stays fail-closed, not a synthetic pass",
    ),
    ("proof-route", "this plan"),
]
RECORD_ONLY = [
    ("review-readiness", "ruleset context in its own workflow; this plan does not filter it"),
    ("frozen-subject-lock", "ruleset context in its own workflow; this plan does not filter it"),
]
NARROW_WITH_PREGATE = [
    "msrv",
    "product-candidates-interop",
    "operator-latency",
    "integrated-dogfood",
    "test-core-platforms",
    "compat-delegation",
    "test-intent-experimental",
    "test-proof-experimental",
    "test-shared-protocol",
    "coverage",
]
NARROW_STANDALONE = [
    "workflow-syntax",
    "workflow-security",
    "workflow-construction",
    "dependency-graph-delta",
    "collect-performance-fixtures",
    "shallow-diff-smoke",
    "feature-configuration-proof",
]
ROOT_DOCS = {
    "AGENTS.md",
    "CHANGELOG.md",
    "CODE_OF_CONDUCT.md",
    "CONTRIBUTING.md",
    "COPYING",
    "LICENSE",
    "NOTICE",
    "README.md",
    "SECURITY.md",
}


def contract() -> dict[str, object]:
    return {
        "narrow_if_with_pregate": NARROW_IF_WITH_PREGATE,
        "narrow_if_standalone": NARROW_IF_STANDALONE,
        "always_ci_jobs": [job for job, _reason in ALWAYS_CI],
        "record_only": [job for job, _reason in RECORD_ONLY],
        "narrow_with_pregate": list(NARROW_WITH_PREGATE),
        "narrow_standalone": list(NARROW_STANDALONE),
    }


def normalize(path: str) -> str:
    return path.strip().replace("\\", "/").removeprefix("./")


def path_kind(path: str) -> str:
    """Return ledger, docs, other, or unknown for one changed path."""
    normalized = normalize(path)
    if not normalized or normalized.startswith("/") or normalized.endswith("/"):
        return "unknown"
    parts = normalized.split("/")
    if any(part in {"", ".", ".."} for part in parts):
        return "unknown"
    if normalized.startswith(LEDGER_PREFIX) and len(parts) >= 3:
        return "ledger"
    if normalized.startswith(".allow/"):
        return "other"
    # Schemas are load-bearing for the consumers that read them, including
    # operator-latency. A docs/ prefix must not skip that assertion.
    # Markdown is the only prose under docs/ that may narrow. The
    # feature-configuration matrix, ci-lanes, the support matrix, and
    # direct-floor receipts are parsed by proof jobs. Any other
    # non-markdown docs path is source too: an unrecognized payload must
    # not be assumed prose.
    if normalized.startswith("docs/schemas/") or normalized.endswith(".schema.json"):
        return "other"
    if normalized.startswith("docs/"):
        if normalized.endswith(".md"):
            return "docs"
        return "other"
    if normalized.startswith(".changes/"):
        return "docs"
    if len(parts) == 1 and (normalized in ROOT_DOCS or normalized.startswith("LICENSE")):
        return "docs"
    if len(parts) == 1 and normalized.endswith(".md"):
        return "docs"
    return "other"


def parse_name_status(text: str) -> list[tuple[str, str]] | None:
    """Parse ``git diff --name-status --no-renames`` output.

    Rename and copy statuses fail closed (``None``) so a moved source file
    cannot be classified as the destination path alone. ``None`` is unknown
    impact. An empty diff is an empty list, which classification also treats
    as unknown.
    """
    entries: list[tuple[str, str]] = []
    for raw in text.splitlines():
        line = raw.strip("\r")
        if not line.strip():
            continue
        parts = line.split("\t")
        status = parts[0]
        if not status:
            return None
        code = status[0]
        if code in {"R", "C"}:
            return None
        if code not in {"A", "M", "D", "T"} or len(parts) != 2 or not parts[1]:
            return None
        if parts[1].startswith('"'):
            return None
        entries.append((code, parts[1]))
        if len(entries) > MAX_PATHS:
            return None
    return entries


def classify_entries(entries: list[tuple[str, str]] | None) -> str:
    if entries is None or not entries:
        return "unknown"
    kinds: set[str] = set()
    for status, path in entries:
        if status not in {"A", "M", "D", "T"}:
            return "unknown"
        kind = path_kind(path)
        if kind == "unknown":
            return "unknown"
        kinds.add(kind)
    if "ledger" in kinds and kinds != {"ledger"}:
        return "shared_impact"
    if kinds == {"ledger"}:
        return "review_ledger_only"
    if kinds == {"docs"}:
        return "docs_only"
    return "source_bearing"


def decide(
    event: str,
    successor_class: str | None,
    effective_class: str,
    predecessor_admitted: bool = False,
) -> tuple[str, bool]:
    """Return ``(class, narrow)``.

    ``successor_class is None`` means the event has no predecessor (a pull
    request open, or a zero ``before``). A docs-only successor does not
    narrow a source-bearing effective diff. A ledger successor narrows a
    source-bearing or shared effective diff only when predecessor proof was
    admitted. A ledger-only effective diff still narrows on its own: it
    does not claim that missing source proof is reusable.
    """
    if event != "pull_request":
        return "full_boundary", False
    if successor_class == "review_ledger_only" and effective_class in {
        "source_bearing",
        "shared_impact",
    }:
        if predecessor_admitted:
            return "review_ledger_only", True
        return "unproven_predecessor", False
    if successor_class in {"unknown", "shared_impact", "source_bearing"}:
        return successor_class, False
    if effective_class == "docs_only":
        return "docs_only", True
    if effective_class == "review_ledger_only":
        return "review_ledger_only", True
    if effective_class in {"unknown", "shared_impact", "source_bearing"}:
        return effective_class, False
    return "unknown", False


def lane_rows(class_name: str, narrow: bool) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    ordered = [
        ALWAYS_CI[0],
        ALWAYS_CI[1],
        *RECORD_ONLY,
        *ALWAYS_CI[2:],
    ]
    for job, reason in ordered:
        rows.append({"id": job, "decision": "execute", "reason": reason})
    for job in NARROW_WITH_PREGATE + NARROW_STANDALONE:
        if narrow:
            rows.append(
                {
                    "id": job,
                    "decision": "not_applicable",
                    "reason": f"narrow {class_name}",
                }
            )
        else:
            rows.append(
                {
                    "id": job,
                    "decision": "execute",
                    "reason": f"wide {class_name}",
                }
            )
    return rows


def build_plan(
    event: str,
    class_name: str,
    narrow: bool,
    head: str,
    base: str,
    successor_before: str,
    successor_paths: list[str],
    effective_paths: list[str],
    proof_identity: dict[str, object] | None = None,
    predecessor_proof: dict[str, str] | None = None,
) -> dict[str, object]:
    plan: dict[str, object] = {
        "schema_id": SCHEMA_ID,
        "schema_version": 1,
        "event": event,
        "class": class_name,
        "narrow": narrow,
        "head": head,
        "base": base,
        "successor_before": successor_before,
        "successor_paths": successor_paths,
        "effective_paths": effective_paths,
        "lanes": lane_rows(class_name, narrow),
    }
    if narrow and proof_identity is not None:
        plan["proof_identity"] = proof_identity
    if predecessor_proof is not None:
        plan["predecessor_proof"] = predecessor_proof
    return plan


def paths_of(entries: list[tuple[str, str]] | None) -> list[str]:
    if not entries:
        return []
    return [path for _status, path in entries]


def source_paths_from(paths: object) -> list[str]:
    if not isinstance(paths, list) or not all(isinstance(path, str) for path in paths):
        return []
    return [path for path in paths if path_kind(path) != "ledger"]


def git(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        check=False,
        capture_output=True,
        text=True,
    )


def commit_exists(sha: str) -> bool:
    if not SHA.fullmatch(sha):
        return False
    completed = git("cat-file", "-e", f"{sha}^{{commit}}")
    return completed.returncode == 0


def is_ancestor(before: str, head: str) -> bool:
    if not SHA.fullmatch(before) or not SHA.fullmatch(head):
        return False
    completed = git("merge-base", "--is-ancestor", before, head)
    return completed.returncode == 0


def diff_entries(left: str, right: str) -> list[tuple[str, str]] | None:
    completed = git("diff", "--name-status", "--no-renames", left, right)
    if completed.returncode != 0:
        return None
    return parse_name_status(completed.stdout)


def predecessor_state(before: str) -> str:
    value = before.strip().lower()
    if value == "" or (len(value) >= 40 and set(value) <= {"0"}):
        return "absent"
    if SHA.fullmatch(value):
        return "sha"
    return "malformed"


def proof_job_rules() -> dict[str, frozenset[str]]:
    """Conclusions that count as proof on the wide predecessor run.

    ``review-readiness`` may be neutral: a draft with no disposition still
    publishes a successful publisher job and a neutral check. Failure does
    not. Skipped heavy jobs are not success.
    """
    rules = {job: frozenset({"success"}) for job, _reason in ALWAYS_CI}
    for job in NARROW_WITH_PREGATE + NARROW_STANDALONE:
        rules[job] = frozenset({"success"})
    rules["review-readiness-publisher"] = frozenset({"success"})
    rules["review-readiness"] = frozenset({"success", "neutral"})
    rules["frozen-subject-lock"] = frozenset({"success"})
    return rules


def rejection(reason: str) -> dict[str, object]:
    return {"admitted": False, "reason": reason, "evidence": None}


def terminal_reason(conclusion: object) -> str:
    text = str(conclusion or "").strip().lower()
    if text in {"cancelled", "canceled"}:
        return "cancelled"
    if text == "skipped":
        return "skipped"
    if text in {"pending", "queued", "in_progress", "waiting", "requested"}:
        return "pending"
    return "failed"


def aggregate(reasons: list[str]) -> str:
    for kind in ("pending", "cancelled", "skipped", "failed", "ambiguous", "absent", "malformed"):
        if kind in reasons:
            return kind
    return "failed" if reasons else "success"


def runs_for(runs: list[object], workflow: str, sha: str) -> list[dict[str, object]]:
    matched: list[dict[str, object]] = []
    for run in runs:
        if not isinstance(run, dict):
            continue
        if run.get("workflow") != workflow:
            continue
        if str(run.get("head_sha", "")).lower() != sha:
            continue
        matched.append(run)
    return matched


def classify_group(matched: list[dict[str, object]]) -> tuple[str, dict[str, object] | None]:
    if not matched:
        return "absent", None
    if any(run.get("status") != "completed" for run in matched):
        return "pending", None
    successes = [run for run in matched if run.get("conclusion") == "success"]
    if len(successes) > 1 or (len(successes) == 1 and len(matched) != 1):
        return "ambiguous", None
    if not successes:
        chosen = max(matched, key=lambda run: str(run.get("run_id", "")))
        return terminal_reason(chosen.get("conclusion")), None
    return "candidate", successes[0]


def classify_check(matched: list[dict[str, object]]) -> tuple[str, str | None]:
    if not matched:
        return "absent", None
    if any(run.get("status") != "completed" for run in matched):
        return "pending", None
    good = [run for run in matched if run.get("conclusion") in {"success", "neutral"}]
    if len(good) != 1 or len(matched) != 1:
        if not good:
            chosen = max(matched, key=lambda run: str(run.get("run_id", "")))
            return terminal_reason(chosen.get("conclusion")), None
        return "ambiguous", None
    return "candidate", str(good[0].get("conclusion"))


def job_problem(conclusion: object, allowed: frozenset[str]) -> str | None:
    if conclusion is None:
        return "absent"
    text = str(conclusion).strip().lower()
    if text in allowed:
        return None
    if text == "":
        return "absent"
    return terminal_reason(text)


def proof_conclusions(
    runs: list[object], sha: str, ci_run: dict[str, object]
) -> str:
    reasons: list[str] = []
    jobs = ci_run.get("jobs")
    if not isinstance(jobs, dict):
        return "malformed"
    review_kind, review = classify_group(runs_for(runs, REVIEW_WORKFLOW, sha))
    if review_kind != "candidate" or review is None:
        reasons.append(review_kind)
    else:
        publisher = review.get("jobs") if isinstance(review.get("jobs"), dict) else {}
        problem = job_problem(publisher.get("review-readiness-publisher"), frozenset({"success"}))
        if problem:
            reasons.append(problem)
    frozen_kind, frozen = classify_group(runs_for(runs, FROZEN_WORKFLOW, sha))
    if frozen_kind != "candidate" or frozen is None:
        reasons.append(frozen_kind)
    else:
        frozen_jobs = frozen.get("jobs") if isinstance(frozen.get("jobs"), dict) else {}
        problem = job_problem(frozen_jobs.get("frozen-subject-lock"), frozenset({"success"}))
        if problem:
            reasons.append(problem)
    check_kind, _check_value = classify_check(runs_for(runs, REVIEW_CHECK, sha))
    if check_kind != "candidate":
        reasons.append(check_kind)
    ci_rules = {
        name: allowed
        for name, allowed in proof_job_rules().items()
        if name not in {"review-readiness-publisher", "review-readiness", "frozen-subject-lock"}
    }
    for name, allowed in ci_rules.items():
        problem = job_problem(jobs.get(name), allowed)
        if problem:
            reasons.append(problem)
    return aggregate(reasons)


def reduce_observation(observation: object) -> dict[str, object]:
    """Admit one wide predecessor run, or reject. No network and no git."""
    if not isinstance(observation, dict):
        return rejection("malformed")
    if observation.get("schema_id") != OBSERVATION_SCHEMA_ID:
        return rejection("malformed")
    before = str(observation.get("before", "")).lower()
    base = str(observation.get("base", "")).lower()
    source_paths = observation.get("source_paths")
    runs = observation.get("runs")
    if not isinstance(runs, list) or not all(isinstance(run, dict) for run in runs):
        return rejection("malformed")
    if not isinstance(source_paths, list) or not all(isinstance(path, str) for path in source_paths):
        return rejection("malformed")
    if not source_paths:
        return rejection("wrong_source")
    if not SHA.fullmatch(before) or not SHA.fullmatch(base):
        return rejection("malformed")

    sha = before
    seen: set[str] = set()
    for _hop in range(MAX_FOLLOW):
        if sha in seen:
            return rejection("ambiguous")
        seen.add(sha)
        kind, run = classify_group(runs_for(runs, CI_WORKFLOW, sha))
        if kind != "candidate" or run is None:
            return rejection(kind)
        plan = run.get("plan")
        if not isinstance(plan, dict):
            return rejection("absent")
        if str(plan.get("head", "")).lower() != sha:
            return rejection("stale")
        if plan.get("narrow") is True:
            identity = plan.get("proof_identity")
            if not isinstance(identity, dict):
                return rejection("absent")
            if identity.get("status") != "success":
                mapped = terminal_reason(identity.get("status"))
                return rejection(mapped if mapped != "failed" else "failed")
            if str(identity.get("base", "")).lower() != base:
                return rejection("base_moved")
            if identity.get("source_paths") != source_paths:
                return rejection("wrong_source")
            proven = str(identity.get("proven_head", "")).lower()
            if not SHA.fullmatch(proven):
                return rejection("malformed")
            sha = proven
            continue
        if plan.get("narrow") is not False:
            return rejection("malformed")
        if str(plan.get("base", "")).lower() != base:
            return rejection("base_moved")
        if source_paths_from(plan.get("effective_paths")) != source_paths:
            return rejection("wrong_source")
        job_reason = proof_conclusions(runs, sha, run)
        if job_reason != "success":
            return rejection(job_reason)
        return {
            "admitted": True,
            "reason": "success",
            "evidence": {
                "schema_id": PROOF_SCHEMA_ID,
                "predecessor": before,
                "proven_head": sha,
                "base": base,
                "source_paths": list(source_paths),
                "status": "success",
                "run_id": str(run.get("run_id", "")),
                "run_url": str(run.get("run_url", "")),
            },
        }
    return rejection("ambiguous")


def admit_recorded_proof(
    evidence: object,
    *,
    before: str,
    base: str,
    source_paths: list[str],
    delta_entries: list[tuple[str, str]] | None,
) -> tuple[dict[str, object] | None, str]:
    """Accept recorded proof only when identity and ledger movement match."""
    if not isinstance(evidence, dict) or evidence.get("schema_id") != PROOF_SCHEMA_ID:
        return None, "malformed"
    if evidence.get("status") != "success":
        mapped = terminal_reason(evidence.get("status"))
        if str(evidence.get("status") or "").strip().lower() in {
            "absent",
            "pending",
            "ambiguous",
            "stale",
            "wrong_source",
            "base_moved",
            "malformed",
            "skipped",
            "cancelled",
            "canceled",
            "failed",
            "failure",
        }:
            text = str(evidence.get("status")).strip().lower()
            aliases = {"failure": "failed", "canceled": "cancelled"}
            return None, aliases.get(text, text if text != "failure" else "failed")
        return None, mapped
    predecessor = str(evidence.get("predecessor", "")).lower()
    proven = str(evidence.get("proven_head", "")).lower()
    evidence_base = str(evidence.get("base", "")).lower()
    if not SHA.fullmatch(before) or predecessor != before:
        return None, "stale"
    if not SHA.fullmatch(base) or evidence_base != base:
        return None, "base_moved"
    if not SHA.fullmatch(proven):
        return None, "malformed"
    if evidence.get("source_paths") != source_paths or not source_paths:
        return None, "wrong_source"
    if not isinstance(delta_entries, list):
        return None, "absent"
    if classify_entries(delta_entries) != "review_ledger_only":
        return None, "wrong_source"
    return (
        {
            "schema_id": PROOF_SCHEMA_ID,
            "predecessor": predecessor,
            "proven_head": proven,
            "base": evidence_base,
            "source_paths": list(source_paths),
            "status": "success",
            "run_id": str(evidence.get("run_id", "")),
            "run_url": str(evidence.get("run_url", "")),
        },
        "success",
    )


def movement_entries(proven: str, head: str) -> list[tuple[str, str]] | None:
    if not SHA.fullmatch(proven) or not SHA.fullmatch(head):
        return None
    if proven != head and not is_ancestor(proven, head):
        return None
    if proven == head:
        return []
    return diff_entries(proven, head)


def read_evidence_file(path: str) -> dict[str, object] | None:
    try:
        payload = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError):
        return None
    return payload if isinstance(payload, dict) else None


class _LookupStop(Exception):
    def __init__(self, reason: str) -> None:
        super().__init__(reason)
        self.reason = reason


class _StripAuthRedirect(urllib.request.HTTPRedirectHandler):
    """Drop Authorization before a redirect leaves the API host."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):  # noqa: ANN001
        redirected = super().redirect_request(req, fp, code, msg, headers, newurl)
        if redirected is not None:
            redirected.remove_header("Authorization")
        return redirected


def api_request_url(path: str, query: dict[str, str] | None) -> str:
    base = os.environ.get("GITHUB_API_URL", "https://api.github.com").strip().rstrip("/")
    parsed = urllib.parse.urlparse(base)
    if parsed.scheme != "https" or not parsed.netloc or parsed.username or parsed.password:
        raise _LookupStop("absent")
    if not path.startswith("/") or path.startswith("//"):
        raise _LookupStop("malformed")
    url = f"{parsed.scheme}://{parsed.netloc}{path}"
    if query:
        url = f"{url}?{urllib.parse.urlencode(query)}"
    return url


def github_bytes(path: str, query: dict[str, str] | None) -> bytes:
    token = os.environ.get("GITHUB_TOKEN", "").strip()
    if not token:
        raise _LookupStop("absent")
    request = urllib.request.Request(
        api_request_url(path, query),
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "User-Agent": "cargo-allow-ci-proof-route",
            "X-GitHub-Api-Version": API_VERSION,
        },
        method="GET",
    )
    opener = urllib.request.build_opener(_StripAuthRedirect)
    try:
        with opener.open(request, timeout=15) as response:
            data = response.read(MAX_BODY + 1)
    except urllib.error.HTTPError as error:
        raise _LookupStop("absent") from error
    except (urllib.error.URLError, TimeoutError, OSError) as error:
        raise _LookupStop("absent") from error
    if len(data) > MAX_BODY:
        raise _LookupStop("malformed")
    return data


def github_json(path: str, query: dict[str, str] | None) -> dict[str, object]:
    try:
        payload = json.loads(github_bytes(path, query))
    except json.JSONDecodeError as error:
        raise _LookupStop("malformed") from error
    if not isinstance(payload, dict):
        raise _LookupStop("malformed")
    return payload


def paged_items(path: str, query: dict[str, str], key: str) -> list[object]:
    items: list[object] = []
    for page in range(1, 4):
        payload = github_json(path, {**query, "per_page": "100", "page": str(page)})
        batch = payload.get(key)
        if not isinstance(batch, list):
            raise _LookupStop("malformed")
        items.extend(batch)
        if len(batch) < 100:
            return items
    raise _LookupStop("ambiguous")


def display_job_key(name: str, keys: frozenset[str]) -> str | None:
    """Map one Actions job ``name`` onto an expected workflow job key.

    The jobs API reports the display name. A plain job uses its key. A
    matrix cell is ``<key> (<matrix values>)``. An exact key wins. Otherwise
    the longest key that is a ``"<key> ("`` prefix of a closed display name
    wins, so ``release-rehearsal`` does not absorb
    ``release-rehearsal-required``. Two equal-length matches are ambiguous.
    A name that matches nothing is not a proof row.
    """
    if name in keys:
        return name
    matches = [
        key
        for key in keys
        if name.startswith(key + " (") and name.endswith(")") and len(name) > len(key) + 3
    ]
    if not matches:
        return None
    longest = max(len(key) for key in matches)
    best = [key for key in matches if len(key) == longest]
    if len(best) != 1:
        raise _LookupStop("ambiguous")
    return best[0]


def _cell_kind(conclusion: str) -> str:
    text = conclusion.strip().lower()
    if text == "success":
        return "success"
    if text in {"cancelled", "canceled"}:
        return "cancelled"
    if text == "skipped":
        return "skipped"
    if text in {"pending", "queued", "in_progress", "waiting", "requested"}:
        return "pending"
    return "failure"


def combine_cell_conclusions(values: list[str]) -> str:
    """One job key succeeds only when every associated row succeeded."""
    kinds = [_cell_kind(value) for value in values]
    for kind in ("pending", "cancelled", "skipped", "failure"):
        if kind in kinds:
            return kind
    return "success" if kinds else "failure"


def conclusions_from_job_rows(rows: list[object], keys: frozenset[str]) -> dict[str, str]:
    """Index Actions job rows by expected key.

    Duplicate display names are ambiguous. A key with no row is omitted so
    the caller rejects it as missing. Any unsuccessful, skipped, or
    unfinished row keeps that key from counting as success. Rows whose
    names match no expected key are ignored.
    """
    grouped: dict[str, list[str]] = {}
    seen: set[str] = set()
    for job in rows:
        if not isinstance(job, dict):
            raise _LookupStop("malformed")
        name = job.get("name")
        if not isinstance(name, str) or not name:
            raise _LookupStop("malformed")
        if name in seen:
            raise _LookupStop("ambiguous")
        seen.add(name)
        key = display_job_key(name, keys)
        if key is None:
            continue
        if str(job.get("status") or "") != "completed":
            conclusion = "pending"
        else:
            conclusion = str(job.get("conclusion") or "")
        grouped.setdefault(key, []).append(conclusion)
    return {key: combine_cell_conclusions(values) for key, values in grouped.items()}


def list_job_conclusions(repo: str, run_id: str) -> dict[str, str]:
    rows = paged_items(f"/repos/{repo}/actions/runs/{run_id}/jobs", {}, "jobs")
    return conclusions_from_job_rows(rows, frozenset(proof_job_rules()))


def download_plan(repo: str, run_id: str) -> dict[str, object] | None:
    rows = paged_items(f"/repos/{repo}/actions/runs/{run_id}/artifacts", {}, "artifacts")
    matches = [
        item
        for item in rows
        if isinstance(item, dict) and item.get("name") == "ci-proof-plan" and not item.get("expired")
    ]
    if len(matches) != 1:
        return None
    artifact_id = matches[0].get("id")
    blob = github_bytes(f"/repos/{repo}/actions/artifacts/{artifact_id}/zip", None)
    try:
        with zipfile.ZipFile(io.BytesIO(blob)) as archive:
            names = [name for name in archive.namelist() if name.endswith("proof-plan.json")]
            if len(names) != 1:
                return None
            info = archive.getinfo(names[0])
            if info.file_size > MAX_BODY:
                return None
            plan = json.loads(archive.read(names[0]))
    except (zipfile.BadZipFile, json.JSONDecodeError, KeyError, OSError):
        return None
    return plan if isinstance(plan, dict) else None


def summarize_action_run(repo: str, run: dict[str, object]) -> dict[str, object]:
    path = str(run.get("path") or "")
    status = str(run.get("status") or "")
    conclusion = run.get("conclusion")
    record: dict[str, object] = {
        "workflow": path,
        "head_sha": str(run.get("head_sha", "")).lower(),
        "status": status,
        "conclusion": conclusion if isinstance(conclusion, str) else "",
        "run_id": str(run.get("id", "")),
        "run_url": str(run.get("html_url", "")),
        "jobs": {},
        "plan": None,
    }
    if status != "completed" or conclusion != "success":
        return record
    if path not in {CI_WORKFLOW, REVIEW_WORKFLOW, FROZEN_WORKFLOW}:
        return record
    record["jobs"] = list_job_conclusions(repo, record["run_id"])
    if path == CI_WORKFLOW:
        record["plan"] = download_plan(repo, record["run_id"])
    return record


def lookup_predecessor(
    before: str, base: str, source_paths: list[str], head: str
) -> tuple[dict[str, object] | None, str]:
    """Read predecessor proof from GitHub. Any failure stays unproven."""
    try:
        return _lookup_predecessor(before, base, source_paths, head)
    except _LookupStop as stop:
        print("ci-proof-route: predecessor lookup was not admissible", file=sys.stderr)
        return None, stop.reason
    except Exception:  # noqa: BLE001 - a lookup crash must not skip lanes
        print("ci-proof-route: predecessor lookup was not admissible", file=sys.stderr)
        return None, "absent"


def _lookup_predecessor(
    before: str, base: str, source_paths: list[str], head: str
) -> tuple[dict[str, object] | None, str]:
    repo = os.environ.get("GITHUB_REPOSITORY", "").strip()
    if repo.count("/") != 1 or repo.startswith("/") or ".." in repo.split("/"):
        raise _LookupStop("absent")
    if not SHA.fullmatch(before):
        raise _LookupStop("absent")
    collected: list[object] = []
    queue = [before]
    seen: set[str] = set()
    while queue:
        sha = queue.pop(0)
        if sha in seen:
            continue
        if len(seen) >= MAX_FOLLOW:
            raise _LookupStop("ambiguous")
        seen.add(sha)
        action_runs = paged_items(
            f"/repos/{repo}/actions/runs",
            {"head_sha": sha},
            "workflow_runs",
        )
        checks = paged_items(
            f"/repos/{repo}/commits/{sha}/check-runs",
            {},
            "check_runs",
        )
        for check in checks:
            if not isinstance(check, dict) or check.get("name") != "review-readiness":
                continue
            collected.append(
                {
                    "workflow": REVIEW_CHECK,
                    "head_sha": sha,
                    "status": str(check.get("status") or ""),
                    "conclusion": str(check.get("conclusion") or ""),
                    "run_id": str(check.get("id", "")),
                    "run_url": str(check.get("html_url") or ""),
                }
            )
        for run in action_runs:
            if not isinstance(run, dict) or run.get("path") not in {
                CI_WORKFLOW,
                REVIEW_WORKFLOW,
                FROZEN_WORKFLOW,
            }:
                continue
            record = summarize_action_run(repo, run)
            collected.append(record)
            plan = record.get("plan")
            if not isinstance(plan, dict):
                continue
            identity = plan.get("proof_identity")
            if not isinstance(identity, dict):
                continue
            nxt = str(identity.get("proven_head", "")).lower()
            if SHA.fullmatch(nxt) and nxt not in seen:
                queue.append(nxt)
    reduced = reduce_observation(
        {
            "schema_id": OBSERVATION_SCHEMA_ID,
            "before": before,
            "base": base,
            "head": head,
            "source_paths": source_paths,
            "runs": collected,
        }
    )
    evidence = reduced.get("evidence")
    if not reduced.get("admitted") or not isinstance(evidence, dict):
        return None, str(reduced.get("reason") or "absent")
    return evidence, "success"


def ledger_gate(
    *,
    before: str,
    base: str,
    head: str,
    source_paths: list[str],
    successor_class: str | None,
    effective_class: str,
    evidence_path: str | None,
    delta_entries: list[tuple[str, str]] | None,
    allow_lookup: bool,
) -> tuple[bool, dict[str, object] | None, dict[str, str] | None]:
    if successor_class != "review_ledger_only" or effective_class not in {
        "source_bearing",
        "shared_impact",
    }:
        return False, None, None
    if not source_paths:
        return False, None, {"status": "rejected", "reason": "wrong_source"}
    preliminary: dict[str, object] | None = None
    reason = "absent"
    if evidence_path:
        preliminary = read_evidence_file(evidence_path)
        if preliminary is None:
            reason = "malformed"
    elif allow_lookup and os.environ.get("PROOF_LOOKUP_PREDECESSOR") == "1":
        preliminary, reason = lookup_predecessor(before, base, source_paths, head)
        if preliminary is not None and delta_entries is None:
            proven = str(preliminary.get("proven_head", "")).lower()
            delta_entries = movement_entries(proven, head)
    elif evidence_path is None and delta_entries is None and not allow_lookup:
        reason = "absent"
    if evidence_path and preliminary is not None and delta_entries is None and allow_lookup:
        proven = str(preliminary.get("proven_head", "")).lower()
        delta_entries = movement_entries(proven, head)
    if preliminary is None:
        return False, None, {"status": "rejected", "reason": reason}
    admitted, admit_reason = admit_recorded_proof(
        preliminary,
        before=before,
        base=base,
        source_paths=source_paths,
        delta_entries=delta_entries,
    )
    if admitted is None:
        return False, None, {"status": "rejected", "reason": admit_reason}
    return True, admitted, {"status": "admitted", "reason": "success"}


def plan_from_git() -> dict[str, object]:
    event = os.environ.get("PROOF_EVENT", os.environ.get("GITHUB_EVENT_NAME", "")).strip()
    head_completed = git("rev-parse", "HEAD")
    head = head_completed.stdout.strip() if head_completed.returncode == 0 else ""
    if event != "pull_request":
        return build_plan(event, "full_boundary", False, head, "", "", [], [])

    before_raw = os.environ.get("PROOF_BEFORE", "")
    before_state = predecessor_state(before_raw)
    before = before_raw.strip().lower()
    base_env = os.environ.get("PROOF_BASE_SHA", "").strip().lower()
    parents_completed = git("rev-list", "--parents", "-n", "1", "HEAD")
    if parents_completed.returncode != 0:
        return build_plan(event, "unknown", False, head, "", "", [], [])
    parents = parents_completed.stdout.strip().split()
    if not parents or any(not SHA.fullmatch(part) for part in parents):
        return build_plan(event, "unknown", False, head, "", "", [], [])
    if len(parents) >= 3:
        pr_head = parents[2]
        fallback_base = parents[1]
    else:
        pr_head = parents[0]
        fallback_base = ""
    if base_env and commit_exists(base_env):
        if fallback_base and base_env != fallback_base:
            return build_plan(event, "unknown", False, pr_head, base_env, "", [], [])
        base = base_env
    elif fallback_base and commit_exists(fallback_base):
        base = fallback_base
    else:
        return build_plan(event, "unknown", False, pr_head, "", "", [], [])

    successor_entries: list[tuple[str, str]] | None
    if before_state == "absent":
        successor_class = None
        successor_entries = None
        successor_before = ""
    elif before_state == "malformed" or not is_ancestor(before, pr_head):
        successor_class = "unknown"
        successor_entries = None
        successor_before = before if before_state == "sha" else ""
    else:
        successor_entries = diff_entries(before, pr_head)
        successor_class = classify_entries(successor_entries)
        successor_before = before

    merge_base = git("merge-base", base, pr_head)
    if merge_base.returncode != 0 or not SHA.fullmatch(merge_base.stdout.strip()):
        effective_entries = None
        effective_class = "unknown"
    else:
        effective_entries = diff_entries(merge_base.stdout.strip(), pr_head)
        effective_class = classify_entries(effective_entries)
    effective_paths = paths_of(effective_entries)
    admitted, identity, predecessor_proof = ledger_gate(
        before=successor_before,
        base=base,
        head=pr_head,
        source_paths=source_paths_from(effective_paths),
        successor_class=successor_class,
        effective_class=effective_class,
        evidence_path=os.environ.get("PROOF_PREDECESSOR_EVIDENCE"),
        delta_entries=None,
        allow_lookup=True,
    )
    class_name, narrow = decide(event, successor_class, effective_class, admitted)
    return build_plan(
        event,
        class_name,
        narrow,
        pr_head,
        base,
        successor_before,
        paths_of(successor_entries),
        effective_paths,
        identity,
        predecessor_proof,
    )


def flag(argv: list[str], name: str) -> str | None:
    if name not in argv:
        return None
    index = argv.index(name)
    if index + 1 >= len(argv):
        raise SystemExit(f"ci-proof-route: missing value for {name}")
    return argv[index + 1]


def class_from_file(path: str | None) -> tuple[str, list[str]]:
    if path is None:
        return "unknown", []
    text = Path(path).read_text(encoding="utf-8")
    parsed = parse_name_status(text)
    return classify_entries(parsed), paths_of(parsed)


def plan_from_fixture(argv: list[str]) -> dict[str, object]:
    event = flag(argv, "--event") or ""
    before = (flag(argv, "--before") or "").strip().lower()
    base = (flag(argv, "--base") or "").strip().lower()
    if event != "pull_request":
        return build_plan(event, "full_boundary", False, "fixture", base, before, [], [])
    if "--no-successor" in argv:
        successor_class: str | None = None
        successor_paths: list[str] = []
    else:
        successor_class, successor_paths = class_from_file(flag(argv, "--successor-file"))
    effective_class, effective_paths = class_from_file(flag(argv, "--effective-file"))
    delta_path = flag(argv, "--delta-file")
    if delta_path is None:
        delta_entries = None
    else:
        try:
            delta_entries = parse_name_status(Path(delta_path).read_text(encoding="utf-8"))
        except OSError:
            delta_entries = None
    admitted, identity, predecessor_proof = ledger_gate(
        before=before,
        base=base,
        head="fixture",
        source_paths=source_paths_from(effective_paths),
        successor_class=successor_class,
        effective_class=effective_class,
        evidence_path=flag(argv, "--predecessor-file"),
        delta_entries=delta_entries,
        allow_lookup=False,
    )
    class_name, narrow = decide(event, successor_class, effective_class, admitted)
    return build_plan(
        event,
        class_name,
        narrow,
        "fixture",
        base,
        before,
        successor_paths,
        effective_paths,
        identity,
        predecessor_proof,
    )


def emit(plan: dict[str, object]) -> None:
    text = json.dumps(plan, indent=2) + "\n"
    destination = os.environ.get("PROOF_PLAN_OUT", "proof-plan.json")
    Path(destination).write_text(text, encoding="utf-8", newline="\n")
    output = os.environ.get("GITHUB_OUTPUT")
    if output:
        narrow = "true" if plan["narrow"] else "false"
        with open(output, "a", encoding="utf-8", newline="\n") as handle:
            handle.write(f"narrow={narrow}\n")
            handle.write(f"class={plan['class']}\n")


def main(argv: list[str]) -> int:
    if "--print-contract" in argv:
        json.dump(contract(), sys.stdout)
        sys.stdout.write("\n")
        return 0
    if "--reduce-observation" in argv:
        path = flag(argv, "--reduce-observation") or ""
        try:
            payload = json.loads(Path(path).read_text(encoding="utf-8"))
            result = reduce_observation(payload)
        except (OSError, UnicodeError, json.JSONDecodeError):
            result = rejection("malformed")
        json.dump(result, sys.stdout)
        sys.stdout.write("\n")
        return 0
    try:
        plan = plan_from_fixture(argv) if "--fixture" in argv else plan_from_git()
    except Exception as error:  # noqa: BLE001 - fail wide, do not skip heavy lanes
        print(f"ci-proof-route: {error}", file=sys.stderr)
        event = os.environ.get("PROOF_EVENT", os.environ.get("GITHUB_EVENT_NAME", "")).strip()
        emit(build_plan(event, "unknown", False, "", "", "", [], []))
        return 1
    emit(plan)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
