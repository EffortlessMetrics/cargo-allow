#!/usr/bin/env python3
import importlib.util
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("inventory", ROOT / "scripts/verify-evidence-surface-inventory.py")
inventory = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(inventory)


class InventoryTests(unittest.TestCase):
    def write_inventory(self, directory: Path, body: str) -> Path:
        path = directory / "inventory.toml"
        path.write_text(body, encoding="utf-8")
        return path

    def test_repository_inventory_is_complete(self):
        self.assertEqual(inventory.validate(ROOT, ROOT / "policy/evidence-surface-inventory.toml"), [])

    def test_unclassified_candidate_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tests = root / "crates/cargo-allow/tests"
            tests.mkdir(parents=True)
            (tests / "release_new_contract.rs").write_text('fn t() { require_contains("x"); }', encoding="utf-8")
            path = self.write_inventory(root, "[[surfaces]]\nid='x'\n")
            errors = inventory.validate(root, path)
            self.assertTrue(any("unclassified load-bearing test" in error for error in errors))

    def test_lexical_evidence_cannot_be_a_release_gate(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subject = root / "crates/cargo-allow/tests/release_contract.rs"
            subject.parent.mkdir(parents=True)
            subject.write_text('fn t() { require_contains("x"); }', encoding="utf-8")
            path = self.write_inventory(root, """[[surfaces]]
id='x'
owner_issue=1
semantic_authority='a'
path='crates/cargo-allow/tests/release_contract.rs'
subject='s'
producer='p'
consumer='c'
claimed_acceptance_row='r'
assertion_mechanism='m'
evidence_class='LexicalProjectionOnly'
disposition='d'
required_stronger_owner='o'
may_satisfy_release_gate=true
last_reconciled_commit='c'
claim_boundary='b'
""")
            self.assertTrue(any("cannot use LexicalProjectionOnly" in error for error in inventory.validate(root, path)))

    def test_duplicate_ids_and_paths_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subject = root / "crates/cargo-allow/tests/release_contract.rs"
            subject.parent.mkdir(parents=True)
            subject.write_text('fn t() { require_contains("x"); }', encoding="utf-8")
            row = """owner_issue=1
semantic_authority='a'
path='crates/cargo-allow/tests/release_contract.rs'
subject='s'
producer='p'
consumer='c'
claimed_acceptance_row='r'
assertion_mechanism='m'
evidence_class='LexicalProjectionOnly'
disposition='d'
required_stronger_owner='o'
may_satisfy_release_gate=false
last_reconciled_commit='c'
claim_boundary='b'
"""
            path = self.write_inventory(root, "[[surfaces]]\nid='x'\n" + row + "\n[[surfaces]]\nid='x'\n" + row)
            errors = inventory.validate(root, path)
            self.assertIn("duplicate inventory id: x", errors)
            self.assertIn("duplicate inventory path: crates/cargo-allow/tests/release_contract.rs", errors)


if __name__ == "__main__":
    unittest.main()