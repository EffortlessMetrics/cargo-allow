#!/usr/bin/env python3
"""Discriminating controls for scripts/ci-proof-route.py (#4332).

Locks the lane plan, the ci.yml skip expressions, and the rule that the
four required contexts are never filtered. Stdlib only.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ROUTER = ROOT / "scripts" / "ci-proof-route.py"
CI = ROOT / ".github" / "workflows" / "ci.yml"

NARROW_IF_WITH_PREGATE = (
    "${{ !cancelled() && needs.pregate.result == 'success' && "
    "(needs.proof-route.result != 'success' || needs.proof-route.outputs.narrow != 'true') }}"
)
NARROW_IF_STANDALONE = (
    "${{ !cancelled() && (needs.proof-route.result != 'success' || "
    "needs.proof-route.outputs.narrow != 'true') }}"
)
ALWAYS_CI_JOBS = [
    "pregate",
    "test",
    "package-smoke",
    "cargo-deny",
    "release-impact",
    "release-rehearsal",
    "release-rehearsal-required",
    "proof-route",
]
RECORD_ONLY = ["review-readiness", "frozen-subject-lock"]
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
REQUIRED_CONTEXTS = ["pregate", "test", "review-readiness", "frozen-subject-lock"]
ON_BLOCK = """on:
  pull_request:
    # Full proof runs on drafts too; review-readiness.yml handles Ready events.
    # Avoid launching the same source proof twice on a draft-to-ready transition (#3753).
    types: [opened, synchronize, reopened]
  push:
    branches: [main]
  workflow_dispatch:
"""


def fail(message: str) -> None:
    raise SystemExit(message)


def run_router(args: list[str], env: dict[str, str] | None = None, cwd: Path | None = None) -> dict:
    plan_dir = Path(tempfile.mkdtemp())
    plan_path = plan_dir / "plan.json"
    merged = os.environ.copy()
    merged.pop("GITHUB_OUTPUT", None)
    if env:
        merged.update(env)
    merged["PROOF_PLAN_OUT"] = str(plan_path)
    completed = subprocess.run(
        [sys.executable, str(ROUTER), *args],
        cwd=str(cwd or ROOT),
        env=merged,
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    if completed.returncode != 0:
        fail(
            f"router exited {completed.returncode} for {args}:\n"
            f"{completed.stderr}\n{completed.stdout}"
        )
    if not plan_path.is_file():
        fail(f"router wrote no plan for {args}")
    return json.loads(plan_path.read_text(encoding="utf-8"))


def fixture(event: str, successor: str | None, effective: str | None) -> dict:
    with tempfile.TemporaryDirectory() as tmp:
        directory = Path(tmp)
        args = ["--fixture", "--event", event]
        if successor is None:
            args.append("--no-successor")
        else:
            path = directory / "successor.txt"
            path.write_text(successor, encoding="utf-8", newline="\n")
            args.extend(["--successor-file", str(path)])
        if effective is not None:
            path = directory / "effective.txt"
            path.write_text(effective, encoding="utf-8", newline="\n")
            args.extend(["--effective-file", str(path)])
        return run_router(args)


def assert_plan(name: str, plan: dict, class_name: str, narrow: bool) -> None:
    if plan["class"] != class_name or plan["narrow"] is not narrow:
        fail(f"{name}: got class={plan['class']} narrow={plan['narrow']}")
    decisions = {lane["id"]: lane["decision"] for lane in plan["lanes"]}
    for required in REQUIRED_CONTEXTS:
        if decisions.get(required) != "execute":
            fail(f"{name}: {required} decision is {decisions.get(required)}")
    for job in ("package-smoke", "cargo-deny", "release-impact", "release-rehearsal-required"):
        if decisions.get(job) != "execute":
            fail(f"{name}: {job} decision is {decisions.get(job)}")
    expected = "not_applicable" if narrow else "execute"
    for job in NARROW_WITH_PREGATE + NARROW_STANDALONE:
        if decisions.get(job) != expected:
            fail(f"{name}: {job} decision is {decisions.get(job)}, expected {expected}")
    if narrow:
        for job in NARROW_WITH_PREGATE + NARROW_STANDALONE:
            lane = next(item for item in plan["lanes"] if item["id"] == job)
            if lane["decision"] == "execute" or "not_applicable" != lane["decision"]:
                fail(f"{name}: pruned lane {job} is rendered as executed")
            if not str(lane["reason"]).startswith("narrow "):
                fail(f"{name}: {job} reason does not name the narrow class")


def check_classifications() -> None:
    ledger = "M\t.allow/review-dispositions/pr-4332-narrow-route.json\n"
    source = "M\tcrates/cargo-allow/src/lib.rs\n"
    docs = "M\tdocs/ci.md\n"
    cases = [
        (
            "ledger successor on a source pull request",
            "pull_request",
            ledger,
            source + ledger,
            "review_ledger_only",
            True,
        ),
        (
            "docs successor on a source pull request",
            "pull_request",
            docs,
            source + docs,
            "source_bearing",
            False,
        ),
        ("docs-only effective diff", "pull_request", None, docs, "docs_only", True),
        (
            "ledger-only effective diff",
            "pull_request",
            None,
            ledger,
            "review_ledger_only",
            True,
        ),
        ("source-bearing effective diff", "pull_request", None, source, "source_bearing", False),
        (
            "shared ledger and source",
            "pull_request",
            None,
            ledger + source,
            "shared_impact",
            False,
        ),
        (
            "shared ledger successor fails wide",
            "pull_request",
            ledger + source,
            docs,
            "shared_impact",
            False,
        ),
        ("rename fails wide", "pull_request", None, "R100\told.rs\tnew.rs\n", "unknown", False),
        ("copy fails wide", "pull_request", None, "C100\told.rs\tnew.rs\n", "unknown", False),
        ("empty diff fails wide", "pull_request", None, "", "unknown", False),
        (
            "malformed successor fails wide",
            "pull_request",
            "R100\told.rs\tnew.rs\n",
            docs,
            "unknown",
            False,
        ),
        (
            "other allow path is not ledger-only",
            "pull_request",
            None,
            "A\t.allow/revisions/note.toml\n",
            "source_bearing",
            False,
        ),
        (
            "workflow path is not docs-only",
            "pull_request",
            None,
            "M\t.github/workflows/ci.yml\n",
            "source_bearing",
            False,
        ),
        (
            "nested markdown outside docs is not docs-only",
            "pull_request",
            None,
            "M\tcrates/cargo-allow/README.md\n",
            "source_bearing",
            False,
        ),
        ("root markdown is docs-only", "pull_request", None, "M\tREADME.md\n", "docs_only", True),
        (
            "changelog fragment is docs-only",
            "pull_request",
            None,
            "A\t.changes/note.yaml\n",
            "docs_only",
            True,
        ),
        ("push is a full boundary", "push", None, docs, "full_boundary", False),
        (
            "workflow_dispatch is a full boundary",
            "workflow_dispatch",
            None,
            docs,
            "full_boundary",
            False,
        ),
        ("merge_group is a full boundary", "merge_group", None, ledger, "full_boundary", False),
        ("schedule is a full boundary", "schedule", None, docs, "full_boundary", False),
        ("release is a full boundary", "release", None, docs, "full_boundary", False),
        ("missing event is a full boundary", "", None, source, "full_boundary", False),
    ]
    for name, event, successor, effective, class_name, narrow in cases:
        plan = fixture(event, successor, effective)
        assert_plan(name, plan, class_name, narrow)


def check_contract() -> None:
    completed = subprocess.run(
        [sys.executable, str(ROUTER), "--print-contract"],
        cwd=str(ROOT),
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    if completed.returncode != 0:
        fail(completed.stderr)
    printed = json.loads(completed.stdout)
    expected = {
        "narrow_if_with_pregate": NARROW_IF_WITH_PREGATE,
        "narrow_if_standalone": NARROW_IF_STANDALONE,
        "always_ci_jobs": ALWAYS_CI_JOBS,
        "record_only": RECORD_ONLY,
        "narrow_with_pregate": NARROW_WITH_PREGATE,
        "narrow_standalone": NARROW_STANDALONE,
    }
    if printed != expected:
        fail(f"printed contract drifted from the locked control:\n{printed}")
    for required in ("pregate", "test"):
        if required not in printed["always_ci_jobs"]:
            fail(f"{required} is missing from always_ci_jobs")
        if required in printed["narrow_with_pregate"] or required in printed["narrow_standalone"]:
            fail(f"{required} is listed as narrow")


def job_bodies(text: str) -> dict[str, str]:
    _on, separator, body = text.partition("\njobs:\n")
    if not separator:
        fail("ci.yml has no jobs key")
    matches = list(re.finditer(r"(?m)^  ([a-z0-9-]+):\n", body))
    found: dict[str, str] = {}
    for index, match in enumerate(matches):
        start = match.end()
        end = matches[index + 1].start() if index + 1 < len(matches) else len(body)
        found[match.group(1)] = body[start:end]
    return found


def check_workflow() -> None:
    text = CI.read_text(encoding="utf-8")
    on_block, separator, _rest = text.partition("\njobs:\n")
    if not separator:
        fail("ci.yml has no jobs key")
    if on_block != ON_BLOCK:
        fail("ci.yml trigger block changed; this lane must not add paths or merge_group")
    if "paths:" in on_block:
        fail("ci.yml uses a workflow-level paths filter")
    jobs = job_bodies(text)
    classified = ALWAYS_CI_JOBS + NARROW_WITH_PREGATE + NARROW_STANDALONE
    if set(jobs) != set(classified):
        missing = sorted(set(jobs) - set(classified))
        extra = sorted(set(classified) - set(jobs))
        fail(f"lane authority does not match ci.yml jobs; missing={missing} extra={extra}")
    for job in NARROW_WITH_PREGATE:
        body = jobs[job]
        if "needs: [pregate, proof-route]" not in body or NARROW_IF_WITH_PREGATE not in body:
            fail(f"{job} does not keep the pregate gate and the narrow guard")
        if body.find("needs:") > body.find("steps:"):
            fail(f"{job} declares needs after steps")
    for job in NARROW_STANDALONE:
        body = jobs[job]
        if "needs: proof-route" not in body or NARROW_IF_STANDALONE not in body:
            fail(f"{job} does not use the standalone narrow guard")
        if "needs: [pregate" in body or "needs: pregate" in body:
            fail(f"{job} gained a pregate dependency")
    for job in ALWAYS_CI_JOBS:
        if job == "proof-route":
            continue
        if "proof-route" in jobs[job] or "outputs.narrow" in jobs[job]:
            fail(f"{job} reads the narrow plan; required and release lanes must not")
    route = jobs["proof-route"]
    test_at = route.find("run: python3 scripts/test-ci-proof-route.py")
    plan_at = route.find("run: python3 scripts/ci-proof-route.py")
    if test_at < 0 or plan_at < 0 or test_at > plan_at:
        fail("proof-route must run its controls before publishing a narrow decision")
    if "check --mode no-new" not in jobs["test"] or "--require-change-note" not in jobs["test"]:
        fail("test no longer runs the unconditional no-new and change-note guards")
    for relative in (
        ".github/workflows/review-readiness.yml",
        ".github/workflows/frozen-subject-lock.yml",
    ):
        workflow = (ROOT / relative).read_text(encoding="utf-8")
        on_text = workflow.split("\njobs:\n", 1)[0]
        if "paths:" in on_text or "proof-route" in workflow:
            fail(f"{relative} gained a path filter or a second routing registry")


def git(repo: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=str(repo),
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    if completed.returncode != 0:
        fail(f"git {' '.join(args)} failed: {completed.stderr}")
    return completed.stdout.strip()


def write_repo(repo: Path, relative: str, content: str) -> None:
    path = repo / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8", newline="\n")


def check_git_history() -> None:
    repo = Path(tempfile.mkdtemp())
    git(repo, "init", "-b", "main")
    git(repo, "config", "user.email", "ci-proof-route@example.invalid")
    git(repo, "config", "user.name", "ci-proof-route")
    git(repo, "config", "core.autocrlf", "false")
    write_repo(repo, "README.md", "base\n")
    git(repo, "add", "README.md")
    git(repo, "commit", "-m", "base")
    base = git(repo, "rev-parse", "HEAD")
    write_repo(repo, "crates/cargo-allow/src/lib.rs", "fn main() {}\n")
    git(repo, "add", "crates/cargo-allow/src/lib.rs")
    git(repo, "commit", "-m", "source")
    source = git(repo, "rev-parse", "HEAD")
    write_repo(
        repo,
        ".allow/review-dispositions/pr-4332-narrow-route.json",
        "{}\n",
    )
    git(repo, "add", ".allow/review-dispositions/pr-4332-narrow-route.json")
    git(repo, "commit", "-m", "ledger")
    head = git(repo, "rev-parse", "HEAD")
    plan = run_router(
        [],
        env={
            "PROOF_EVENT": "pull_request",
            "PROOF_BEFORE": source,
            "PROOF_BASE_SHA": base,
        },
        cwd=repo,
    )
    assert_plan("git ledger successor", plan, "review_ledger_only", True)
    if plan["head"] != head:
        fail(f"git ledger successor classified {plan['head']} instead of {head}")
    if "crates/cargo-allow/src/lib.rs" not in plan["effective_paths"]:
        fail("git ledger successor lost the source-bearing effective diff")
    if plan["successor_paths"] != [".allow/review-dispositions/pr-4332-narrow-route.json"]:
        fail(f"git successor paths are {plan['successor_paths']}")

    docs_repo = Path(tempfile.mkdtemp())
    git(docs_repo, "init", "-b", "main")
    git(docs_repo, "config", "user.email", "ci-proof-route@example.invalid")
    git(docs_repo, "config", "user.name", "ci-proof-route")
    git(docs_repo, "config", "core.autocrlf", "false")
    write_repo(docs_repo, "README.md", "base\n")
    git(docs_repo, "add", "README.md")
    git(docs_repo, "commit", "-m", "base")
    docs_base = git(docs_repo, "rev-parse", "HEAD")
    write_repo(docs_repo, "docs/ci.md", "docs\n")
    git(docs_repo, "add", "docs/ci.md")
    git(docs_repo, "commit", "-m", "docs")
    docs_plan = run_router(
        [],
        env={
            "PROOF_EVENT": "pull_request",
            "PROOF_BEFORE": "0" * 40,
            "PROOF_BASE_SHA": docs_base,
        },
        cwd=docs_repo,
    )
    assert_plan("git docs-only open", docs_plan, "docs_only", True)

    wide = run_router(
        [],
        env={"PROOF_EVENT": "merge_group", "PROOF_BEFORE": source, "PROOF_BASE_SHA": base},
        cwd=repo,
    )
    assert_plan("git merge_group", wide, "full_boundary", False)


def main() -> None:
    check_classifications()
    check_contract()
    check_workflow()
    check_git_history()
    print("ci-proof-route controls passed")


if __name__ == "__main__":
    main()
