"""Cheap accept/reject controls; these do not establish a real floor run."""

import unittest
from pathlib import Path
import shlex

from floor_execution_identity import identity


HOST = "x86_64-pc-windows-msvc"


def verbose(version="1.95.0", host=HOST):
    return f"release: {version}\nhost: {host}\n"


class ExecutionIdentityTests(unittest.TestCase):
    def test_bootstrap_excludes_only_retained_output_graders(self):
        source = Path(__file__).with_name("proof-direct-floors.sh").read_text(encoding="utf-8")
        array = source.split("DRIFT_TEST_SKIPS=(", 1)[1].split(")", 1)[0]
        arguments = shlex.split(array)
        self.assertEqual(arguments[::2], ["--skip"] * 5)
        self.assertEqual(arguments[1::2], [
            "minimum_direct_version_drift_retained_receipts_are_current_with_the_live_tree",
            "check_exits_zero_when_every_release_set_receipt_is_current",
            "minimum_direct_version_fixtures_retained_proof_receipt_is_law_clean",
            "minimum_direct_version_products_retained_advisory_receipts_are_clean",
            "minimum_direct_version_products_cargo_allow_receipt_certifies_its_closure",
        ])
        self.assertIn('-- "${DRIFT_TEST_SKIPS[@]}"', source)
        self.assertIn('-- ${DRIFT_TEST_SKIPS[*]}', source)

    def test_observed_patch_and_host_are_preserved(self):
        result = identity("1.95", verbose("1.95.2"), verbose("1.95.1"))
        self.assertEqual(result["toolchain"], "1.95.2")
        self.assertEqual(result["cargo"], "1.95.1")
        self.assertEqual(result["target"], "host:" + HOST)

    def test_wrong_unavailable_or_unstable_identity_is_rejected(self):
        for output in (
            "", verbose("1.950.0"), verbose("1.96.0"),
            verbose("1.95.0-nightly"), verbose(host="host"),
            verbose() + "release: 1.95.0\n",
        ):
            with self.subTest(output=output), self.assertRaises(ValueError):
                identity("1.95", output, verbose())
            with self.subTest(cargo=output), self.assertRaises(ValueError):
                identity("1.95", verbose(), output)

    def test_host_mismatch_and_exact_patch_mismatch_are_rejected(self):
        with self.assertRaises(ValueError):
            identity("1.95", verbose(), verbose(host="x86_64-unknown-linux-gnu"))
        with self.assertRaises(ValueError):
            identity("1.95.1", verbose(), verbose())


if __name__ == "__main__":
    unittest.main()
