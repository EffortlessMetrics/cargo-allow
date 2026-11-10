#!/usr/bin/env python3
"""Lane applicability for #2569 and #4332.

This is the one source-controlled plan for which CI lanes execute.
``scripts/feature-config-routing.py`` stays the row selector inside
``feature-configuration-proof`` when that lane runs. Release-impact
classification stays in the release graph. This script does not replace
either of those.

A ``pull_request`` run is narrow only in two cases:

- the successor delta (``before``..head, when ``before`` is a real
  ancestor) is entirely under ``.allow/review-dispositions/``
- there is no such successor, or the successor is docs-only, and the
  effective diff (merge-base of the PR base..head) is provably docs-only

A docs-only successor on a source-bearing pull request stays wide: docs
routing uses the effective diff, not the last commit. Ledger routing uses
the successor so a review-ledger commit does not replay the heavy matrix.

Unknown, empty, rename/copy, malformed, and shared ledger impact stay
wide. ``push``, ``workflow_dispatch``, ``merge_group``, ``schedule``,
``release``, and every other event stay wide, including before the
merge-group trigger exists. Planner failure writes ``narrow=false`` and
exits non-zero so heavy lanes still run.

The ruleset contexts (pregate, test, review-readiness,
frozen-subject-lock), package-smoke, cargo-deny, and the release graph
always execute. Heavy non-required lanes are NotApplicable only when this
plan succeeds with ``narrow=true``; GitHub then skips those jobs. A skip
is not a green execution.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

SCHEMA_ID = "cargo-allow.ci-proof-plan.v1"
LEDGER_PREFIX = ".allow/review-dispositions/"
SHA = re.compile(r"^[0-9a-f]{40}([0-9a-f]{24})?$")
MAX_PATHS = 2000

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
    if normalized.startswith("docs/") or normalized.startswith(".changes/"):
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


def decide(event: str, successor_class: str | None, effective_class: str) -> tuple[str, bool]:
    """Return ``(class, narrow)``.

    ``successor_class is None`` means the event has no predecessor (a pull
    request open, or a zero ``before``). A docs-only successor does not
    narrow a source-bearing effective diff.
    """
    if event != "pull_request":
        return "full_boundary", False
    if successor_class == "review_ledger_only":
        return "review_ledger_only", True
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
) -> dict[str, object]:
    return {
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


def paths_of(entries: list[tuple[str, str]] | None) -> list[str]:
    if not entries:
        return []
    return [path for _status, path in entries]


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
    class_name, narrow = decide(event, successor_class, effective_class)
    return build_plan(
        event,
        class_name,
        narrow,
        pr_head,
        base,
        successor_before,
        paths_of(successor_entries),
        paths_of(effective_entries),
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
    if event != "pull_request":
        return build_plan(event, "full_boundary", False, "fixture", "", "", [], [])
    if "--no-successor" in argv:
        successor_class: str | None = None
        successor_paths: list[str] = []
    else:
        successor_class, successor_paths = class_from_file(flag(argv, "--successor-file"))
    effective_class, effective_paths = class_from_file(flag(argv, "--effective-file"))
    class_name, narrow = decide(event, successor_class, effective_class)
    return build_plan(
        event,
        class_name,
        narrow,
        "fixture",
        "",
        "",
        successor_paths,
        effective_paths,
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
