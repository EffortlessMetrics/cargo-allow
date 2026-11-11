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
# Unread prose outside docs/architecture and outside both pins.
PROSE_DOC = "docs/ci/receipts/direct-floor-proof-cargo-allow-v1.selection.md"

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
DOCS_ONLY_HEAVY_GUARD = (
    "success() && (needs.proof-route.result != 'success' || "
    "needs.proof-route.outputs.narrow != 'true' || "
    "needs.proof-route.outputs.class != 'docs_only')"
)
TEST_GATE_MARKERS = (
    "Evidence-surface inventory guard",
    "cargo fmt --all --check",
    "cargo run -p cargo-allow -- audit",
    "check --mode no-new",
    "--require-change-note",
)
TEST_SETUP_USES = (
    "actions/checkout@",
    "dtolnay/rust-toolchain@",
    "./.github/actions/rust-cache",
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
    docs = f"M\t{PROSE_DOC}\n"
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
        ("root README stays wide", "pull_request", None, "M\tREADME.md\n", "source_bearing", False),
        (
            "root changelog stays docs-only",
            "pull_request",
            None,
            "M\tCHANGELOG.md\n",
            "docs_only",
            True,
        ),
        (
            "changes fragment stays wide",
            "pull_request",
            None,
            "A\t.changes/note.yaml\n",
            "source_bearing",
            False,
        ),
        (
            "extraction shim doc stays wide",
            "pull_request",
            None,
            "M\tdocs/architecture/extraction-shims.md\n",
            "source_bearing",
            False,
        ),
        (
            "repo snapshot stays wide",
            "pull_request",
            None,
            "M\tdocs/architecture/repo-snapshot.md\n",
            "source_bearing",
            False,
        ),
        ("unpinned prose stays docs-only", "pull_request", None, docs, "docs_only", True),
        (
            "product move map stays wide",
            "pull_request",
            None,
            "M\tdocs/architecture/product-move-map.md\n",
            "source_bearing",
            False,
        ),
        (
            "new architecture reader stays wide",
            "pull_request",
            None,
            "M\tdocs/architecture/new-reader.md\n",
            "source_bearing",
            False,
        ),
        (
            "changie sensor doc stays wide",
            "pull_request",
            None,
            "M\tdocs/how-to/run-changie-sensor.md\n",
            "source_bearing",
            False,
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


def step_if_expression(part: str) -> str:
    """Return the step's if expression. Comments are not conditions."""
    for line in part.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        match = re.match(r"if:\s*(.*)$", stripped)
        if match:
            return match.group(1)
    return ""


def step_is_docs_guarded(part: str) -> bool:
    return DOCS_ONLY_HEAVY_GUARD in step_if_expression(part)


def check_comment_guard_fixture() -> None:
    """The old whole-body search accepts a comment-only guard. The if parser does not."""
    comment_only = "\n".join(
        (
            "name: Comment guard",
            "        # if: ${{ " + DOCS_ONLY_HEAVY_GUARD + " }}",
            "        run: 'true'",
        )
    )
    if DOCS_ONLY_HEAVY_GUARD not in comment_only:
        fail("comment-guard fixture lost the guard text the old whole-body search matched")
    if step_is_docs_guarded(comment_only):
        fail("a guard that appears only in a comment counted as the step if")
    real = "\n".join(
        (
            "name: Real guard",
            "        if: ${{ " + DOCS_ONLY_HEAVY_GUARD + " }}",
            "        # " + DOCS_ONLY_HEAVY_GUARD,
            "        run: 'true'",
        )
    )
    if not step_is_docs_guarded(real):
        fail("the if expression lost the docs_only guard")


def check_test_docs_only_steps(body: str) -> None:
    """Heavy test steps carry the docs_only guard. Gate steps do not."""
    steps_at = body.find("\n    steps:\n")
    if steps_at < 0:
        fail("test job has no steps")
    header = body[:steps_at]
    if "needs: [pregate, package-smoke, proof-route]" not in header:
        fail("test dropped pregate, package-smoke, or proof-route from needs")
    if TEST_IF not in header:
        fail("test job if no longer keeps the package-smoke success-or-skipped gate")
    if DOCS_ONLY_HEAVY_GUARD in header:
        fail("the docs_only guard skipped the test job instead of a heavy step")
    parts = re.split(r"(?m)^      - ", body[steps_at:])[1:]
    if not parts:
        fail("test job steps did not split")
    gates: list[str] = []
    heavy = 0
    uploads = 0
    setups = 0
    for part in parts:
        first = part.splitlines()[0]
        guarded = step_is_docs_guarded(part)
        gate_hits = [marker for marker in TEST_GATE_MARKERS if marker in part]
        if "upload-artifact@" in part:
            uploads += 1
            if "if: always()" not in part or guarded or gate_hits:
                fail("an artifact upload must stay on if: always() without the docs_only guard")
            continue
        if gate_hits:
            if guarded or "if: always()" in part:
                fail(f"a gate step gained a skip: {gate_hits}")
            gates.extend(gate_hits)
            continue
        if any(marker in first for marker in TEST_SETUP_USES):
            setups += 1
            if guarded:
                fail(f"setup step gained the docs_only guard: {first}")
            continue
        if not guarded:
            fail(f"heavy test step has no docs_only guard:\n{part[:240]}")
        heavy += 1
    if gates != list(TEST_GATE_MARKERS):
        fail(f"gate steps drifted: {gates}")
    if setups != len(TEST_SETUP_USES):
        fail(f"expected {len(TEST_SETUP_USES)} setup steps, found {setups}")
    if uploads != 4:
        fail(f"expected 4 unconditional artifact uploads, found {uploads}")
    if heavy != 18:
        fail(f"expected 18 guarded heavy steps, found {heavy}")


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
        if job in {"proof-route", "package-smoke", "test"}:
            continue
        if "proof-route" in jobs[job] or "outputs.narrow" in jobs[job]:
            fail(f"{job} reads the narrow plan; required and release lanes must not")
    check_test_docs_only_steps(jobs["test"])
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
    write_repo(docs_repo, PROSE_DOC, "docs\n")
    git(docs_repo, "add", "--", PROSE_DOC)
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


def _strip_rust_comments(text: str) -> str:
    out: list[str] = []
    index = 0
    length = len(text)
    while index < length:
        if text.startswith("//", index):
            newline = text.find("\n", index)
            if newline < 0:
                break
            out.append("\n")
            index = newline + 1
            continue
        if text.startswith("/*", index):
            end = text.find("*/", index + 2)
            if end < 0:
                break
            out.append("\n" * text.count("\n", index, end))
            index = end + 2
            continue
        if text[index] == '"':
            out.append(text[index])
            index += 1
            while index < length:
                out.append(text[index])
                if text[index] == "\\" and index + 1 < length:
                    out.append(text[index + 1])
                    index += 2
                    continue
                if text[index] == '"':
                    index += 1
                    break
                index += 1
            continue
        out.append(text[index])
        index += 1
    return "".join(out)


def _guarded_step_blob() -> str:
    """Non-comment text of test steps whose if expression guards on docs_only."""
    test = job_bodies(CI.read_text(encoding="utf-8"))["test"]
    steps_at = test.find("\n    steps:\n")
    parts = re.split(r"(?m)^      - ", test[steps_at:])[1:]
    guarded_lines: list[str] = []
    for part in parts:
        kept: list[str] = []
        expr = ""
        for line in part.splitlines():
            stripped = line.strip()
            if stripped.startswith("#"):
                continue
            kept.append(line)
            match = re.match(r"if:\s*(.*)$", stripped)
            if match and not expr:
                expr = match.group(1)
        if "outputs.class != 'docs_only'" in expr and "outputs.narrow" in expr:
            guarded_lines.append("\n".join(kept))
    return "\n".join(guarded_lines)


def _package_dirs(blob: str) -> list[Path]:
    packages = sorted(set(re.findall(r"(?:^|\s)-p ([A-Za-z0-9_-]+)", blob)))
    name_to_dir: dict[str, Path] = {}
    for cargo in (ROOT / "crates").glob("*/Cargo.toml"):
        cargo_text = cargo.read_text(encoding="utf-8")
        name = re.search(r'(?m)^name\s*=\s*"([^"]+)"', cargo_text)
        if name:
            name_to_dir[name.group(1)] = cargo.parent
    return [name_to_dir[package] for package in packages if package in name_to_dir]


def guarded_step_markdown() -> set[str]:
    """docs/*.md that a guarded test step reads.

    include_str!, read/read_to_string of a literal docs path, product-doc
    kinds walked by the support-matrix test, repo files copied into a
    rehearsal fixture, the doc-artifact ledger when a guarded test validates
    it, and read_text or shell reads in a guarded step's script.
    Architecture markdown stays out of this pin and is source by directory.
    """
    include_str_re = re.compile(r'include_str!\s*\(\s*"([^"]+)"', re.S)
    read_re = re.compile(
        r'\bread\s*\(\s*&?[A-Za-z_][\w.]*\s*,\s*"(docs/[^"]+\.md)"\s*,?\s*\)',
        re.S,
    )
    join_re = re.compile(
        r'(?:read_to_string|fs::read|std::fs::read)\s*\(\s*[\w.:]+\.join\(\s*"(docs/[^"]+\.md)"\s*\)',
        re.S,
    )
    read_text_re = re.compile(
        r"""\(\s*(?:root|ROOT)\s*/\s*["'](docs/[^"']+\.md)["']\s*\)\s*\.read_text""",
        re.S,
    )
    shell_re = re.compile(r"(?:^|\n)\s*(?:cat|head|tail)\s+(docs/\S+\.md)|<\s*(docs/\S+\.md)")
    ledger_re = re.compile(r'(?m)^path\s*=\s*"(docs/[^"]+\.md)"')
    blob = _guarded_step_blob()
    scripts = sorted(set(re.findall(r"scripts/[\w./-]+\.(?:sh|py)", blob)))
    found: set[str] = set()

    def add(path: str) -> None:
        norm = path.replace("\\", "/")
        if norm.startswith("docs/") and norm.endswith(".md") and ".." not in norm.split("/"):
            found.add(norm)

    ledger = False
    for directory in _package_dirs(blob):
        for src in directory.rglob("*.rs"):
            text = _strip_rust_comments(src.read_text(encoding="utf-8", errors="replace"))
            for literal in include_str_re.findall(text):
                resolved = (src.parent / literal).resolve()
                try:
                    add(resolved.relative_to(ROOT.resolve()).as_posix())
                except ValueError:
                    continue
            for literal in read_re.findall(text):
                add(literal)
            for literal in join_re.findall(text):
                add(literal)
            if "docs/products/{product}/{kind}.md" in text:
                kind_block = re.search(r"const DOC_KINDS:[^=]*=\s*&\[(.*?)\]", text, re.S)
                kinds = re.findall(r'"([^"]+)"', kind_block.group(1)) if kind_block else []
                named = re.findall(r'\(\s*"([a-z0-9-]+)"\s*,\s*vec!\[', text)
                for product in named:
                    for kind in kinds:
                        add(f"docs/products/{product}/{kind}.md")
            if "std::fs::copy" in text and "repo.join" in text:
                for block in re.findall(r"\[(.*?)\]", text, re.S):
                    if "docs/" not in block:
                        continue
                    for literal in re.findall(r'"(docs/[^"]+\.md)"', block):
                        add(literal)
            if (
                "validate_doc_artifact_files" in text
                and ".allow/artifacts/doc-artifacts.toml" in text
            ):
                ledger = True
    if ledger:
        ledger_text = (ROOT / ".allow/artifacts/doc-artifacts.toml").read_text(encoding="utf-8")
        for literal in ledger_re.findall(ledger_text):
            add(literal)
    for script in scripts:
        path = ROOT / script
        if not path.is_file():
            continue
        body = "\n".join(
            line
            for line in path.read_text(encoding="utf-8", errors="replace").splitlines()
            if not line.lstrip().startswith("#")
        )
        for literal in read_text_re.findall(body):
            add(literal)
        for left, right in shell_re.findall(body):
            add(left or right)
    return found


def _config_doc_literals(text: str) -> set[str]:
    """Quoted ``docs/*.md`` paths in one policy file."""
    return set(re.findall(r'"(docs/[^"]+\.md)"', text))


def _projection_docs(text: str) -> set[str]:
    """``projection = "docs/*.md"`` values in one policy file."""
    return set(re.findall(r'(?m)^\s*projection\s*=\s*"(docs/[^"]+\.md)"', text))


def _scan_indirect_readers() -> tuple[set[str], set[str], set[str], set[str], frozenset[str]]:
    """Return readers, assigned-join subset, old special case, config docs, scanned policy.

    The assigned-join subset omits a projection-field read. The special case
    is repository-root ``include_str!`` markdown plus extraction-shims.md only.
    Config docs are ``docs/*.md`` paths named by a ``policy/*.toml`` file a
    guarded test loads, when a guarded test also reads that path. The
    product-move ledger projection is one of those paths.
    ``policy/extraction-parity.toml`` is scanned when a guarded test loads it.
    """
    cached = getattr(_scan_indirect_readers, "cached", None)
    if cached is not None:
        return cached
    include_str_re = re.compile(r'include_str!\s*\(\s*"([^"]+)"', re.S)
    assigned_join_re = re.compile(
        r'(?:let|let mut)\s+(\w+)\s*=\s*[\w.:]+\.join\(\s*"(docs/[^"]+\.md)"\s*\)\s*;',
        re.S,
    )
    assigned_projection_re = re.compile(
        r'(?:let|let mut)\s+(\w+)\s*=\s*[\w.:]+\.join\(\s*&[\w.]+\.projection\b',
        re.S,
    )
    inline_projection_re = re.compile(
        r"(?:read_to_string|fs::read)\s*\([^;\n]*\.join\(\s*&[\w.]+\.projection\b",
        re.S,
    )
    projection_literal_re = re.compile(r'projection\s*(?:!=|=)\s*"(docs/[^"]+\.md)"')
    policy_re = re.compile(r'"(policy/[^"]+\.toml)"')
    read_binding_re = r"(?:read_to_string|fs::read)\s*\(\s*&?{name}\b"
    shim = "docs/architecture/extraction-shims.md"
    roots: set[str] = set()
    assigned: set[str] = set()
    projection: set[str] = set()
    mentioned_policy: set[str] = set()
    reads_projection = False
    legacy_shim = False
    for directory in _package_dirs(_guarded_step_blob()):
        for src in directory.rglob("*.rs"):
            text = _strip_rust_comments(src.read_text(encoding="utf-8", errors="replace"))
            mentioned_policy.update(policy_re.findall(text))
            for literal in include_str_re.findall(text):
                resolved = (src.parent / literal).resolve()
                try:
                    rel = resolved.relative_to(ROOT.resolve()).as_posix()
                except ValueError:
                    continue
                if "/" not in rel and rel.endswith(".md") and (ROOT / rel).is_file():
                    roots.add(rel)
            for name, path in assigned_join_re.findall(text):
                if re.search(read_binding_re.format(name=re.escape(name)), text):
                    assigned.add(path)
            if f'.join("{shim}")' in text and "read_to_string" in text:
                legacy_shim = True
            file_reads_projection = bool(inline_projection_re.search(text))
            for name in assigned_projection_re.findall(text):
                if re.search(read_binding_re.format(name=re.escape(name)), text):
                    file_reads_projection = True
            if not file_reads_projection:
                continue
            reads_projection = True
            named = set(projection_literal_re.findall(text))
            if not named and "policy/product-move-ledger.toml" in text:
                ledger = (ROOT / "policy/product-move-ledger.toml").read_text(encoding="utf-8")
                named.update(_projection_docs(ledger))
            for path in named:
                if (ROOT / path).is_file():
                    projection.add(path)
    config: set[str] = set()
    scanned: set[str] = set()
    read_docs = assigned | projection
    for rel in sorted(mentioned_policy):
        if not rel.startswith("policy/") or rel.count("/") != 1:
            continue
        file = ROOT / rel
        if not file.is_file():
            continue
        scanned.add(rel)
        toml_text = file.read_text(encoding="utf-8")
        if reads_projection:
            for path in _projection_docs(toml_text):
                if (ROOT / path).is_file():
                    config.add(path)
                    projection.add(path)
        for path in _config_doc_literals(toml_text):
            if path in read_docs and (ROOT / path).is_file():
                config.add(path)
    legacy = set(roots)
    if legacy_shim:
        legacy.add(shim)
    result = (
        roots | assigned | projection,
        roots | assigned,
        legacy,
        config,
        frozenset(scanned),
    )
    _scan_indirect_readers.cached = result
    return result


def derive_indirect_readers() -> set[str]:
    """Readers the 73-path markdown pin does not keep.

    Repository-root markdown embedded with include_str!, an assigned
    ``join("docs/...md")`` later passed to ``read_to_string`` or ``fs::read``,
    and a ``docs/*.md`` path named by a ``policy/*.toml`` file a guarded
    test loads, including the product-move ledger projection and
    ``policy/extraction-parity.toml``. Architecture paths in this set are
    source by directory, not by ``INDIRECT_CONSUMED_PATHS``.
    """
    return _scan_indirect_readers()[0]


def indirect_reader_problems(paths: set[str], derived: set[str]) -> list[str]:
    problems: list[str] = []
    for path in sorted(paths):
        if not (ROOT / path).is_file():
            problems.append(f"missing:{path}")
    if set(paths) != set(derived):
        missing = sorted(set(derived) - set(paths))
        extra = sorted(set(paths) - set(derived))
        problems.append(f"drift missing={missing} extra={extra}")
    return problems


def _skip_rust_string(text: str, index: int) -> int:
    if index >= len(text) or text[index] != '"':
        return index
    index += 1
    while index < len(text):
        if text[index] == "\\" and index + 1 < len(text):
            index += 2
            continue
        if text[index] == '"':
            return index + 1
        index += 1
    return index


def _match_delim(text: str, open_at: int, open_ch: str, close_ch: str) -> tuple[str, int] | None:
    depth = 0
    index = open_at
    while index < len(text):
        char = text[index]
        if char == '"':
            index = _skip_rust_string(text, index)
            continue
        if char == open_ch:
            depth += 1
        elif char == close_ch:
            depth -= 1
            if depth == 0:
                return text[open_at + 1 : index], index + 1
        index += 1
    return None


def _split_rust_list(text: str) -> list[str]:
    parts: list[str] = []
    buf: list[str] = []
    depth = 0
    index = 0
    while index < len(text):
        char = text[index]
        if char == '"':
            nxt = _skip_rust_string(text, index)
            buf.append(text[index:nxt])
            index = nxt
            continue
        if char in "([{<":
            depth += 1
        elif char in ")]}>":
            depth = max(0, depth - 1)
        if char == "," and depth == 0:
            parts.append("".join(buf).strip())
            buf = []
            index += 1
            continue
        buf.append(char)
        index += 1
    tail = "".join(buf).strip()
    if tail:
        parts.append(tail)
    return parts


def _rust_functions(text: str) -> list[tuple[str, str, str, int, int]]:
    found: list[tuple[str, str, str, int, int]] = []
    pattern = re.compile(r"(?m)^(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([A-Za-z_][\w]*)")
    index = 0
    while True:
        match = pattern.search(text, index)
        if not match:
            break
        cursor = match.end()
        while cursor < len(text) and text[cursor].isspace():
            cursor += 1
        if cursor < len(text) and text[cursor] == "<":
            generic = _match_delim(text, cursor, "<", ">")
            if generic is None:
                break
            cursor = generic[1]
            while cursor < len(text) and text[cursor].isspace():
                cursor += 1
        if cursor >= len(text) or text[cursor] != "(":
            index = match.end()
            continue
        params = _match_delim(text, cursor, "(", ")")
        if params is None:
            break
        brace = text.find("{", params[1])
        if brace < 0:
            break
        body = _match_delim(text, brace, "{", "}")
        if body is None:
            break
        found.append((match.group(1), params[0], body[0], match.start(), body[1]))
        index = body[1]
    return found


def _path_param_indexes(params: str) -> list[tuple[int, str]]:
    indexes: list[tuple[int, str]] = []
    for index, part in enumerate(_split_rust_list(params)):
        if ":" not in part or part.startswith("self"):
            continue
        name, ty = part.split(":", 1)
        name = name.strip().removeprefix("mut ").strip()
        if re.search(r"(?:^|[^A-Za-z])str(?:[^A-Za-z]|$)", ty) or "String" in ty:
            indexes.append((index, name))
    return indexes


def _helper_reads_param(body: str, param: str) -> bool:
    direct = re.compile(
        r"(?:std::fs::|fs::)read(?:_to_string)?\s*\(\s*&?\s*[\w.:]+\.join\(\s*&?\s*"
        + re.escape(param)
        + r"\b"
    )
    if direct.search(body):
        return True
    bind = re.compile(
        r"(?:let|let mut)\s+(\w+)\s*=\s*[\w.:]+\.join\(\s*&?\s*" + re.escape(param) + r"\b"
    )
    locals_for = set(bind.findall(body))
    if not locals_for:
        return False
    read_local = re.compile(r"(?:std::fs::|fs::)read(?:_to_string)?\s*\(\s*&?\s*(\w+)\b")
    return any(local in locals_for for local in read_local.findall(body))


def _is_docs_markdown(value: str) -> bool:
    parts = value.split("/")
    return value.startswith("docs/") and value.endswith(".md") and ".." not in parts and len(parts) >= 2


def _const_docs(text: str) -> dict[str, str]:
    found: dict[str, str] = {}
    for name, value in re.findall(r'(?m)^const\s+([A-Za-z_][\w]*)\s*:\s*&str\s*=\s*"([^"]*)"\s*;', text):
        if _is_docs_markdown(value):
            found[name] = value
    return found


def _slice_docs(functions: list[tuple[str, str, str, int, int]]) -> dict[str, set[str]]:
    found: dict[str, set[str]] = {}
    for name, _params, body, _start, _end in functions:
        compact = " ".join(body.split())
        match = re.fullmatch(r"&?\[(.*)\]", compact)
        if not match:
            continue
        values = {item for item in re.findall(r'"([^"]*)"', match.group(1)) if _is_docs_markdown(item)}
        if values:
            found[name] = values
    return found


def _docs_from_expr(expr: str, const_docs: dict[str, str], slice_docs: dict[str, set[str]]) -> set[str]:
    expr = expr.strip().removeprefix("&").strip()
    if len(expr) >= 2 and expr[0] == '"' and expr[-1] == '"':
        value = expr[1:-1]
        return {value} if _is_docs_markdown(value) else set()
    if re.fullmatch(r"[A-Za-z_][\w]*", expr):
        if expr in const_docs:
            return {const_docs[expr]}
        if expr in slice_docs:
            return set(slice_docs[expr])
    return set()


def helper_mediated_markdown(text: str) -> set[str]:
    """docs/*.md reached through a same-file read helper.

    The helper's body reads ``root.join`` of a ``&str`` parameter with
    ``fs::read_to_string`` or ``fs::read``. The call passes that path as a
    literal, a same-file ``&str`` constant, a same-file slice of those
    literals, or a ``for`` loop over that slice or a const array.
    """
    functions = _rust_functions(text)
    const_docs = _const_docs(text)
    slice_docs = _slice_docs(functions)
    helpers: list[tuple[str, list[int]]] = []
    for name, params, body, _start, _end in functions:
        indexes = [index for index, param in _path_param_indexes(params) if _helper_reads_param(body, param)]
        if indexes:
            helpers.append((name, indexes))
    found: set[str] = set()
    call_re_cache: dict[str, re.Pattern[str]] = {}
    for name, indexes in helpers:
        pattern = call_re_cache.setdefault(name, re.compile(rf"(?<![\w:]){re.escape(name)}\s*\("))
        for match in pattern.finditer(text):
            if text[max(0, match.start() - 4) : match.start()].rstrip().endswith("fn"):
                continue
            parsed = _match_delim(text, match.end() - 1, "(", ")")
            if parsed is None:
                continue
            args = _split_rust_list(parsed[0])
            owner = next(
                (
                    body
                    for _fn_name, _params, body, fn_start, fn_end in functions
                    if fn_start <= match.start() < fn_end
                ),
                "",
            )
            for index in indexes:
                if index >= len(args):
                    continue
                expr = args[index].strip()
                docs = _docs_from_expr(expr, const_docs, slice_docs)
                bare = expr.removeprefix("&").strip()
                if not docs and re.fullmatch(r"[A-Za-z_][\w]*", bare):
                    loop_call = re.search(rf"for\s+{re.escape(bare)}\s+in\s+([A-Za-z_][\w]*)\s*\(", owner)
                    if loop_call and loop_call.group(1) in slice_docs:
                        docs = set(slice_docs[loop_call.group(1)])
                    loop_list = re.search(rf"for\s+{re.escape(bare)}\s+in\s+\[([^\]]*)\]", owner, re.S)
                    if loop_list:
                        for item in _split_rust_list(loop_list.group(1)):
                            docs |= _docs_from_expr(item, const_docs, slice_docs)
                found |= docs
    return found


def derive_helper_markdown() -> set[str]:
    """Existing helper-mediated docs a guarded test package reads."""
    found: set[str] = set()
    for directory in _package_dirs(_guarded_step_blob()):
        for src in directory.rglob("*.rs"):
            text = _strip_rust_comments(src.read_text(encoding="utf-8", errors="replace"))
            for path in helper_mediated_markdown(text):
                if (ROOT / path).is_file():
                    found.add(path)
    return found


def helper_reader_problems(paths: set[str], derived: set[str]) -> list[str]:
    problems: list[str] = []
    for path in sorted(paths):
        if not (ROOT / path).is_file():
            problems.append(f"missing:{path}")
    if set(paths) != set(derived):
        missing = sorted(set(derived) - set(paths))
        extra = sorted(set(paths) - set(derived))
        problems.append(f"drift missing={missing} extra={extra}")
    return problems


def check_helper_readers() -> None:
    """Helper-mediated docs stay wide. A new literal or a missing path fails."""
    route = load_router()
    derived = derive_helper_markdown()
    pinned = set(route.HELPER_CONSUMED_MARKDOWN)
    machine = set(route.MACHINE_CONSUMED_MARKDOWN)
    indirect = set(route.INDIRECT_CONSUMED_PATHS)
    fresh = derived - machine - indirect
    problems = helper_reader_problems(pinned, fresh)
    if problems:
        fail("helper readers " + "; ".join(problems))
    if len(machine) != 73:
        fail("helper scan changed the 73-path markdown pin")
    if pinned & machine or pinned & indirect:
        fail("helper pin overlaps an existing pin")
    if set(route.INDIRECT_CONSUMED_PATHS) != {"README.md", "SUPPORT.md"}:
        fail("helper scan changed the root companion")
    sensor = "docs/how-to/run-changie-sensor.md"
    required = (
        sensor,
        "docs/release/README.md",
        "docs/release/0.1.10.md",
        "docs/release/0.1.11.md",
    )
    if set(required) != pinned:
        fail(f"helper pin is {sorted(pinned)}")
    for path in required:
        if route.path_kind(path) != "other":
            fail(f"{path} helper reader is not source")
        assert_plan(
            f"{path} forces a wide plan",
            fixture("pull_request", None, f"M\t{path}\n"),
            "source_bearing",
            False,
        )
    already = (
        "docs/release/0.2.0.md",
        "docs/release/github/v0.2.0.md",
        "docs/release/0.2.0-rc.1.md",
        "docs/release/incidents/2026-08-24-rc1-tag-movement.md",
    )
    for path in already:
        if path not in derived or path not in machine or path in pinned:
            fail(f"{path} should stay on the 73-path pin")
    absent = (
        "docs/identity.md",
        "docs/design.md",
        "docs/adr/README.md",
        "docs/source-of-truth/README.md",
        "docs/templates/spec.md",
        "docs/templates/proposal.md",
    )
    for path in absent:
        if path in derived or path in pinned:
            fail(f"{path} was pinned without a helper read")
    literal = """
fn read_workspace_file(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap()
}

fn reads_new_doc() {
    let _doc = read_workspace_file(&root, "docs/how-to/helper-named-extra.md");
}
"""
    blind = """
fn read_workspace_file(root: &Path, rel: &str) -> String {
    rel.to_string()
}

fn reads_new_doc() {
    let _doc = read_workspace_file(&root, "docs/how-to/helper-named-extra.md");
}
"""
    extra = "docs/how-to/helper-named-extra.md"
    if helper_mediated_markdown(blind):
        fail("a helper that does not read was treated as a reader")
    if helper_mediated_markdown(literal) != {extra}:
        fail(f"helper literal fixture parsed {sorted(helper_mediated_markdown(literal))}")
    const_form = """
const RELEASE_DOC: &str = "docs/release/helper-const.md";

fn read_workspace_file(root: &Path, relative_path: &str) -> String {
    fs::read_to_string(root.join(relative_path)).unwrap()
}

fn reads_release() {
    let _doc = read_workspace_file(&root, RELEASE_DOC);
}
"""
    if helper_mediated_markdown(const_form) != {"docs/release/helper-const.md"}:
        fail("helper const fixture was not parsed")
    before = helper_reader_problems(pinned - {sensor}, fresh)
    if not any(item.startswith("drift ") for item in before):
        fail("dropping the changie sensor doc did not fail the drift check")
    after = helper_reader_problems(pinned, fresh)
    if after:
        fail("helper pin did not match after the sensor doc was restored")
    missing = helper_reader_problems(pinned | {"docs/how-to/missing-helper.md"}, fresh)
    if not any(item.startswith("missing:") for item in missing):
        fail("missing helper path did not fail the check")


def check_indirect_readers() -> None:
    """Root embeds stay pinned. Every architecture markdown path stays wide."""
    route = load_router()
    pinned = set(route.INDIRECT_CONSUMED_PATHS)
    generalized, assigned_only, legacy, config, scanned = _scan_indirect_readers()
    derived = derive_indirect_readers()
    if derived != generalized:
        fail("derive_indirect_readers diverged from the indirect scan")
    architecture = {path for path in derived if route.is_architecture_markdown(path)}
    outside = derived - architecture
    if outside != pinned:
        fail(
            "root indirect readers drifted "
            f"pinned={sorted(pinned)} derived={sorted(outside)}"
        )
    problems = indirect_reader_problems(pinned, outside)
    if problems:
        fail("indirect readers " + "; ".join(problems))
    if pinned & set(route.MACHINE_CONSUMED_MARKDOWN):
        fail("indirect readers were folded into the markdown pin")
    move_map = "docs/architecture/product-move-map.md"
    if move_map in assigned_only:
        fail("assigned-join scan claimed the projection reader")
    if move_map not in derived or move_map not in architecture:
        fail("projection reader was missed")
    if move_map in pinned:
        fail("projection reader was folded into the root companion")
    for path in ("README.md", "SUPPORT.md"):
        if path not in derived or path not in pinned:
            fail(f"{path} indirect reader was not derived")
        if route.path_kind(path) != "other":
            fail(f"{path} indirect reader is not source")
        assert_plan(
            f"{path} forces a wide plan",
            fixture("pull_request", None, f"M\t{path}\n"),
            "source_bearing",
            False,
        )
    known = (
        "docs/architecture/cargo-proof.md",
        "docs/architecture/extraction-parity.md",
        "docs/architecture/extraction-shims.md",
        "docs/architecture/intent-edit.md",
        "docs/architecture/intent-engine.md",
        "docs/architecture/intent-model.md",
        "docs/architecture/intent-protocol.md",
        "docs/architecture/product-crate-law.md",
        move_map,
        "docs/architecture/proof-adapter-cargo-allow.md",
        "docs/architecture/proof-adapter-command.md",
        "docs/architecture/proof-adapter-hawk.md",
        "docs/architecture/proof-adapter-ripr.md",
        "docs/architecture/proof-engine.md",
        "docs/architecture/proof-protocol.md",
        "docs/architecture/repo-edit.md",
        "docs/architecture/repo-snapshot.md",
        "docs/architecture/rust-source-index.md",
    )
    for path in known:
        if path not in derived:
            fail(f"{path} indirect reader was not derived")
    on_disk = sorted(
        path.relative_to(ROOT).as_posix()
        for path in (ROOT / "docs" / "architecture").glob("*.md")
        if path.is_file()
    )
    if move_map not in on_disk:
        fail(f"{move_map} is not on disk")
    nested = sorted(
        path.relative_to(ROOT).as_posix()
        for path in (ROOT / "docs" / "architecture").rglob("*.md")
        if path.is_file() and path.parent.name != "architecture"
    )
    if nested:
        fail(f"nested architecture markdown is outside the directory rule: {nested}")
    for path in on_disk:
        if not route.is_architecture_markdown(path) or route.path_kind(path) != "other":
            fail(f"{path} architecture markdown is not source")
        if path in pinned or path in route.MACHINE_CONSUMED_MARKDOWN:
            fail(f"{path} architecture markdown was hand-pinned")
        assert_plan(
            f"{path} forces a wide plan",
            fixture("pull_request", None, f"M\t{path}\n"),
            "source_bearing",
            False,
        )
    new_reader = "docs/architecture/new-reader.md"
    if (ROOT / new_reader).exists():
        fail("synthetic architecture reader must not already exist")
    if new_reader in pinned or new_reader in route.MACHINE_CONSUMED_MARKDOWN:
        fail("synthetic architecture reader was pinned")
    if route.path_kind(new_reader) != "other":
        fail("a newly added architecture reader is not source")
    assert_plan(
        "new architecture reader forces a wide plan",
        fixture("pull_request", None, f"M\t{new_reader}\n"),
        "source_bearing",
        False,
    )
    if PROSE_DOC in pinned or PROSE_DOC in derived or route.path_kind(PROSE_DOC) != "docs":
        fail(f"{PROSE_DOC} no longer samples unread prose")
    if route.is_architecture_markdown(PROSE_DOC):
        fail(f"{PROSE_DOC} is architecture markdown")
    assert_plan(
        "unpinned prose stays docs-only",
        fixture("pull_request", None, f"M\t{PROSE_DOC}\n"),
        "docs_only",
        True,
    )
    fragment = ".changes/note.yaml"
    if route.path_kind(fragment) != "other":
        fail(f"{fragment} is not source")
    if route.path_kind(".changes/v0.2.0.md") != "other":
        fail(".changes markdown is not source")
    assigned_problems = indirect_reader_problems(assigned_only, derived)
    if not any(item.startswith("drift ") for item in assigned_problems):
        fail("assigned-join-only set did not drift")
    legacy_problems = indirect_reader_problems(legacy, derived)
    if not any(item.startswith("drift ") for item in legacy_problems):
        fail("extraction-shims-only set did not drift")
    missing = indirect_reader_problems(
        pinned | {"docs/architecture/missing-indirect.md"},
        outside,
    )
    if not any(item.startswith("missing:") for item in missing):
        fail("missing indirect path did not fail the check")
    drifted = indirect_reader_problems(pinned - {"README.md"}, outside)
    if not any(item.startswith("drift ") for item in drifted):
        fail("indirect reader drift did not fail the check")
    for required_policy in (
        "policy/product-move-ledger.toml",
        "policy/extraction-parity.toml",
    ):
        if required_policy not in scanned:
            fail(f"{required_policy} was not scanned for config-named docs")
    if move_map not in config:
        fail(f"config-named docs missed the projection: {sorted(config)}")
    for path in config:
        if route.is_architecture_markdown(path):
            if path in pinned or route.path_kind(path) != "other":
                fail(f"{path} config-named architecture doc is not directory source")
            continue
        if path not in pinned and path not in route.MACHINE_CONSUMED_MARKDOWN:
            fail(f"config-named docs path is outside the directory rule and unpinned: {path}")
    parity_text = (ROOT / "policy/extraction-parity.toml").read_text(encoding="utf-8")
    config_extra = "docs/architecture/config-named-extra.md"
    injected = _config_doc_literals(
        parity_text + f'\nnote = "{config_extra}"\n'
    )
    if injected != {config_extra}:
        fail("a new extraction-parity docs path was not parsed")
    if config_extra in pinned or config_extra in route.MACHINE_CONSUMED_MARKDOWN:
        fail("injected architecture reader was pinned")
    if route.path_kind(config_extra) != "other":
        fail("injected architecture reader is not source")
    assert_plan(
        "injected architecture reader forces a wide plan",
        fixture("pull_request", None, f"M\t{config_extra}\n"),
        "source_bearing",
        False,
    )
    config_missing = indirect_reader_problems(
        config | {"docs/architecture/missing-config.md"},
        config,
    )
    if not any(item.startswith("missing:") for item in config_missing):
        fail("missing config path did not fail the check")
    config_removed = indirect_reader_problems(config - {move_map}, config)
    if not any(item.startswith("drift ") for item in config_removed):
        fail("removed config-named docs path did not fail the drift check")


def check_consumed_markdown() -> None:
    """Pinned markdown stays wide. A prose sample still narrows."""
    route = load_router()
    derived = guarded_step_markdown()
    pinned = set(route.MACHINE_CONSUMED_MARKDOWN)
    if derived != pinned:
        missing = sorted(derived - pinned)
        extra = sorted(pinned - derived)
        fail(f"machine-consumed markdown pin drifted missing={missing} extra={extra}")
    if len(pinned) != 73:
        fail(f"machine-consumed markdown pin count {len(pinned)}, expected 73")
    prose = PROSE_DOC
    if prose in pinned or route.path_kind(prose) != "docs":
        fail(f"{prose} no longer samples prose docs_only")
    required = (
        "docs/ci.md",
        "docs/status/SUPPORT_TIERS.md",
        "docs/specs/CARGO-ALLOW-SPEC-0009-design-to-proof-walking-skeleton.md",
        "docs/specs/CARGO-ALLOW-SPEC-0011-three-product-convergence.md",
    )
    for path in required:
        if path not in pinned or route.path_kind(path) != "other":
            fail(f"{path} is not pinned source")
        assert_plan(
            f"{path} forces a wide plan",
            fixture("pull_request", None, f"M\t{path}\n"),
            "source_bearing",
            False,
        )
    assert_plan(
        "unpinned markdown stays docs-only",
        fixture("pull_request", None, f"M\t{prose}\n"),
        "docs_only",
        True,
    )
    assert_plan(
        "pinned markdown beside prose stays wide",
        fixture("pull_request", None, f"M\t{required[0]}\nM\t{prose}\n"),
        "source_bearing",
        False,
    )


def check_machine_docs() -> None:
    """Machine-read docs stay source. Markdown prose can still narrow."""
    route = load_router()
    expected = {
        "docs/ci.md": "other",
        "docs/README.md": "other",
        "docs/status/SUPPORT_TIERS.md": "other",
        "docs/specs/CARGO-ALLOW-SPEC-0009-design-to-proof-walking-skeleton.md": "other",
        "docs/architecture/repo-snapshot.md": "other",
        "docs/architecture/repo-protocol.md": "other",
        "docs/architecture/cargo-intent.md": "other",
        "docs/architecture/product-package-topology.md": "other",
        "docs/architecture/extraction-shims.md": "other",
        "docs/architecture/product-move-map.md": "other",
        "docs/architecture/new-reader.md": "other",
        "docs/architecture/nested/new-reader.md": "docs",
        "docs/how-to/run-changie-sensor.md": "other",
        "docs/release/README.md": "other",
        "docs/ci/receipts/direct-floor-proof-cargo-allow-v1.selection.md": "docs",
        ".changes/note.yaml": "other",
        ".changes/v0.2.0.md": "other",
        "README.md": "other",
        "SUPPORT.md": "other",
        "CHANGELOG.md": "docs",
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
        f"M\tdocs/assets/feature-configuration-matrix-v1.json\nM\t{PROSE_DOC}\n",
    )
    assert_plan("machine docs beside prose stays wide", mixed, "source_bearing", False)
    successor = fixture(
        "pull_request",
        "M\tdocs/ci/receipts/direct-floor-proof-cargo-allow-v1.json\n",
        f"M\t{PROSE_DOC}\n",
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
    check_consumed_markdown()
    check_indirect_readers()
    check_helper_readers()
    check_comment_guard_fixture()
    check_contract()
    check_workflow()
    check_git_history()
    check_machine_docs_git()
    check_predecessor_policy()
    check_matrix_job_names()
    print("ci-proof-route controls passed")


if __name__ == "__main__":
    main()
