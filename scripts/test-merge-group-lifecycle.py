#!/usr/bin/env python3
"""Exercise actual Git subjects and queue-only workflow wiring (#4602)."""

import copy
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest

import yaml

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts/merge-group-subject.py"
SPEC = importlib.util.spec_from_file_location("merge_group_subject", SCRIPT)
SUBJECT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SUBJECT)
QUEUE_ROOTS = ("ci.yml", "frozen-subject-lock.yml", "sentinel.yml", "ub-review.yml")
DEVELOPMENT_EVENTS = {
    "pull_request", "pull_request_target", "pull_request_review",
    "pull_request_review_comment", "issue_comment", "check_run", "check_suite",
}


def triggers(workflow):
    value = workflow["on"]
    if isinstance(value, str):
        return {value: None}
    if isinstance(value, list):
        return dict.fromkeys(value)
    if not isinstance(value, dict):
        raise AssertionError("workflow triggers must be a mapping/list/string")
    return value


def check_queue_root(workflow):
    events = triggers(workflow)
    # The queue root must publish on merge_group; pull_request, push and
    # workflow_dispatch are retained for the first queue landing and main health.
    assert "merge_group" in events, events
    assert set(events) <= {"merge_group", "pull_request", "push", "workflow_dispatch"}, events
    assert events["merge_group"]["types"] == ["checks_requested"]
    assert events["merge_group"]["branches"] == ["main"]
    for job in workflow["jobs"].values():
        for step in job.get("steps", []):
            if step.get("uses", "").startswith("actions/checkout@"):
                ref = step.get("with", {}).get("ref")
                if ref is not None:
                    # The queue root must bind the actual G, not a bare H.
                    assert (
                        "github.event.merge_group.head_sha" in ref
                        or "github.ref" in ref
                    ), ref
                assert step.get("with", {}).get("persist-credentials") == "false"


class GitSubjectTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="queue-subject-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-b", "main")
        self.git("config", "user.name", "Fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        (self.root / "base.txt").write_text("base\n")
        self.git("add", ".")
        self.git("commit", "-m", "base")
        self.base = self.git("rev-parse", "HEAD")
        self.git("checkout", "-b", "predecessor")
        (self.root / "predecessor.txt").write_text("preceding change\n")
        self.git("add", ".")
        self.git("commit", "-m", "predecessor")
        self.predecessor = self.git("rev-parse", "HEAD")
        self.git("checkout", "-b", "candidate", "main")
        (self.root / "candidate.txt").write_text("selected change\n")
        self.git("add", ".")
        self.git("commit", "-m", "candidate H")
        self.head = self.git("rev-parse", "HEAD")
        self.git("checkout", "-b", "queue", "main")
        self.git("merge", "--no-ff", "--no-edit", "predecessor")
        self.git("merge", "--no-ff", "--no-edit", "candidate")
        self.group = self.git("rev-parse", "HEAD")
        self.env = dict(os.environ, GITHUB_EVENT_NAME="merge_group",
                        GITHUB_REPOSITORY="owner/repo", GITHUB_SHA=self.group,
                        GITHUB_REF="refs/heads/gh-readonly-queue/main/pr-2-fixture",
                        GITHUB_RUN_ID="123", GITHUB_RUN_ATTEMPT="1")
        self.event = {
            "action": "checks_requested", "repository": {"full_name": "owner/repo"},
            "merge_group": {"base_ref": "refs/heads/main", "base_sha": self.base,
                            "head_ref": self.env["GITHUB_REF"], "head_sha": self.group},
        }

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.root,
                                       text=True, stderr=subprocess.DEVNULL).strip()

    def run_subject(self, event=None, env=None, mode="subject"):
        event_file = self.root / "event.json"
        event_file.write_text(json.dumps(event if event is not None else self.event))
        environment = dict(self.env if env is None else env,
                           GITHUB_EVENT_PATH=str(event_file))
        output = self.root / f"{mode}.json"
        output.unlink(missing_ok=True)
        result = subprocess.run([sys.executable, str(SCRIPT), mode, "--receipt", str(output)],
                                cwd=self.root, env=environment, capture_output=True, text=True)
        receipt = json.loads(output.read_text()) if output.exists() else None
        return result, receipt

    def test_actual_composition_passes_and_receipt_names_G_not_H(self):
        result, receipt = self.run_subject()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(receipt["head_sha"], self.group)
        self.assertNotEqual(receipt["head_sha"], self.head)
        self.assertEqual(receipt["base_sha"], self.base)
        self.assertEqual(receipt["tree_sha"], self.git("rev-parse", "HEAD^{tree}"))
        self.assertEqual(receipt["parents"], self.git("show", "-s", "--format=%P", "HEAD").split())
        self.assertEqual(self.git("show", "HEAD:predecessor.txt"), "preceding change")
        self.assertEqual(self.git("show", "HEAD:candidate.txt"), "selected change")

    def test_checkout_of_H_cannot_stand_in_for_G(self):
        self.git("checkout", "candidate")
        result, receipt = self.run_subject()
        self.assertNotEqual(result.returncode, 0)
        self.assertIsNone(receipt)
        self.assertIn("actual merge-group G differ", result.stderr)

    def test_event_environment_repository_ref_and_identity_must_agree(self):
        for key, value in (("GITHUB_SHA", self.head), ("GITHUB_REPOSITORY", "other/repo"),
                           ("GITHUB_REF", "refs/heads/main"), ("GITHUB_RUN_ATTEMPT", "")):
            with self.subTest(key=key):
                result, receipt = self.run_subject(env=dict(self.env, **{key: value}))
                self.assertNotEqual(result.returncode, 0)
                self.assertIsNone(receipt)

    def test_all_nonqueue_events_refuse_subject_proof(self):
        for event in sorted(DEVELOPMENT_EVENTS | {"push", "schedule", "workflow_dispatch"}):
            with self.subTest(event=event):
                result, receipt = self.run_subject(env=dict(self.env, GITHUB_EVENT_NAME=event))
                self.assertNotEqual(result.returncode, 0)
                self.assertIsNone(receipt)

    def test_wrong_action_missing_or_unknown_base_refuses(self):
        for patch in ({"action": "destroyed"},
                      {"merge_group": dict(self.event["merge_group"], base_sha="0" * 40)},
                      {"merge_group": dict(self.event["merge_group"], base_ref="refs/heads/other")},
                      {"merge_group": dict(self.event["merge_group"], base_sha=self.group)},
                      {"merge_group": None}):
            with self.subTest(patch=patch):
                result, receipt = self.run_subject(event=dict(self.event, **patch))
                self.assertNotEqual(result.returncode, 0)
                self.assertIsNone(receipt)

    def test_known_nonancestor_base_is_rejected(self):
        self.git("checkout", "-b", "unrelated", "main")
        (self.root / "unrelated.txt").write_text("not in G\n")
        self.git("add", ".")
        self.git("commit", "-m", "nonancestor")
        foreign = self.git("rev-parse", "HEAD")
        self.git("checkout", "queue")
        event = copy.deepcopy(self.event)
        event["merge_group"]["base_sha"] = foreign
        result, receipt = self.run_subject(event=event)
        self.assertNotEqual(result.returncode, 0)
        self.assertIsNone(receipt)

    def test_changed_tracked_checkout_is_not_the_observed_tree(self):
        (self.root / "candidate.txt").write_text("uncommitted source\n")
        result, receipt = self.run_subject()
        self.assertNotEqual(result.returncode, 0)
        self.assertIsNone(receipt)

    def test_result_cli_binds_success_and_refuses_missing_producer(self):
        manifest_text = (ROOT / "docs/ci-lanes.toml").read_text()
        (self.root / "docs").mkdir()
        (self.root / "docs/ci-lanes.toml").write_text(manifest_text)
        manifest = tomllib.loads(manifest_text)
        required = {row["id"] for row in manifest["lane"] if row["posture"] == "required"}
        required = (required | {"queue-subject", "pregate"}) - {"queue-qualification"}
        needs = {name: {"result": "success"} for name in required}
        env = dict(self.env, QUEUE_NEEDS_JSON=json.dumps(needs))
        result, receipt = self.run_subject(mode="result", env=env)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(receipt["schema_id"], "cargo-allow.merge-group-result.v1")
        self.assertEqual(receipt["head_sha"], self.group)
        self.assertEqual(set(receipt["required_job_results"]), required)
        needs.pop("pregate")
        env["QUEUE_NEEDS_JSON"] = json.dumps(needs)
        result, receipt = self.run_subject(mode="result", env=env)
        self.assertNotEqual(result.returncode, 0)
        self.assertIsNone(receipt)

    def test_recomposition_cannot_reuse_the_preceding_event(self):
        self.git("checkout", "main")
        (self.root / "base-moved.txt").write_text("new base\n")
        self.git("add", "base-moved.txt")
        self.git("commit", "-m", "base moved")
        base2 = self.git("rev-parse", "HEAD")
        self.git("checkout", "-b", "queue2")
        self.git("merge", "--no-ff", "--no-edit", "predecessor")
        self.git("merge", "--no-ff", "--no-edit", "candidate")
        group2 = self.git("rev-parse", "HEAD")
        env = dict(self.env, GITHUB_SHA=group2)
        result, receipt = self.run_subject(env=env)
        self.assertNotEqual(result.returncode, 0)
        self.assertIsNone(receipt)
        event = copy.deepcopy(self.event)
        event["merge_group"].update(head_sha=group2, base_sha=base2)
        result, receipt = self.run_subject(env=env, event=event)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(receipt["head_sha"], group2)
        self.assertEqual(receipt["base_sha"], base2)


class RequiredResultTests(unittest.TestCase):
    def setUp(self):
        self.manifest = tomllib.loads((ROOT / "docs/ci-lanes.toml").read_text())
        required = {row["id"] for row in self.manifest["lane"] if row["posture"] == "required"}
        self.required = (required | {"queue-subject", "pregate"}) - {"queue-qualification"}
        self.needs = {name: {"result": "success"} for name in self.required}

    def test_complete_required_portfolio_succeeds(self):
        self.assertEqual(set(SUBJECT.required_results(self.needs, self.manifest)), self.required)

    def test_each_mandatory_producer_must_execute_successfully(self):
        for name in self.required:
            for value in ("failure", "skipped", "cancelled", "neutral", "", None):
                with self.subTest(lane=name, result=value):
                    needs = copy.deepcopy(self.needs)
                    needs[name]["result"] = value
                    with self.assertRaises(SUBJECT.InvalidSubject):
                        SUBJECT.required_results(needs, self.manifest)

    def test_missing_extra_or_malformed_records_fail(self):
        missing = dict(self.needs)
        missing.pop("pregate")
        malformed = dict(self.needs, pregate="success")
        for needs in (missing, malformed, None, dict(self.needs, unknown={"result": "success"})):
            with self.subTest(needs=needs):
                with self.assertRaises(SUBJECT.InvalidSubject):
                    SUBJECT.required_results(needs, self.manifest)


class WorkflowWiringTests(unittest.TestCase):
    def setUp(self):
        self.workflows = {p.name: yaml.load(p.read_text(), Loader=yaml.BaseLoader)
                          for p in (ROOT / ".github/workflows").glob("*.y*ml")}

    def test_every_ordinary_root_is_queue_only_and_checks_out_G(self):
        for name in QUEUE_ROOTS:
            with self.subTest(workflow=name):
                check_queue_root(self.workflows[name])

    def test_no_development_or_periodic_wrapper_survives(self):
        # Step 1 keeps pull_request alongside merge_group for the first queue
        # landing; the queue roots and sentinel are the deliberate exceptions.
        step_one_roots = set(QUEUE_ROOTS) | {"review-readiness.yml"}
        for name, workflow in self.workflows.items():
            with self.subTest(workflow=name):
                events = triggers(workflow)
                if name not in step_one_roots:
                    self.assertFalse(DEVELOPMENT_EVENTS & set(events))
                self.assertNotIn("schedule", events)
                if "push" in events:
                    if name == "release.yml":
                        self.assertEqual(events["push"], {"tags": ["v*"]})
                    elif name == "release-authorized.yml":
                        self.assertEqual(events["push"], {"branches": ["main"],
                            "paths": ["release/authorize-v0.2.0.json"]})
                    elif name in {"ci.yml", "review-readiness.yml", "sentinel.yml"}:
                        self.assertEqual(events["push"], {"branches": ["main"]})
                    else:
                        self.fail(f"ordinary push workflow survives: {name}")

    def test_regression_control_detects_missing_merge_group_and_bare_head_checkout(self):
        workflow = copy.deepcopy(self.workflows["ci.yml"])
        del workflow["on"]["merge_group"]
        with self.assertRaises(AssertionError):
            check_queue_root(workflow)
        workflow = copy.deepcopy(self.workflows["ci.yml"])
        workflow["jobs"]["dependency-graph-delta"]["steps"][0]["with"]["ref"] = "${{ github.event.pull_request.head.sha }}"
        with self.assertRaises(AssertionError):
            check_queue_root(workflow)

    def test_aggregate_covers_only_mandatory_portfolio_and_observes_all_results(self):
        manifest = tomllib.loads((ROOT / "docs/ci-lanes.toml").read_text())
        required = {row["id"] for row in manifest["lane"] if row["posture"] == "required"}
        required = (required | {"queue-subject", "pregate"}) - {"queue-qualification"}
        jobs = self.workflows["ci.yml"]["jobs"]
        aggregate = jobs["queue-qualification"]
        self.assertEqual(set(aggregate["needs"]), required)
        self.assertEqual(aggregate["if"], "always() && github.event_name == 'merge_group'")
        for name in required - {"queue-subject"}:
            needs = jobs[name]["needs"]
            self.assertIn("queue-subject", [needs] if isinstance(needs, str) else needs)
        result_step = next(step for step in aggregate["steps"] if "QUEUE_NEEDS_JSON" in step.get("env", {}))
        self.assertEqual(result_step["env"]["QUEUE_NEEDS_JSON"], "${{ toJSON(needs) }}")
        self.assertIn("merge-group-subject.py result", result_step["run"])

    def test_feature_and_dependency_routing_use_immutable_group_base(self):
        jobs = self.workflows["ci.yml"]["jobs"]
        routing = next(s for s in jobs["feature-configuration-proof"]["steps"] if s.get("id") == "routing")
        self.assertIn("github.event.merge_group.base_sha", routing["env"]["BASE_SHA"])
        self.assertIn("set -euo pipefail", routing["run"])
        self.assertNotIn("origin/", routing["run"])
        delta = next(s for s in jobs["dependency-graph-delta"]["steps"] if "GITHUB_BASE_SHA" in s.get("env", {}))
        self.assertIn("github.event.merge_group.base_sha", delta["env"]["GITHUB_BASE_SHA"])
        policy = next(s for s in jobs["test"]["steps"] if "QUEUE_BASE_SHA" in s.get("env", {}))
        self.assertIn("github.event.merge_group.base_sha", policy["env"]["QUEUE_BASE_SHA"])
        self.assertIn("github.event.merge_group.head_sha", policy["env"]["QUEUE_HEAD_SHA"])
        self.assertIn('--base "$QUEUE_BASE_SHA" --head "$QUEUE_HEAD_SHA"', policy["run"])
        self.assertNotIn("origin/", policy["run"])

    def test_feature_configuration_routing_base_falls_back_on_push(self):
        routing = next(
            s for s in self.workflows["ci.yml"]["jobs"]["feature-configuration-proof"]["steps"]
            if s.get("id") == "routing"
        )
        self.assertIn("github.event.before", routing["env"]["BASE_SHA"])
        run_lines = routing["run"].splitlines()
        start = next(i for i, line in enumerate(run_lines) if "${BASE_SHA//0/}" in line)
        end = next(i for i in range(start, len(run_lines)) if run_lines[i].strip() == "fi")
        guard = "\n".join(run_lines[start:end + 1])
        self.assertIn('BASE_SHA="$(git rev-parse HEAD~1)"', guard)

        bash = shutil.which("bash")
        if os.name == "nt":
            git_bash = Path(r"C:\Program Files\Git\bin\bash.exe")
            if git_bash.exists():
                bash = str(git_bash)
        if not bash:
            self.skipTest("bash is unavailable for the routing-base characterization")

        with tempfile.TemporaryDirectory(prefix="feature-routing-base-") as tmp:
            root = Path(tmp)
            subprocess.run(["git", "init", "-b", "main"], cwd=root, check=True, capture_output=True)
            subprocess.run(["git", "config", "user.name", "Fixture"], cwd=root, check=True)
            subprocess.run(["git", "config", "user.email", "fixture@example.invalid"], cwd=root, check=True)
            (root / "base.txt").write_text("base\n")
            subprocess.run(["git", "add", "."], cwd=root, check=True)
            subprocess.run(["git", "commit", "-m", "base"], cwd=root, check=True, capture_output=True)
            base = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
            (root / "head.txt").write_text("head\n")
            subprocess.run(["git", "add", "."], cwd=root, check=True)
            subprocess.run(["git", "commit", "-m", "head"], cwd=root, check=True, capture_output=True)

            script = guard + '\ngit rev-parse --verify "${BASE_SHA}^{commit}"\n'
            for label, value in [
                ("event.before", base),
                ("all zeros", "0" * 40),
                ("empty", ""),
            ]:
                with self.subTest(case=label):
                    env = dict(os.environ, BASE_SHA=value)
                    completed = subprocess.run(
                        [bash, "-c", script], cwd=root, env=env,
                        capture_output=True, text=True,
                    )
                    self.assertEqual(completed.returncode, 0, completed.stderr)
                    self.assertEqual(completed.stdout.strip(), base)

    def test_policy_change_evidence_resolves_each_event_pair(self):
        step = next(
            s for s in self.workflows["ci.yml"]["jobs"]["test"]["steps"]
            if "QUEUE_BASE_SHA" in s.get("env", {})
        )
        self.assertIn("github.event.pull_request.base.sha", step["env"]["QUEUE_BASE_SHA"])
        self.assertIn("github.event.merge_group.base_sha", step["env"]["QUEUE_BASE_SHA"])
        self.assertIn("github.event.before", step["env"]["QUEUE_BASE_SHA"])
        self.assertIn("github.event.pull_request.head.sha", step["env"]["QUEUE_HEAD_SHA"])
        self.assertIn("github.event.merge_group.head_sha", step["env"]["QUEUE_HEAD_SHA"])
        self.assertIn("github.sha", step["env"]["QUEUE_HEAD_SHA"])
        # The step must resolve a pair for every event; an event_name gate
        # would drop the only post-merge observation of a landed change.
        self.assertNotIn("if", step)
        self.assertNotIn("github.event_name ==", step["run"])

        bash = shutil.which("bash")
        if os.name == "nt":
            git_bash = Path(r"C:\Program Files\Git\bin\bash.exe")
            if git_bash.exists():
                bash = str(git_bash)
        if not bash:
            self.skipTest("bash is unavailable for the policy-change characterization")

        with tempfile.TemporaryDirectory(prefix="policy-change-evidence-") as tmp:
            root = Path(tmp)
            args = root / "cargo-args"
            base = "1" * 40
            head = "2" * 40
            script = 'cargo() { printf "%s\\n" "$*" > "$CARGO_ARGS_OUT"; }\n' + step["run"]
            for label, event, base_sha, expect_run in [
                ("merge-group", "merge_group", base, True),
                ("pull-request", "pull_request", base, True),
                ("push-before", "push", base, True),
                ("push-all-zeros", "push", "0" * 40, False),
                ("workflow-dispatch", "workflow_dispatch", "", False),
            ]:
                with self.subTest(case=label):
                    if args.exists():
                        args.unlink()
                    env = dict(
                        os.environ,
                        GITHUB_EVENT_NAME=event,
                        QUEUE_BASE_SHA=base_sha,
                        QUEUE_HEAD_SHA=head,
                        CARGO_ARGS_OUT=str(args),
                    )
                    completed = subprocess.run(
                        [bash, "-c", script], cwd=root, env=env,
                        capture_output=True, text=True,
                    )
                    self.assertEqual(completed.returncode, 0, completed.stderr)
                    self.assertEqual(args.exists(), expect_run, completed.stdout)
                    if expect_run:
                        self.assertIn(
                            f"diff --base {base_sha} --head {head}",
                            args.read_text(encoding="utf-8"),
                        )
                    else:
                        self.assertIn("no comparison base for", completed.stdout)

    def test_review_contracts_concurrency_distinguishes_pull_requests(self):
        workflow = self.workflows["ub-review.yml"]
        self.assertIn("pull_request", triggers(workflow))
        group = workflow["concurrency"]["group"]
        self.assertIn("github.event.merge_group.head_sha", group)
        # An empty suffix would put every PR in one group, so GitHub would
        # cancel older pending runs as new PRs arrive.
        self.assertIn("github.event.pull_request.number", group)
        self.assertEqual(workflow["concurrency"]["cancel-in-progress"], "false")


if __name__ == "__main__":
    unittest.main(verbosity=2)
