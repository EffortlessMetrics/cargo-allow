#!/usr/bin/env python3
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("closeout", ROOT / "scripts/verify-campaign-issue-closeout.py")
closeout = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(closeout)


class FakeApi:
    def __init__(self, responses):
        self.responses = responses
        self.calls = []

    def request(self, path, method="GET", payload=None):
        self.calls.append((path, method, payload))
        return self.responses.get((path, method), self.responses.get(path, []))


def event(body, number=3846):
    return {"action": "closed", "issue": {"number": number, "body": body}}


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
        self.assertEqual(closeout.handle(event(""), api, membership, "main"), 0)
        self.assertEqual([call[1] for call in api.calls], ["GET", "POST", "PATCH"])
        comment = api.calls[1][2]["body"]
        api.responses["/issues/3846/comments?per_page=100"] = [{"body": comment}]
        api.calls.clear()
        closeout.handle(event(""), api, membership, "main")
        self.assertEqual([call[1] for call in api.calls], ["GET", "PATCH"])

    def test_complete_requires_reachable_merged_pr(self):
        body = closeout.MARKER + "\n```json\n" + json.dumps({
            "schema_id": closeout.SCHEMA,
            "issue": 3846,
            "result": "Complete",
            "closeout_id": "CARGO-ALLOW-CLOSEOUT-3846",
            "merged_pr": 3854,
        }) + "\n```"
        api = FakeApi({
            "/pulls/3854": {
                "state": "closed", "merged_at": "2026-08-25T00:00:00Z",
                "base": {"ref": "main"}, "merge_commit_sha": "a" * 40,
            },
            "/compare/" + "a" * 40 + "...main": {"status": "behind"},
            "/issues/3846/comments?per_page=100": [],
        })
        closeout.handle(event(body), api, {3846: ("Complete", {"Complete"})}, "main")
        self.assertTrue(any(call[1] == "PATCH" for call in api.calls))

        api = FakeApi({
            "/pulls/3854": {
                "state": "closed", "merged_at": "2026-08-25T00:00:00Z",
                "base": {"ref": "main"}, "merge_commit_sha": "a" * 40,
            },
            "/compare/" + "a" * 40 + "...main": {"status": "ahead"},
        })
        closeout.handle(event(body), api, {3846: ("Complete", {"Complete"})}, "main")
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
            closeout.handle(event(body), api, membership, "main")
            self.assertEqual(api.calls, [])

    def test_malformed_marker_is_reopened_as_instrument_failure(self):
        api = FakeApi({"/issues/3846/comments?per_page=100": []})
        closeout.handle(event(closeout.MARKER), api, {3846: ("Complete", {"Complete"})}, "main")
        self.assertIn("instrument_failure", api.calls[1][2]["body"])

    def test_unrelated_issue_is_untouched(self):
        api = FakeApi({})
        closeout.handle(event("", number=9999), api, {3846: ("Complete", {"Complete"})}, "main")
        self.assertEqual(api.calls, [])


if __name__ == "__main__":
    unittest.main()
