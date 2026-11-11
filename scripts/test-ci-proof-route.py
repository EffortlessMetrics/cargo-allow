#!/usr/bin/env python3
"""Discriminating controls for scripts/ci-proof-route.py (#4332).

Locks the lane plan, the ci.yml skip expressions, and the rule that the
four required contexts are never filtered. Stdlib only.
"""

from __future__ import annotations

import importlib.util
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
TEST_ONLY_SKIP = [
    "product-candidates-interop",
    "operator-latency",
    "integrated-dogfood",
    "compat-delegation",
]
TEST_ONLY_IF_WITH_PREGATE = (
    "${{ !cancelled() && needs.pregate.result == 'success' && "
    "(needs.proof-route.result != 'success' || (needs.proof-route.outputs.narrow != 'true' && "
    "needs.proof-route.outputs.class != 'test_only')) }}"
)
PACKAGE_SMOKE_IF = (
    "${{ !cancelled() && needs.pregate.result == 'success' && "
    "(needs.proof-route.result != 'success' || (needs.proof-route.outputs.class != 'docs_only' && "
    "needs.proof-route.outputs.class != 'test_only')) }}"
)
TEST_IF = (
    "${{ !cancelled() && needs.pregate.result == 'success' && "
    "(needs.package-smoke.result == 'success' || needs.package-smoke.result == 'skipped') }}"
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
  # Required pregate and test contexts must also run on a merge queue.
  # This does not enable a queue or add a bypass. checks_requested is the
  # only merge_group activity.
  merge_group:
    types: [checks_requested]
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
    merged.pop("PROOF_LOOKUP_PREDECESSOR", None)
    merged.pop("PROOF_PREDECESSOR_EVIDENCE", None)
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


BEFORE = "b" * 40
BASE = "c" * 40
PROVEN = "d" * 40
LEDGER_DELTA = "M\t.allow/review-dispositions/pr-4332-narrow-route.json\n"


def fixture(
    event: str,
    successor: str | None,
    effective: str | None,
    *,
    before: str = "",
    base: str = "",
    evidence: dict | None = None,
    delta: str | None = None,
) -> dict:
    with tempfile.TemporaryDirectory() as tmp:
        directory = Path(tmp)
        args = ["--fixture", "--event", event]
        if before:
            args.extend(["--before", before])
        if base:
            args.extend(["--base", base])
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
        if evidence is not None:
            path = directory / "evidence.json"
            path.write_text(json.dumps(evidence), encoding="utf-8", newline="\n")
            args.extend(["--predecessor-file", str(path)])
        if delta is not None:
            path = directory / "delta.txt"
            path.write_text(delta, encoding="utf-8", newline="\n")
            args.extend(["--delta-file", str(path)])
        return run_router(args)


def proof_evidence(**overrides: object) -> dict:
    evidence: dict[str, object] = {
        "schema_id": "cargo-allow.ci-predecessor-proof.v1",
        "predecessor": BEFORE,
        "proven_head": PROVEN,
        "base": BASE,
        "source_paths": ["crates/cargo-allow/src/lib.rs"],
        "status": "success",
        "run_id": "1",
        "run_url": "https://github.com/EffortlessMetrics/cargo-allow/actions/runs/1",
    }
    evidence.update(overrides)
    return evidence


def assert_plan(name: str, plan: dict, class_name: str, narrow: bool) -> None:
    if plan["class"] != class_name or plan["narrow"] is not narrow:
        fail(f"{name}: got class={plan['class']} narrow={plan['narrow']}")
    decisions = {lane["id"]: lane["decision"] for lane in plan["lanes"]}
    for required in REQUIRED_CONTEXTS:
        if decisions.get(required) != "execute":
            fail(f"{name}: {required} decision is {decisions.get(required)}")
    for job in ("cargo-deny", "release-impact", "release-rehearsal-required"):
        if decisions.get(job) != "execute":
            fail(f"{name}: {job} decision is {decisions.get(job)}")
    package_smoke = "not_applicable" if class_name in {"docs_only", "test_only"} else "execute"
    if decisions.get("package-smoke") != package_smoke:
        fail(f"{name}: package-smoke decision is {decisions.get('package-smoke')}, expected {package_smoke}")
    for job in NARROW_WITH_PREGATE + NARROW_STANDALONE:
        if narrow:
            expected = "not_applicable"
        elif class_name == "test_only" and job in TEST_ONLY_SKIP:
            expected = "not_applicable"
        else:
            expected = "execute"
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
            "unproven_predecessor",
            False,
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
        (
            "operator-latency schema is not docs-only",
            "pull_request",
            None,
            "M\tdocs/schemas/operator-latency.v4.schema.json\n",
            "source_bearing",
            False,
        ),
        (
            "schema beside genuine docs stays wide",
            "pull_request",
            None,
            "M\tdocs/schemas/operator-latency.v4.schema.json\n" + docs,
            "source_bearing",
            False,
        ),
        (
            "schema directory readme stays wide",
            "pull_request",
            None,
            "M\tdocs/schemas/README.md\n",
            "source_bearing",
            False,
        ),
        (
            "ledger successor with an unreadable effective diff stays wide",
            "pull_request",
            ledger,
            "R100\told.rs\tnew.rs\n",
            "unknown",
            False,
        ),
        (
            "quoted path fails wide",
            "pull_request",
            None,
            '"M"\tdocs/ci.md\n',
            "unknown",
            False,
        ),
        (
            "schema successor on a docs pull request stays wide",
            "pull_request",
            "M\tdocs/schemas/operator-latency.v4.schema.json\n",
            docs,
            "source_bearing",
            False,
        ),
        (
            "crate tests are test_only",
            "pull_request",
            None,
            "M\tcrates/cargo-allow/tests/route.rs\n",
            "test_only",
            False,
        ),
        (
            "test script is test_only",
            "pull_request",
            None,
            "M\tscripts/test-ci-proof-route.py\n",
            "test_only",
            False,
        ),
        (
            "fixture tree is test_only",
            "pull_request",
            None,
            "M\ttests/fixtures/repo-edit/sample.toml\n"
            "M\tfixtures/unsafe/src/lib.rs\n",
            "test_only",
            False,
        ),
        (
            "hawk boundary fixture stays source",
            "pull_request",
            None,
            "M\ttests/fixtures/proof-adapter-hawk/parity-boundary-v1.toml\n",
            "source_bearing",
            False,
        ),
        (
            "ripr boundary fixture stays source",
            "pull_request",
            None,
            "M\ttests/fixtures/proof-adapter-ripr/parity-boundary-v1.toml\n",
            "source_bearing",
            False,
        ),
        (
            "cargo-proof plan fixture stays source",
            "pull_request",
            None,
            "M\ttests/fixtures/cargo-proof/proof-plan-smoke-v1.toml\n",
            "source_bearing",
            False,
        ),
        (
            "cargo-proof obligation fixture stays source",
            "pull_request",
            None,
            "M\ttests/fixtures/cargo-proof/intent-obligation-plan-smoke-v1.json\n",
            "source_bearing",
            False,
        ),
        (
            "dogfood stage fixture stays source",
            "pull_request",
            None,
            "M\ttests/fixtures/three-product-dogfood/pipeline-stages-v1.toml\n",
            "source_bearing",
            False,
        ),
        (
            "package byte authority stays source",
            "pull_request",
            None,
            "M\tscripts/test-package-byte-authority.py\n",
            "source_bearing",
            False,
        ),
        (
            "perf budget smoke stays source",
            "pull_request",
            None,
            "M\tscripts/test-perf-budget-smoke.sh\n",
            "source_bearing",
            False,
        ),
        (
            "exact candidate package candidate stays source",
            "pull_request",
            None,
            "M\tscripts/test-exact-candidate-package-candidate.py\n",
            "source_bearing",
            False,
        ),
        (
            "exact candidate staged packaging stays source",
            "pull_request",
            None,
            "M\tscripts/test-exact-candidate-staged-packaging.py\n",
            "source_bearing",
            False,
        ),
        (
            "exact candidate index checksum stays source",
            "pull_request",
            None,
            "M\tscripts/test-exact-candidate-index-checksum.py\n",
            "source_bearing",
            False,
        ),
        (
            "exact candidate isolated install stays source",
            "pull_request",
            None,
            "M\tscripts/test-exact-candidate-isolated-install.py\n",
            "source_bearing",
            False,
        ),
        (
            "exact candidate install journey stays source",
            "pull_request",
            None,
            "M\tscripts/test-exact-candidate-install-journey.sh\n",
            "source_bearing",
            False,
        ),
        (
            "exact candidate install journey py stays source",
            "pull_request",
            None,
            "M\tscripts/test-exact-candidate-install-journey.py\n",
            "source_bearing",
            False,
        ),
        (
            "exact candidate package identity stays source",
            "pull_request",
            None,
            "M\tscripts/test-exact-candidate-package-identity.py\n",
            "source_bearing",
            False,
        ),
        (
            "exact upgrade rollback journey stays source",
            "pull_request",
            None,
            "M\tscripts/test-exact-upgrade-rollback-journey.sh\n",
            "source_bearing",
            False,
        ),
        (
            "release binary contract stays source",
            "pull_request",
            None,
            "M\tscripts/test-release-binary.sh\n",
            "source_bearing",
            False,
        ),
        (
            "linux gnu candidate stays source",
            "pull_request",
            None,
            "M\tscripts/test-linux-gnu-candidate.py\n",
            "source_bearing",
            False,
        ),
        (
            "final package docs stays source",
            "pull_request",
            None,
            "M\tscripts/test-final-package-docs.py\n",
            "source_bearing",
            False,
        ),
        (
            "release topology publisher stays source",
            "pull_request",
            None,
            "M\tscripts/test-release-topology-publisher.py\n",
            "source_bearing",
            False,
        ),
        (
            "qualify worklist pages stays source",
            "pull_request",
            None,
            "M\tscripts/test-qualify-worklist-pages.py\n",
            "source_bearing",
            False,
        ),
        (
            "exact candidate interop characterization stays source",
            "pull_request",
            None,
            "M\tscripts/test-exact-candidate-interop-smoke.sh\n",
            "source_bearing",
            False,
        ),
        (
            "delegation test stays source",
            "pull_request",
            None,
            "M\tcrates/cargo-allow/tests/intent_staged_precommit_delegate.rs\n",
            "source_bearing",
            False,
        ),
        (
            "compat fixtures stay source",
            "pull_request",
            None,
            "M\ttests/compat/fixtures/sample.toml\n",
            "source_bearing",
            False,
        ),
        (
            "nested compat fixtures stay source",
            "pull_request",
            None,
            "M\ttests/compat/fixtures/nested/sample.toml\n",
            "source_bearing",
            False,
        ),
        (
            "undecided test script stays source",
            "pull_request",
            None,
            "M\tscripts/test-not-a-real-decision.py\n",
            "source_bearing",
            False,
        ),
        (
            "test plus source stays wide",
            "pull_request",
            None,
            "M\tcrates/cargo-allow/tests/route.rs\nM\tcrates/cargo-allow/src/lib.rs\n",
            "source_bearing",
            False,
        ),
        (
            "unit test inside src stays wide",
            "pull_request",
            None,
            "M\tcrates/cargo-allow/src/refresh_fixtures.rs\n",
            "source_bearing",
            False,
        ),
        (
            "docs fixture payload stays source",
            "pull_request",
            None,
            "M\tdocs/dogfood/fixtures/sample.json\n",
            "source_bearing",
            False,
        ),
        (
            "renamed test file stays wide",
            "pull_request",
            None,
            "R100\tcrates/cargo-allow/tests/old.rs\tcrates/cargo-allow/tests/new.rs\n",
            "unknown",
            False,
        ),
        (
            "push of tests stays wide",
            "push",
            None,
            "M\tcrates/cargo-allow/tests/route.rs\n",
            "full_boundary",
            False,
        ),
        (
            "test successor on a source pull request stays wide",
            "pull_request",
            "M\tcrates/cargo-allow/tests/route.rs\n",
            "M\tcrates/cargo-allow/src/lib.rs\nM\tcrates/cargo-allow/tests/route.rs\n",
            "source_bearing",
            False,
        ),
    ]
    for name, event, successor, effective, class_name, narrow in cases:
        plan = fixture(event, successor, effective)
        assert_plan(name, plan, class_name, narrow)
        for required in REQUIRED_CONTEXTS:
            decision = next(lane["decision"] for lane in plan["lanes"] if lane["id"] == required)
            if decision != "execute":
                fail(f"{name}: {required} disappeared from the plan")

    admitted = fixture(
        "pull_request",
        ledger,
        source + ledger,
        before=BEFORE,
        base=BASE,
        evidence=proof_evidence(),
        delta=LEDGER_DELTA,
    )
    assert_plan(
        "ledger successor reuses exact predecessor proof",
        admitted,
        "review_ledger_only",
        True,
    )
    if admitted.get("proof_identity", {}).get("proven_head") != PROVEN:
        fail("admitted plan did not keep the predecessor proof identity")
    unproven = fixture("pull_request", ledger, source + ledger)
    if "proof_identity" in unproven:
        fail("unproven ledger successor stored proof identity")
    if unproven.get("predecessor_proof", {}).get("reason") != "absent":
        fail("missing predecessor proof was treated as reusable")

    rejected = [
        ("failed predecessor", proof_evidence(status="failed"), LEDGER_DELTA, "failed"),
        ("pending predecessor", proof_evidence(status="pending"), LEDGER_DELTA, "pending"),
        ("cancelled predecessor", proof_evidence(status="cancelled"), LEDGER_DELTA, "cancelled"),
        ("skipped predecessor", proof_evidence(status="skipped"), LEDGER_DELTA, "skipped"),
        ("stale predecessor", proof_evidence(predecessor="e" * 40), LEDGER_DELTA, "stale"),
        ("base-moved predecessor", proof_evidence(base="e" * 40), LEDGER_DELTA, "base_moved"),
        (
            "wrong-source predecessor",
            proof_evidence(source_paths=["docs/ci.md"]),
            LEDGER_DELTA,
            "wrong_source",
        ),
        (
            "predecessor movement is not ledger-only",
            proof_evidence(),
            "M\tcrates/cargo-allow/src/lib.rs\n",
            "wrong_source",
        ),
        ("malformed predecessor evidence", {"schema_id": "nope"}, LEDGER_DELTA, "malformed"),
    ]
    for name, evidence, delta, reason in rejected:
        plan = fixture(
            "pull_request",
            ledger,
            source + ledger,
            before=BEFORE,
            base=BASE,
            evidence=evidence,
            delta=delta,
        )
        assert_plan(name, plan, "unproven_predecessor", False)
        if plan.get("predecessor_proof", {}).get("reason") != reason:
            fail(f"{name}: reason {plan.get('predecessor_proof')}, expected {reason}")
        if "proof_identity" in plan:
            fail(f"{name}: rejected proof was reused")
    missing_delta = fixture(
        "pull_request",
        ledger,
        source + ledger,
        before=BEFORE,
        base=BASE,
        evidence=proof_evidence(),
    )
    assert_plan("predecessor without movement proof", missing_delta, "unproven_predecessor", False)
    if missing_delta.get("predecessor_proof", {}).get("reason") != "absent":
        fail("unverifiable predecessor movement was reused")


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
        "test_only_if_with_pregate": TEST_ONLY_IF_WITH_PREGATE,
        "test_only_skip": TEST_ONLY_SKIP,
        "package_smoke_if": PACKAGE_SMOKE_IF,
        "package_smoke_skip_classes": ["docs_only", "test_only"],
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
        fail("ci.yml trigger block drifted; keep merge_group and do not add a paths filter")
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
        expected_if = TEST_ONLY_IF_WITH_PREGATE if job in TEST_ONLY_SKIP else NARROW_IF_WITH_PREGATE
        if "needs: [pregate, proof-route]" not in body or expected_if not in body:
            fail(f"{job} does not keep the pregate gate and its narrow guard")
        if job in TEST_ONLY_SKIP and NARROW_IF_WITH_PREGATE in body:
            fail(f"{job} still uses only the docs-only narrow guard")
        if job not in TEST_ONLY_SKIP and "class != 'test_only'" in body:
            fail(f"{job} would skip on a test_only plan")
        if body.find("needs:") > body.find("steps:"):
            fail(f"{job} declares needs after steps")
    for job in NARROW_STANDALONE:
        body = jobs[job]
        if "needs: proof-route" not in body or NARROW_IF_STANDALONE not in body:
            fail(f"{job} does not use the standalone narrow guard")
        if "needs: [pregate" in body or "needs: pregate" in body:
            fail(f"{job} gained a pregate dependency")
    for job in ALWAYS_CI_JOBS:
        if job in {"proof-route", "package-smoke"}:
            continue
        if "proof-route" in jobs[job] or "outputs.narrow" in jobs[job]:
            fail(f"{job} reads the narrow plan; required and release lanes must not")
    smoke = jobs["package-smoke"]
    if "needs: [pregate, proof-route]" not in smoke or PACKAGE_SMOKE_IF not in smoke:
        fail("package-smoke is not conditional on docs_only and test_only")
    if "outputs.narrow" in smoke:
        fail("package-smoke must key off class, not the full narrow flag")
    if TEST_IF not in jobs["test"]:
        fail("test still skips when package-smoke is skipped")
    if "needs.package-smoke.result == 'success'" not in jobs["test"]:
        fail("test restores cutover receipts when package-smoke did not run")
    route = jobs["proof-route"]
    test_at = route.find("run: python3 scripts/test-ci-proof-route.py")
    plan_at = route.find("run: python3 scripts/ci-proof-route.py")
    if test_at < 0 or plan_at < 0 or test_at > plan_at:
        fail("proof-route must run its controls before publishing a narrow decision")
    for scope in ("actions: read", "checks: read", "contents: read"):
        if scope not in route:
            fail(f"proof-route is missing {scope}; predecessor lookup must stay read-only")
    controls_end = route.find("name: Routing controls")
    classify_at = route.find("name: Classify the lane plan")
    if controls_end < 0 or classify_at < 0:
        fail("proof-route lost its control or classify step")
    if "PROOF_LOOKUP_PREDECESSOR" in route[controls_end:classify_at]:
        fail("routing controls must not look up predecessor proof")
    if 'PROOF_LOOKUP_PREDECESSOR: "1"' not in route[classify_at:]:
        fail("classify step must opt in to predecessor lookup")
    if "needs.pregate.result == 'success'" not in NARROW_IF_WITH_PREGATE:
        fail("a pregate failure must not become a narrow route")
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
    assert_plan("git ledger successor", plan, "unproven_predecessor", False)
    if plan.get("predecessor_proof", {}).get("reason") != "absent":
        fail("git ledger successor reused missing predecessor proof")
    if "proof_identity" in plan:
        fail("git ledger successor stored proof identity without proof")
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

    source_paths = [
        path for path in plan["effective_paths"] if not path.startswith(".allow/review-dispositions/")
    ]
    evidence_dir = Path(tempfile.mkdtemp())
    evidence_path = evidence_dir / "evidence.json"
    evidence_path.write_text(
        json.dumps(
            {
                "schema_id": "cargo-allow.ci-predecessor-proof.v1",
                "predecessor": source,
                "proven_head": source,
                "base": base,
                "source_paths": source_paths,
                "status": "success",
                "run_id": "1",
                "run_url": "https://github.com/EffortlessMetrics/cargo-allow/actions/runs/1",
            }
        ),
        encoding="utf-8",
        newline="\n",
    )
    reused = run_router(
        [],
        env={
            "PROOF_EVENT": "pull_request",
            "PROOF_BEFORE": source,
            "PROOF_BASE_SHA": base,
            "PROOF_PREDECESSOR_EVIDENCE": str(evidence_path),
        },
        cwd=repo,
    )
    assert_plan("git exact predecessor reuse", reused, "review_ledger_only", True)
    if reused.get("proof_identity", {}).get("proven_head") != source:
        fail("git reuse did not preserve the proven head")
    if reused.get("proof_identity", {}).get("source_paths") != source_paths:
        fail("git reuse changed the source identity")

    stale = dict(json.loads(evidence_path.read_text(encoding="utf-8")))
    stale["predecessor"] = base
    stale_path = evidence_dir / "stale.json"
    stale_path.write_text(json.dumps(stale), encoding="utf-8", newline="\n")
    stale_plan = run_router(
        [],
        env={
            "PROOF_EVENT": "pull_request",
            "PROOF_BEFORE": source,
            "PROOF_BASE_SHA": base,
            "PROOF_PREDECESSOR_EVIDENCE": str(stale_path),
        },
        cwd=repo,
    )
    assert_plan("git stale predecessor", stale_plan, "unproven_predecessor", False)

    refused = run_router(
        [],
        env={
            "PROOF_EVENT": "pull_request",
            "PROOF_BEFORE": source,
            "PROOF_BASE_SHA": base,
            "PROOF_LOOKUP_PREDECESSOR": "1",
            "GITHUB_TOKEN": "not-a-real-token",
            "GITHUB_REPOSITORY": "EffortlessMetrics/cargo-allow",
            "GITHUB_API_URL": "http://127.0.0.1:9",
        },
        cwd=repo,
    )
    assert_plan("git lookup without https is not proof", refused, "unproven_predecessor", False)
    if "not-a-real-token" in json.dumps(refused):
        fail("predecessor lookup copied the token into the plan")

    merge_repo = Path(tempfile.mkdtemp())
    git(merge_repo, "init", "-b", "main")
    git(merge_repo, "config", "user.email", "ci-proof-route@example.invalid")
    git(merge_repo, "config", "user.name", "ci-proof-route")
    git(merge_repo, "config", "core.autocrlf", "false")
    write_repo(merge_repo, "README.md", "base\n")
    git(merge_repo, "add", "README.md")
    git(merge_repo, "commit", "-m", "base")
    merge_base = git(merge_repo, "rev-parse", "HEAD")
    git(merge_repo, "checkout", "-b", "pr")
    write_repo(merge_repo, "crates/cargo-allow/src/lib.rs", "fn main() {}\n")
    git(merge_repo, "add", "crates/cargo-allow/src/lib.rs")
    git(merge_repo, "commit", "-m", "source")
    pr_tip = git(merge_repo, "rev-parse", "HEAD")
    git(merge_repo, "checkout", "main")
    git(merge_repo, "merge", "--no-ff", "pr", "-m", "merge")
    parents = git(merge_repo, "rev-list", "--parents", "-n", "1", "HEAD").split()
    if len(parents) < 3 or parents[1] != merge_base or parents[2] != pr_tip:
        fail(f"merge --no-ff parents were {parents}")
    merged = run_router(
        [],
        env={
            "PROOF_EVENT": "pull_request",
            "PROOF_BEFORE": "0" * 40,
            "PROOF_BASE_SHA": merge_base,
        },
        cwd=merge_repo,
    )
    assert_plan("git merge commit source", merged, "source_bearing", False)
    if merged["head"] != pr_tip:
        fail(f"merge commit classified {merged['head']} instead of the pull request head")
    mismatched = run_router(
        [],
        env={
            "PROOF_EVENT": "pull_request",
            "PROOF_BEFORE": "0" * 40,
            "PROOF_BASE_SHA": pr_tip,
        },
        cwd=merge_repo,
    )
    assert_plan("git base mismatch", mismatched, "unknown", False)
    git(merge_repo, "checkout", "-b", "side", merge_base)
    write_repo(merge_repo, "side.txt", "side\n")
    git(merge_repo, "add", "side.txt")
    git(merge_repo, "commit", "-m", "side")
    side = git(merge_repo, "rev-parse", "HEAD")
    git(merge_repo, "checkout", "main")
    unrelated = run_router(
        [],
        env={
            "PROOF_EVENT": "pull_request",
            "PROOF_BEFORE": side,
            "PROOF_BASE_SHA": merge_base,
        },
        cwd=merge_repo,
    )
    assert_plan("git non-ancestor before", unrelated, "unknown", False)
    malformed = run_router(
        [],
        env={
            "PROOF_EVENT": "pull_request",
            "PROOF_BEFORE": "not-a-sha",
            "PROOF_BASE_SHA": merge_base,
        },
        cwd=merge_repo,
    )
    assert_plan("git malformed before", malformed, "unknown", False)
    if malformed["head"] != pr_tip:
        fail("malformed before moved the classified head")


CI_JOBS = ALWAYS_CI_JOBS + NARROW_WITH_PREGATE + NARROW_STANDALONE
SOURCE_PATHS = ["crates/cargo-allow/src/lib.rs"]


def reduce_observation(observation: dict) -> dict:
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "observation.json"
        path.write_text(json.dumps(observation), encoding="utf-8", newline="\n")
        completed = subprocess.run(
            [sys.executable, str(ROUTER), "--reduce-observation", str(path)],
            cwd=str(ROOT),
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
    if completed.returncode != 0:
        fail(f"reduce exited {completed.returncode}: {completed.stderr}")
    return json.loads(completed.stdout)


def wide_observation(sha: str, **changes: object) -> dict:
    jobs = {job: "success" for job in CI_JOBS}
    job_changes = changes.pop("ci_jobs", None)
    if isinstance(job_changes, dict):
        jobs.update(job_changes)
    narrow = bool(changes.pop("narrow", False))
    identity = changes.pop("identity", None)
    plan_paths = changes.pop("plan_paths", SOURCE_PATHS)
    plan = {
        "narrow": narrow,
        "head": sha,
        "base": changes.pop("plan_base", BASE),
        "effective_paths": plan_paths,
    }
    if identity is not None:
        plan["proof_identity"] = identity
    if changes.pop("omit_plan", False):
        plan_value = None
    else:
        plan_value = plan
    publisher = str(changes.pop("publisher", "success"))
    frozen = str(changes.pop("frozen", "success"))
    review = str(changes.pop("review", "neutral"))
    runs = [
        {
            "workflow": ".github/workflows/ci.yml",
            "head_sha": sha,
            "status": changes.pop("status", "completed"),
            "conclusion": changes.pop("conclusion", "success"),
            "run_id": "10",
            "run_url": "https://example.invalid/10",
            "jobs": jobs,
            "plan": plan_value,
        },
        {
            "workflow": ".github/workflows/review-readiness.yml",
            "head_sha": sha,
            "status": "completed",
            "conclusion": "success" if publisher == "success" else publisher,
            "run_id": "11",
            "jobs": {"review-readiness-publisher": publisher},
        },
        {
            "workflow": ".github/workflows/frozen-subject-lock.yml",
            "head_sha": sha,
            "status": "completed",
            "conclusion": "success" if frozen == "success" else frozen,
            "run_id": "12",
            "jobs": {"frozen-subject-lock": frozen},
        },
        {
            "workflow": "check:review-readiness",
            "head_sha": sha,
            "status": "completed",
            "conclusion": review,
            "run_id": "13",
        },
    ]
    extra = changes.pop("extra_runs", None)
    if isinstance(extra, list):
        runs.extend(extra)
    observation = {
        "schema_id": "cargo-allow.ci-predecessor-observation.v1",
        "before": sha,
        "base": BASE,
        "head": sha,
        "source_paths": SOURCE_PATHS,
        "runs": runs,
    }
    observation.update(changes)
    return observation


def expect_reason(name: str, observation: dict, reason: str) -> None:
    result = reduce_observation(observation)
    if result.get("admitted") or result.get("reason") != reason:
        fail(f"{name}: {result}")


def check_predecessor_policy() -> None:
    admitted = reduce_observation(wide_observation(PROVEN))
    if not admitted.get("admitted") or admitted["evidence"]["proven_head"] != PROVEN:
        fail(f"wide neutral review-readiness was not proof: {admitted}")
    if admitted["evidence"]["source_paths"] != SOURCE_PATHS:
        fail("wide proof changed the source identity")

    expect_reason("failed review check", wide_observation(PROVEN, review="failure"), "failed")
    expect_reason("skipped heavy job", wide_observation(PROVEN, ci_jobs={"msrv": "skipped"}), "skipped")
    expect_reason("pending run", wide_observation(PROVEN, status="in_progress", conclusion=""), "pending")
    expect_reason("cancelled run", wide_observation(PROVEN, conclusion="cancelled"), "cancelled")
    expect_reason("failed run", wide_observation(PROVEN, conclusion="failure"), "failed")
    expect_reason("missing plan", wide_observation(PROVEN, omit_plan=True), "absent")
    expect_reason("empty source identity", wide_observation(PROVEN, source_paths=[]), "wrong_source")
    expect_reason("wrong source plan", wide_observation(PROVEN, plan_paths=["docs/ci.md"]), "wrong_source")
    expect_reason("base moved", wide_observation(PROVEN, plan_base="e" * 40), "base_moved")
    expect_reason("publisher failed", wide_observation(PROVEN, publisher="failure"), "failed")
    expect_reason("frozen lock failed", wide_observation(PROVEN, frozen="failure"), "failed")
    expect_reason("malformed observation", {"schema_id": "nope"}, "malformed")

    duplicate = wide_observation(PROVEN)
    duplicate["runs"].append(dict(duplicate["runs"][0], run_id="99"))
    expect_reason("ambiguous rerun", duplicate, "ambiguous")
    inflight = wide_observation(PROVEN)
    inflight["runs"].append(
        {
            "workflow": ".github/workflows/ci.yml",
            "head_sha": PROVEN,
            "status": "in_progress",
            "conclusion": "",
            "run_id": "14",
            "jobs": {},
            "plan": None,
        }
    )
    expect_reason("in-progress rerun", inflight, "pending")

    followed = wide_observation(PROVEN)
    followed["before"] = BEFORE
    followed["runs"].append(
        {
            "workflow": ".github/workflows/ci.yml",
            "head_sha": BEFORE,
            "status": "completed",
            "conclusion": "success",
            "run_id": "9",
            "jobs": {job: "skipped" for job in CI_JOBS},
            "plan": {
                "narrow": True,
                "head": BEFORE,
                "base": BASE,
                "effective_paths": SOURCE_PATHS + [".allow/review-dispositions/pr.json"],
                "proof_identity": {
                    "status": "success",
                    "base": BASE,
                    "source_paths": SOURCE_PATHS,
                    "proven_head": PROVEN,
                },
            },
        }
    )
    result = reduce_observation(followed)
    if not result.get("admitted") or result["evidence"]["proven_head"] != PROVEN:
        fail(f"narrow predecessor did not follow the wide proof: {result}")
    if result["evidence"]["predecessor"] != BEFORE:
        fail("followed proof lost the successor predecessor")

    missing_identity = wide_observation(BEFORE, narrow=True)
    expect_reason("narrow plan without proof identity", missing_identity, "absent")
    no_wide = {
        "schema_id": "cargo-allow.ci-predecessor-observation.v1",
        "before": BEFORE,
        "base": BASE,
        "head": BEFORE,
        "source_paths": SOURCE_PATHS,
        "runs": [followed["runs"][-1]],
    }
    expect_reason("proof identity without the wide run", no_wide, "absent")


MATRIX_DISPLAY = {
    "test-core-platforms": (
        "test-core-platforms (ubuntu-latest, x86_64-unknown-linux-gnu)",
        "test-core-platforms (windows-latest, x86_64-pc-windows-msvc)",
        "test-core-platforms (macos-26, aarch64-apple-darwin)",
        "test-core-platforms (macos-15-intel, x86_64-apple-darwin)",
    ),
    "compat-delegation": (
        "compat-delegation (ubuntu-latest)",
        "compat-delegation (windows-latest)",
    ),
    "test-proof-experimental": (
        "test-proof-experimental (ubuntu-latest)",
        "test-proof-experimental (windows-latest)",
    ),
}


def load_router():
    spec = importlib.util.spec_from_file_location("ci_proof_route_under_test", ROUTER)
    if spec is None or spec.loader is None:
        fail("router module did not load")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def api_job_rows(changes: dict[str, tuple[str, str]] | None = None, drop: set[str] | None = None) -> list[dict]:
    """Jobs API rows. ``changes`` maps a display name to (status, conclusion)."""
    rows: list[dict] = []
    number = 1
    skipped = drop or set()
    for job in CI_JOBS:
        if job in skipped:
            continue
        for name in MATRIX_DISPLAY.get(job, (job,)):
            status, conclusion = "completed", "success"
            if changes and name in changes:
                status, conclusion = changes[name]
            rows.append(
                {
                    "id": number,
                    "run_id": 10,
                    "name": name,
                    "status": status,
                    "conclusion": conclusion,
                    "steps": [{"name": "run", "status": status, "conclusion": conclusion}],
                }
            )
            number += 1
    return rows


def check_matrix_job_names() -> None:
    route = load_router()
    keys = frozenset(route.proof_job_rules())
    indexed = route.conclusions_from_job_rows(api_job_rows(), keys)
    for job in CI_JOBS:
        if indexed.get(job) != "success":
            fail(f"matrix display names did not prove {job}: {indexed.get(job)}")
    if any("(" in key for key in indexed):
        fail(f"display names leaked into job keys: {sorted(indexed)}")
    admitted = wide_observation(PROVEN)
    admitted["runs"][0]["jobs"] = indexed
    result = reduce_observation(admitted)
    if not result.get("admitted"):
        fail(f"API-shaped successful matrix jobs were not proof: {result}")

    failed_name = MATRIX_DISPLAY["test-core-platforms"][1]
    failed = route.conclusions_from_job_rows(
        api_job_rows({failed_name: ("completed", "failure")}),
        keys,
    )
    if failed.get("test-core-platforms") == "success":
        fail("an unsuccessful matrix row counted as success")
    rejected = wide_observation(PROVEN)
    rejected["runs"][0]["jobs"] = failed
    expect_reason("unsuccessful matrix row", rejected, "failed")

    pending_name = MATRIX_DISPLAY["compat-delegation"][0]
    pending = route.conclusions_from_job_rows(
        api_job_rows({pending_name: ("in_progress", "")}),
        keys,
    )
    if pending.get("compat-delegation") != "pending":
        fail(f"unfinished matrix row was {pending.get('compat-delegation')}")
    waiting = wide_observation(PROVEN)
    waiting["runs"][0]["jobs"] = pending
    expect_reason("unfinished matrix row", waiting, "pending")

    skipped_name = MATRIX_DISPLAY["test-proof-experimental"][0]
    skipped = route.conclusions_from_job_rows(
        api_job_rows({skipped_name: ("completed", "skipped")}),
        keys,
    )
    if skipped.get("test-proof-experimental") == "success":
        fail("a skipped matrix row counted as success")

    missing = route.conclusions_from_job_rows(api_job_rows(drop={"msrv"}), keys)
    if "msrv" in missing:
        fail("a missing job key was invented")
    absent = wide_observation(PROVEN)
    absent["runs"][0]["jobs"] = missing
    expect_reason("missing job row", absent, "absent")

    open_name = "pregate (ubuntu"
    open_rows = [row for row in api_job_rows() if row["name"] != "pregate"]
    open_rows.append({"id": 99, "name": open_name, "status": "completed", "conclusion": "success"})
    opened = route.conclusions_from_job_rows(open_rows, keys)
    if opened.get("pregate") == "success":
        fail("an unclosed display name satisfied pregate")

    longer = route.display_job_key(
        "release-rehearsal-required (ubuntu-latest)",
        frozenset({"release-rehearsal", "release-rehearsal-required"}),
    )
    if longer != "release-rehearsal-required":
        fail(f"shorter job key absorbed a longer key: {longer}")

    extra = api_job_rows() + [
        {"id": 100, "name": "Build", "status": "completed", "conclusion": "failure"}
    ]
    ignored = route.conclusions_from_job_rows(extra, keys)
    if "Build" in ignored or any(value != "success" for value in ignored.values()):
        fail(f"an unrelated job changed proof: {ignored}")

    duplicate = api_job_rows() + [
        {"id": 101, "name": "pregate", "status": "completed", "conclusion": "success"}
    ]
    try:
        route.conclusions_from_job_rows(duplicate, keys)
    except route._LookupStop as stop:
        if stop.reason != "ambiguous":
            fail(f"duplicate display name was {stop.reason}")
    else:
        fail("duplicate display name was accepted")

    try:
        route.conclusions_from_job_rows(
            [{"id": 1, "name": "", "status": "completed", "conclusion": "success"}],
            keys,
        )
    except route._LookupStop as stop:
        if stop.reason != "malformed":
            fail(f"empty job name was {stop.reason}")
    else:
        fail("empty job name was accepted")

    try:
        route.conclusions_from_job_rows(["not-a-job"], keys)
    except route._LookupStop as stop:
        if stop.reason != "malformed":
            fail(f"non-object job row was {stop.reason}")
    else:
        fail("non-object job row was accepted")


def check_machine_docs() -> None:
    """Machine-read docs stay source. Markdown prose can still narrow."""
    route = load_router()
    expected = {
        "docs/ci.md": "docs",
        "docs/README.md": "docs",
        "docs/ci/receipts/direct-floor-proof-cargo-allow-v1.selection.md": "docs",
        ".changes/note.yaml": "docs",
        "README.md": "docs",
        "docs/assets/feature-configuration-matrix-v1.json": "other",
        "docs/ci-lanes.toml": "other",
        "docs/support-matrix.toml": "other",
        "docs/ci/receipts/direct-floor-proof-cargo-allow-v1.json": "other",
        "docs/ci/receipts/direct-floor-proof-shared-v1.json": "other",
        "docs/schemas/operator-latency.v4.schema.json": "other",
        "docs/schemas/README.md": "other",
        "docs/ci/payload.bin": "other",
        "docs/notes": "other",
    }
    for path, kind in expected.items():
        got = route.path_kind(path)
        if got != kind:
            fail(f"path_kind {path}: {got}, expected {kind}")
    prose = "M\tdocs/ci/receipts/direct-floor-proof-cargo-allow-v1.selection.md\n"
    assert_plan(
        "receipt selection markdown stays docs-only",
        fixture("pull_request", None, prose),
        "docs_only",
        True,
    )
    named = (
        "docs/assets/feature-configuration-matrix-v1.json",
        "docs/ci-lanes.toml",
        "docs/support-matrix.toml",
        "docs/ci/receipts/direct-floor-proof-cargo-allow-v1.json",
        "docs/ci/payload.bin",
        "docs/notes",
    )
    for path in named:
        plan = fixture("pull_request", None, f"M\t{path}\n")
        assert_plan(f"{path} stays source", plan, "source_bearing", False)
    mixed = fixture(
        "pull_request",
        None,
        "M\tdocs/assets/feature-configuration-matrix-v1.json\nM\tdocs/ci.md\n",
    )
    assert_plan("machine docs beside prose stays wide", mixed, "source_bearing", False)
    successor = fixture(
        "pull_request",
        "M\tdocs/ci/receipts/direct-floor-proof-cargo-allow-v1.json\n",
        "M\tdocs/ci.md\n",
    )
    assert_plan(
        "receipt successor on a docs pull request stays wide",
        successor,
        "source_bearing",
        False,
    )


def check_machine_docs_git() -> None:
    samples = (
        "docs/assets/feature-configuration-matrix-v1.json",
        "docs/ci-lanes.toml",
        "docs/support-matrix.toml",
        "docs/ci/receipts/direct-floor-proof-cargo-allow-v1.json",
        "docs/ci/payload.bin",
    )
    for relative in samples:
        repo = Path(tempfile.mkdtemp())
        git(repo, "init", "-b", "main")
        git(repo, "config", "user.email", "ci-proof-route@example.invalid")
        git(repo, "config", "user.name", "ci-proof-route")
        git(repo, "config", "core.autocrlf", "false")
        write_repo(repo, "README.md", "base\n")
        git(repo, "add", "README.md")
        git(repo, "commit", "-m", "base")
        base = git(repo, "rev-parse", "HEAD")
        write_repo(repo, relative, "{}\n")
        git(repo, "add", "--", relative)
        git(repo, "commit", "-m", "machine")
        plan = run_router(
            [],
            env={
                "PROOF_EVENT": "pull_request",
                "PROOF_BEFORE": "0" * 40,
                "PROOF_BASE_SHA": base,
            },
            cwd=repo,
        )
        assert_plan(f"git {relative} stays wide", plan, "source_bearing", False)


REQUIRED_SKIPPED_FIXTURES = (
    "tests/fixtures/cargo-proof/intent-obligation-plan-smoke-v1.json",
    "tests/fixtures/cargo-proof/proof-plan-smoke-v1.toml",
    "tests/fixtures/proof-adapter-hawk/parity-boundary-v1.toml",
    "tests/fixtures/proof-adapter-ripr/parity-boundary-v1.toml",
    "tests/fixtures/three-product-dogfood/pipeline-stages-v1.toml",
)


def check_skipped_lane_fixtures() -> None:
    module = load_router()
    module._SURFACE_CACHE.pop("fixtures", None)
    status, derived = module.derive_skipped_lane_fixtures()
    if status != "ok":
        fail(f"ci.yml fixture map: {status}")
    missing = sorted(set(REQUIRED_SKIPPED_FIXTURES) - derived)
    if missing:
        fail(f"skipped-lane fixtures missing from the job map: {missing}")
    for path in sorted(derived):
        if module.is_test_surface(path) or module.path_kind(path) == "test":
            fail(f"{path} is consumed by a skipped lane and still test_only")
    ordinary = (
        "tests/fixtures/repo-edit/sample.toml",
        "fixtures/unsafe/src/lib.rs",
        "tests/fixtures/proof-adapter-hawk/parity-analysis-receipt-v1.toml",
        "tests/fixtures/proof-adapter-hawk/parity-finding-mapping-v1.toml",
        "tests/fixtures/proof-adapter-ripr/parity-grip-receipt-v1.toml",
    )
    for path in ordinary:
        if not module.is_test_surface(path):
            fail(f"{path} is not a skipped-lane fixture and no longer classifies test_only")
    module._SURFACE_CACHE.clear()
    original = module.CI_WORKFLOW
    module.CI_WORKFLOW = "ci-proof-route-missing.yml"
    try:
        broken, _found = module.derive_skipped_lane_fixtures()
        if broken == "ok":
            fail("a missing ci.yml still produced a fixture map")
        if module.is_test_surface("tests/fixtures/repo-edit/sample.toml"):
            fail("an unreadable job map still classified a fixture test_only")
        if module.path_kind("fixtures/unsafe/src/lib.rs") == "test":
            fail("an unreadable job map still classified a fixture as test")
    finally:
        module.CI_WORKFLOW = original
        module._SURFACE_CACHE.clear()


def check_script_decisions() -> None:
    module = load_router()
    status, derived = module.derive_live_scripts()
    if status != "ok":
        fail(f"ci.yml job map: {status}")
    live = module.live_script_surfaces()
    decided = module.TEST_ONLY_SCRIPT_DECISIONS
    disk = module.disk_test_scripts()
    error = module.partition_test_scripts(disk, live, decided)
    if error:
        fail(f"scripts/test-* decision: {error}")
    if not derived <= live:
        fail("derived live scripts were dropped")
    if not module.PINNED_SOURCE_SCRIPTS <= live:
        fail("pinned source scripts were dropped")
    exact = {path for path in disk if Path(path).name.startswith("test-exact-candidate-")}
    if not exact <= live:
        fail(f"test-exact-candidate-* left test_only: {sorted(exact - live)}")
    required = {
        "scripts/test-package-byte-authority.py",
        "scripts/test-perf-budget-smoke.sh",
        "scripts/test-release-binary.sh",
        "scripts/test-linux-gnu-candidate.py",
    }
    if not required <= live:
        fail(f"required live scripts missing: {sorted(required - live)}")
    injected = module.partition_test_scripts(
        disk | {"scripts/test-not-a-real-decision.py"},
        live,
        decided,
    )
    if injected is None or "scripts/test-not-a-real-decision.py" not in injected:
        fail("a new scripts/test-* without a decision was accepted")
    if module.script_surface_decision("scripts/test-not-a-real-decision.py") != "undecided":
        fail("an unknown scripts/test-* file received a test_only decision")
    if module.is_test_surface("scripts/test-not-a-real-decision.py"):
        fail("an unknown scripts/test-* file used the bare prefix")
    if not module.is_test_surface("scripts/test-ci-proof-route.py"):
        fail("a decided test script no longer classifies test_only")
    if module.is_test_surface(module.DELEGATION_TEST):
        fail("the delegation test is still a test surface")
    if module.is_test_surface("tests/compat/fixtures/sample.toml"):
        fail("compat fixtures are still a test surface")
    for path in sorted(decided):
        if module.script_surface_decision(path) != "test_only":
            fail(f"{path} lost its test_only decision")
    for path in sorted(live):
        if module.script_surface_decision(path) != "source":
            fail(f"{path} is live but not source")


def main() -> None:
    check_classifications()
    check_script_decisions()
    check_skipped_lane_fixtures()
    check_machine_docs()
    check_contract()
    check_workflow()
    check_git_history()
    check_machine_docs_git()
    check_predecessor_policy()
    check_matrix_job_names()
    print("ci-proof-route controls passed")


if __name__ == "__main__":
    main()
