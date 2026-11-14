#!/usr/bin/env python3
"""Tests for the merge-group checkout-ref guard."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts/check-merge-group-checkout-refs.py"
SPEC = importlib.util.spec_from_file_location("check_merge_group_checkout_refs", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise SystemExit("could not load the checkout-ref guard")
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)

MERGE_GROUP = """\
on:
  pull_request:
  merge_group:
    types: [checks_requested]
"""


class CheckoutRefGuardTests(unittest.TestCase):
    def test_github_ref_without_merge_group_term_is_rejected(self) -> None:
        text = MERGE_GROUP + (
            "      - uses: actions/checkout@v4\n"
            "        with:\n"
            "          ref: ${{ github.event.pull_request.head.sha || github.ref }}\n"
        )
        found = GUARD.offending_refs(text)
        self.assertEqual(len(found), 1)
        self.assertIn("github.ref", found[0][1])

    def test_github_head_ref_without_merge_group_term_is_rejected(self) -> None:
        text = MERGE_GROUP + "          ref: ${{ github.head_ref }}\n"
        self.assertEqual(len(GUARD.offending_refs(text)), 1)

    def test_merge_group_head_sha_form_passes(self) -> None:
        text = MERGE_GROUP + (
            "          ref: ${{ github.event.pull_request.head.sha || "
            "github.event.merge_group.head_sha || github.sha }}\n"
        )
        self.assertEqual(GUARD.offending_refs(text), [])

    def test_github_sha_form_passes(self) -> None:
        text = MERGE_GROUP + "          ref: ${{ github.sha }}\n"
        self.assertEqual(GUARD.offending_refs(text), [])

    def test_workflow_without_merge_group_is_not_scanned(self) -> None:
        text = (
            "on:\n  push:\n    branches: [main]\n"
            "          ref: ${{ github.ref }}\n"
        )
        self.assertEqual(GUARD.offending_refs(text), [])

    def test_merge_group_term_in_the_ref_passes(self) -> None:
        text = MERGE_GROUP + (
            "          ref: ${{ github.event.merge_group.head_sha || github.ref }}\n"
        )
        self.assertEqual(GUARD.offending_refs(text), [])


if __name__ == "__main__":
    unittest.main()
