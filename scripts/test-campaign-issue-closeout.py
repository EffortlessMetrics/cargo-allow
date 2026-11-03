#!/usr/bin/env python3
import importlib.util
import json
import os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import subprocess
import sys
import threading
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("closeout", ROOT / "scripts/verify-campaign-issue-closeout.py")
closeout = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(closeout)

UNREAD_INVENTORY = Path("policy/evidence-surface-inventory.toml")


class FakeApi:
    def __init__(self, responses):
        self.responses = responses
        self.calls = []

    def request(self, path, method="GET", payload=None):
        self.calls.append((path, method, payload))
        return self.responses.get((path, method), self.responses.get(path, []))


def event(body, number=3846):
    return {"action": "closed", "issue": {"number": number, "body": body}}


def write_inventory(directory, rows):
    path = Path(directory) / "inventory.toml"
    lines = [f'schema = "{closeout.INVENTORY_SCHEMA}"']
    for surface_id, evidence_class in rows:
        lines.append("\n[[surfaces]]")
        lines.append(f'id = "{surface_id}"')
        lines.append(f'evidence_class = "{evidence_class}"')
    path.write_text("\n".join(lines) + "\n")
    return path


def complete_body(evidence_surfaces):
    payload = {
        "schema_id": closeout.SCHEMA,
        "issue": 3846,
        "result": "Complete",
        "closeout_id": "CARGO-ALLOW-CLOSEOUT-3846",
        "merged_pr": 3854,
    }
    if evidence_surfaces is not None:
        payload["evidence_surfaces"] = evidence_surfaces
    return closeout.MARKER + "\n```json\n" + json.dumps(payload) + "\n```"


MERGED_PR_RESPONSES = {
    "/pulls/3854": {
        "state": "closed", "merged_at": "2026-08-25T00:00:00Z",
        "base": {"ref": "main"}, "merge_commit_sha": "a" * 40,
    },
    "/compare/" + "a" * 40 + "...main": {"status": "ahead"},
    "/issues/3846/comments?per_page=100": [],
}


class CloseoutTests(unittest.TestCase):
    def test_membership_rejects_duplicates(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "membership.toml"
            path.write_text("[[children]]\nissue=1\nrequired='Complete'\naccepted=['Complete']\n[[children]]\nissue=1\nrequired='Complete'\naccepted=['Complete']\n")
            with self.assertRaises(ValueError):
                closeout.load_membership(path)

    def test_missing_payload_reopens_once_and_does_not_duplicate_comment(self):
        api = FakeApi({"/issues/3846/comments?per_page=100": []})
        membership = {3846: ("Complete", {"Complete", "NotPlanned", "Duplicate"})}
        self.assertEqual(closeout.handle(event(""), api, membership, "main", UNREAD_INVENTORY), 0)
        self.assertEqual([call[1] for call in api.calls], ["GET", "POST", "PATCH"])
        comment = api.calls[1][2]["body"]
        api.responses["/issues/3846/comments?per_page=100"] = [{"body": comment}]
        api.calls.clear()
        closeout.handle(event(""), api, membership, "main", UNREAD_INVENTORY)
        self.assertEqual([call[1] for call in api.calls], ["GET", "PATCH"])

    def test_complete_requires_reachable_merged_pr(self):
        with tempfile.TemporaryDirectory() as directory:
            inventory = write_inventory(directory, [("typed-surface", "TypedModelValidation")])
            body = complete_body(["typed-surface"])
            api = FakeApi({
                "/pulls/3854": {
                    "state": "closed", "merged_at": "2026-08-25T00:00:00Z",
                    "base": {"ref": "main"}, "merge_commit_sha": "a" * 40,
                },
                "/compare/" + "a" * 40 + "...main": {"status": "behind"},
                "/issues/3846/comments?per_page=100": [],
            })
            closeout.handle(event(body), api, {3846: ("Complete", {"Complete"})}, "main", inventory)
            self.assertTrue(any(call[1] == "PATCH" for call in api.calls))
            self.assertIn("merge_commit_not_reachable_from_main", api.calls[-2][2]["body"])

            api = FakeApi(dict(MERGED_PR_RESPONSES))
            closeout.handle(event(body), api, {3846: ("Complete", {"Complete"})}, "main", inventory)
            self.assertEqual(api.calls, [
                ("/pulls/3854", "GET", None),
                ("/compare/" + "a" * 40 + "...main", "GET", None),
            ])

    def test_accepted_non_code_outcomes_do_not_call_github(self):
        membership = {3846: ("Complete", {"Complete", "NotPlanned", "Duplicate"})}
        for payload in (
            {"schema_id": closeout.SCHEMA, "issue": 3846, "result": "NotPlanned", "reason": "superseded"},
            {"schema_id": closeout.SCHEMA, "issue": 3846, "result": "Duplicate", "replacement_issue": 3851},
        ):
            body = closeout.MARKER + "\n```json\n" + json.dumps(payload) + "\n```"
            api = FakeApi({})
            closeout.handle(event(body), api, membership, "main", UNREAD_INVENTORY)
            self.assertEqual(api.calls, [])

    def test_malformed_marker_is_reopened_as_instrument_failure(self):
        api = FakeApi({"/issues/3846/comments?per_page=100": []})
        closeout.handle(event(closeout.MARKER), api, {3846: ("Complete", {"Complete"})}, "main", UNREAD_INVENTORY)
        self.assertIn("instrument_failure", api.calls[1][2]["body"])

    def test_unrelated_issue_is_untouched(self):
        api = FakeApi({})
        closeout.handle(event("", number=9999), api, {3846: ("Complete", {"Complete"})}, "main", UNREAD_INVENTORY)
        self.assertEqual(api.calls, [])

    def test_complete_without_evidence_surfaces_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            inventory = write_inventory(directory, [("typed-surface", "TypedModelValidation")])
            api = FakeApi(dict(MERGED_PR_RESPONSES))
            closeout.handle(event(complete_body(None)), api, {3846: ("Complete", {"Complete"})}, "main", inventory)
            self.assertTrue(any(call[1] == "PATCH" for call in api.calls))
            self.assertIn("evidence_surfaces_missing", api.calls[-2][2]["body"])

    def test_complete_rejects_unknown_evidence_surface(self):
        with tempfile.TemporaryDirectory() as directory:
            inventory = write_inventory(directory, [("typed-surface", "TypedModelValidation")])
            api = FakeApi(dict(MERGED_PR_RESPONSES))
            closeout.handle(event(complete_body(["not-in-inventory"])), api, {3846: ("Complete", {"Complete"})}, "main", inventory)
            self.assertTrue(any(call[1] == "PATCH" for call in api.calls))
            self.assertIn("evidence_surface_unknown", api.calls[-2][2]["body"])

    def test_complete_rejects_insufficient_evidence_classes(self):
        # Negative control 12 (#3810): acceptance backed only by
        # LexicalProjectionOnly cannot manufacture Complete.
        with tempfile.TemporaryDirectory() as directory:
            inventory = write_inventory(directory, [
                ("lexical-a", "LexicalProjectionOnly"),
                ("lexical-b", "LexicalProjectionOnly"),
                ("historical", "HistoricalFixtureOnly"),
                ("typed", "TypedModelValidation"),
            ])
            for declared in (
                ["lexical-a"],
                ["lexical-a", "lexical-b"],
                ["lexical-a", "historical"],
            ):
                api = FakeApi(dict(MERGED_PR_RESPONSES))
                closeout.handle(event(complete_body(declared)), api, {3846: ("Complete", {"Complete"})}, "main", inventory)
                self.assertTrue(any(call[1] == "PATCH" for call in api.calls), declared)
                self.assertIn("insufficient_evidence_class", api.calls[-2][2]["body"], declared)

            api = FakeApi(dict(MERGED_PR_RESPONSES))
            closeout.handle(event(complete_body(["lexical-a", "typed"])), api, {3846: ("Complete", {"Complete"})}, "main", inventory)
            self.assertFalse(any(call[1] == "PATCH" for call in api.calls))

    def test_complete_rejects_invalid_surface_lists(self):
        with tempfile.TemporaryDirectory() as directory:
            inventory = write_inventory(directory, [("typed-surface", "TypedModelValidation")])
            for declared in (
                ["typed-surface", "typed-surface"],
                [""],
                ["   "],
                [42],
            ):
                api = FakeApi(dict(MERGED_PR_RESPONSES))
                closeout.handle(event(complete_body(declared)), api, {3846: ("Complete", {"Complete"})}, "main", inventory)
                self.assertTrue(any(call[1] == "PATCH" for call in api.calls), declared)
                self.assertIn("evidence_surfaces_invalid", api.calls[-2][2]["body"], declared)

    def test_complete_rejects_unknown_evidence_class_as_insufficient(self):
        # A class this guard does not recognize cannot be assumed to prove
        # the named authority; only the named sufficient classes back a
        # Complete closeout.
        with tempfile.TemporaryDirectory() as directory:
            inventory = write_inventory(directory, [
                ("typo", "TypedModelValidaton"),
                ("lexical", "LexicalProjectionOnly"),
                ("structured", "StructuredShapeValidation"),
            ])
            api = FakeApi(dict(MERGED_PR_RESPONSES))
            closeout.handle(event(complete_body(["typo"])), api, {3846: ("Complete", {"Complete"})}, "main", inventory)
            self.assertTrue(any(call[1] == "PATCH" for call in api.calls))
            self.assertIn("insufficient_evidence_class", api.calls[-2][2]["body"])

            api = FakeApi(dict(MERGED_PR_RESPONSES))
            closeout.handle(event(complete_body(["typo", "lexical"])), api, {3846: ("Complete", {"Complete"})}, "main", inventory)
            self.assertTrue(any(call[1] == "PATCH" for call in api.calls))
            self.assertIn("insufficient_evidence_class", api.calls[-2][2]["body"])

            api = FakeApi(dict(MERGED_PR_RESPONSES))
            closeout.handle(event(complete_body(["typo", "structured"])), api, {3846: ("Complete", {"Complete"})}, "main", inventory)
            self.assertFalse(any(call[1] == "PATCH" for call in api.calls))

    def test_non_table_inventory_row_is_instrument_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            inventory = Path(directory) / "inventory.toml"
            inventory.write_text(
                'schema = "'
                + closeout.INVENTORY_SCHEMA
                + '"\n\n[[surfaces]]\n"bad"\n'
            )
            api = FakeApi(dict(MERGED_PR_RESPONSES))
            closeout.handle(event(complete_body(["typed-surface"])), api, {3846: ("Complete", {"Complete"})}, "main", inventory)
            self.assertTrue(any(call[1] == "PATCH" for call in api.calls))
            self.assertIn("instrument_failure", api.calls[-2][2]["body"])

    def test_malformed_inventory_is_instrument_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            inventory = Path(directory) / "inventory.toml"
            inventory.write_text("schema = 'wrong.schema'\n")
            api = FakeApi(dict(MERGED_PR_RESPONSES))
            closeout.handle(event(complete_body(["typed-surface"])), api, {3846: ("Complete", {"Complete"})}, "main", inventory)
            self.assertTrue(any(call[1] == "PATCH" for call in api.calls))
            self.assertIn("instrument_failure", api.calls[-2][2]["body"])



VERIFIER = Path(__file__).resolve().with_name("verify-campaign-issue-closeout.py")
PREFIX = "/repos/EffortlessMetrics/cargo-allow"
ISSUE_PATH = f"{PREFIX}/issues/3846"
COMMENT_PATH = f"{ISSUE_PATH}/comments"
COMMENT_GET = f"{COMMENT_PATH}?per_page=100"
PR_PATH = f"{PREFIX}/pulls/4147"
MERGE_SHA = "a" * 40
COMPARE_PATH = f"{PREFIX}/compare/{MERGE_SHA}...main"
MARKER = "<!-- cargo-allow:campaign-closeout.v1 -->"

class TestCampaignCloseoutRequestLaw(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory(prefix="closeout-request-law-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.policy = self.root / "policy"
        self.policy.mkdir()
        self.membership = self.policy / "campaign-issue-closeout.toml"
        self.membership.write_text(
            '[[children]]\nissue=3846\nrequired="Complete"\n', encoding="utf-8",
        )
        self.inventory = self.policy / "evidence-surface-inventory.toml"
        self.set_evidence_class("TypedModelValidation")
        self.pr = {
            "merged_at": "2026-11-02T00:00:00Z",
            "state": "closed",
            "base": {"ref": "main"},
            "merge_commit_sha": MERGE_SHA,
        }
        self.comparison = {"status": "identical"}
        self.requests: list[dict] = []
        self.requests_lock = threading.Lock()
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_args) -> None:
                pass

            def handle_api(self) -> None:
                raw_length = self.headers.get("Content-Length", "0")
                try:
                    length = int(raw_length)
                except ValueError:
                    length = -1
                if not 0 <= length <= 8192:
                    self.send_error(413, "request body exceeds bounded fixture cap")
                    return
                # A permitted body length is not a permitted infinite read.
                self.connection.settimeout(5.0)
                try:
                    raw = self.rfile.read(length)
                except (TimeoutError, OSError):
                    return
                try:
                    body = json.loads(raw) if raw else None
                except json.JSONDecodeError:
                    self.send_error(400, "invalid JSON request")
                    return
                record = {"method": self.command, "path": self.path, "body": body}
                # Record the complete request *before* sending the response;
                # no eventual logging race or silent failed log read.
                with owner.requests_lock:
                    owner.requests.append(record)
                if self.command == "GET" and self.path == PR_PATH:
                    response, status = owner.pr, 200
                elif self.command == "GET" and self.path == COMPARE_PATH:
                    response, status = owner.comparison, 200
                elif self.command == "GET" and self.path == COMMENT_GET:
                    response, status = [], 200
                elif self.command == "POST" and self.path == COMMENT_PATH:
                    response, status = {}, 200
                elif self.command == "PATCH" and self.path == ISSUE_PATH:
                    response, status = {}, 200
                else:
                    response, status = {"error": "unrecognized fixture request"}, 404
                payload = json.dumps(response).encode("utf-8")
                self.send_response(status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)

            do_GET = handle_api
            do_POST = handle_api
            do_PATCH = handle_api
            do_PUT = handle_api
            do_DELETE = handle_api

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.server.daemon_threads = False
        self.thread = threading.Thread(target=self.server.serve_forever, kwargs={"poll_interval": 0.05})
        self.thread.start()
        self.addCleanup(self.shutdown_server)

    def shutdown_server(self) -> None:
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=3)
        self.assertFalse(self.thread.is_alive(), "loopback HTTP listener was not reaped")

    def set_evidence_class(self, evidence_class: str) -> None:
        self.inventory.write_text(
            'schema="cargo-allow.evidence-surface-inventory.v1"\n'
            '[[surfaces]]\nid="release-authorization-v1"\n'
            f'evidence_class="{evidence_class}"\n', encoding="utf-8",
        )

    @staticmethod
    def complete_body() -> str:
        payload = {
            "schema_id": "cargo-allow.campaign-issue-closeout.v1",
            "issue": 3846,
            "result": "Complete",
            "merged_pr": 4147,
            "closeout_id": "fixture-closeout-3846-controls",
            "evidence_surfaces": ["release-authorization-v1"],
        }
        return f"closeout\n\n{MARKER}\n```json\n{json.dumps(payload)}\n```\n"

    def execute(self, body: str, issue: int = 3846, verifier: Path = VERIFIER) -> list[dict]:
        with self.requests_lock:
            self.requests.clear()
        event = self.root / "event.json"
        event.write_text(
            json.dumps({"action": "closed", "issue": {"number": issue, "body": body}}),
            encoding="utf-8",
        )
        env = {k: v for k, v in os.environ.items() if not k.upper().endswith("_PROXY")}
        env.update({
            "GITHUB_EVENT_PATH": str(event),
            "GITHUB_API_URL": f"http://127.0.0.1:{self.server.server_port}",
            "GITHUB_REPOSITORY": "EffortlessMetrics/cargo-allow",
            "GITHUB_TOKEN": "fixture-token",
            "NO_PROXY": "127.0.0.1",
        })
        result = subprocess.run(
            [sys.executable, str(verifier), "--event", str(event),
             "--membership", str(self.membership), "--inventory", str(self.inventory)],
            cwd=self.root, env=env, text=True, capture_output=True, timeout=15,
            check=False,
        )
        self.assertEqual(result.returncode, 0, f"verifier failed: {result.stderr}")
        self.assertEqual(result.stderr, "", "verifier emitted unexpected stderr")
        with self.requests_lock:
            return list(self.requests)

    def assert_reopen(self, records: list[dict], code: str, expected_reads: list[str]) -> None:
        expected = ([{"method": "GET", "path": path, "body": None}
                     for path in expected_reads] + [
            {"method": "GET", "path": COMMENT_GET, "body": None},
            {"method": "POST", "path": COMMENT_PATH, "body": records[-2]["body"]},
            {"method": "PATCH", "path": ISSUE_PATH,
             "body": {"state": "open", "state_reason": "reopened"}},
        ])
        self.assertEqual(records, expected, "reopen must write only bounded comment+exact PATCH")
        comment = records[-2]["body"]
        self.assertIsInstance(comment, dict)
        self.assertEqual(set(comment), {"body"})
        self.assertIn(MARKER, comment["body"])
        self.assertIn("Issue: #3846", comment["body"])
        self.assertIn(f"Codes: `{code}`", comment["body"])
        self.assertLess(len(comment["body"]), 2048)

    def assert_accepted(self, records: list[dict]) -> None:
        self.assertEqual(records, [
            {"method": "GET", "path": PR_PATH, "body": None},
            {"method": "GET", "path": COMPARE_PATH, "body": None},
        ], "Complete must consult the exact PR and reachability and perform zero writes")

    def test_missing_record_reopens_with_exact_payload_before_pr_reads(self) -> None:
        self.assert_reopen(self.execute("no closeout record"), "missing_closeout", [])

    def test_complete_reads_pr_and_reachability_without_any_write(self) -> None:
        self.assert_accepted(self.execute(self.complete_body()))

    def test_nonmember_is_strict_no_request_no_write(self) -> None:
        self.assertEqual(self.execute("outside campaign denominator", issue=999999), [])

    def test_unmerged_and_wrong_base_reject_without_compare(self) -> None:
        self.pr["merged_at"] = None
        self.pr["state"] = "closed"
        self.assert_reopen(
            self.execute(self.complete_body()), "pull_request_not_merged", [PR_PATH],
        )
        self.pr["merged_at"] = "2026-11-02T00:00:00Z"
        self.pr["state"] = "open"
        self.assert_reopen(
            self.execute(self.complete_body()), "pull_request_not_merged", [PR_PATH],
        )
        self.pr["state"] = "closed"
        self.pr["base"] = {"ref": "other"}
        self.assert_reopen(
            self.execute(self.complete_body()), "pull_request_wrong_base", [PR_PATH],
        )

    def test_unreachable_and_insufficient_evidence_reject(self) -> None:
        self.comparison = {"status": "behind"}
        self.assert_reopen(
            self.execute(self.complete_body()), "merge_commit_not_reachable_from_main",
            [PR_PATH, COMPARE_PATH],
        )
        self.comparison = {"status": "identical"}
        self.set_evidence_class("LexicalProjectionOnly")
        self.assert_reopen(
            self.execute(self.complete_body()), "insufficient_evidence_class",
            [PR_PATH, COMPARE_PATH],
        )

    def test_oracle_rejects_three_known_counterfactuals(self) -> None:
        source = VERIFIER.read_text(encoding="utf-8")
        cases = [
            ("wrong_reopen_payload",
             'payload={"state": "open", "state_reason": "reopened"}',
             'payload={"state": "closed", "state_reason": "completed"}',
             "no closeout record", "reopen"),
            ("missing_merge_reachability",
             'comparison = api.request(f"/compare/{merge_sha}...{base_branch}")',
             'comparison = {"status": "identical"}',
             self.complete_body(), "accept"),
            ("unexpected_accepted_write",
             '    if not errors:\n        return 0\n    comment = bounded_comment',
             '    if not errors:\n        api.request(f"/issues/{number}/comments", method="POST", payload={"body": "unexpected write"})\n        return 0\n    comment = bounded_comment',
             self.complete_body(), "accept"),
        ]
        for name, before, after, event_body, oracle in cases:
            with self.subTest(name=name):
                self.assertEqual(source.count(before), 1, f"mutation seam moved: {name}")
                mutant = self.root / f"{name}.py"
                mutant.write_text(source.replace(before, after), encoding="utf-8")
                records = self.execute(event_body, verifier=mutant)
                with self.assertRaises(AssertionError, msg=f"oracle let {name} pass"):
                    if oracle == "reopen":
                        self.assert_reopen(records, "missing_closeout", [])
                    else:
                        self.assert_accepted(records)




if __name__ == "__main__":
    unittest.main()
