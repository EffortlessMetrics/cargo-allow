"""Cheap accept/reject controls; these do not establish a real floor run."""

import unittest
from pathlib import Path
import shlex

from floor_execution_identity import identity


HOST = "x86_64-pc-windows-msvc"


def verbose(version="1.95.0", host=HOST):
    return f"release: {version}\nhost: {host}\n"


class ExecutionIdentityTests(unittest.TestCase):
    def test_pregate_grades_floor_freshness_before_heavy_jobs(self):
        workflow = Path(__file__).resolve().parent.parent.joinpath(
            ".github/workflows/ci.yml"
        ).read_text(encoding="utf-8")
        pregate = workflow.split("  pregate:\n", 1)[1].split(
            "\n  workflow-syntax:", 1
        )[0]
        self.assertIn(
            "run: cargo run -p cargo-allow --locked -- min-version-drift check --format human",
            pregate,
        )
        self.assertIn("id: floor_freshness", pregate)
        self.assertIn(
            "FLOOR_FRESHNESS_OUTCOME: ${{ steps.floor_freshness.outcome || 'skipped' }}",
            pregate,
        )
        self.assertIn('"floor-freshness:$FLOOR_FRESHNESS_OUTCOME"', pregate)
        self.assertNotIn("continue-on-error:", pregate)
        self.assertNotIn("cache-hit", pregate)
        for job in ("msrv", "test-core-platforms", "coverage"):
            block = workflow.split(f"\n  {job}:\n", 1)[1]
            declaration = block.split("steps:", 1)[0]
            self.assertIn("pregate", declaration)

    def test_pregate_cache_matches_its_pinned_toolchain(self):
        workflow = Path(__file__).resolve().parent.parent.joinpath(
            ".github/workflows/ci.yml"
        ).read_text(encoding="utf-8")
        pregate = workflow.split("  pregate:\n", 1)[1].split(
            "\n  workflow-syntax:", 1
        )[0]
        toolchain = pregate.index("uses: dtolnay/rust-toolchain@1.95.0")
        cache = pregate.index("uses: ./.github/actions/rust-cache")
        proof = pregate.index("id: no_new")
        self.assertLess(toolchain, cache)
        self.assertLess(cache, proof)
        self.assertIn("lane: pregate\n          toolchain: 1.95.0", pregate)
        # rust-toolchain.toml pins the floating stable channel and ranks
        # above `rustup default` (the only thing the dtolnay action
        # changes), so without this override cargo would resolve the
        # runner's stable while the cache keys on 1.95.0. The pin must be
        # present and must equal the cache toolchain identity.
        pin = pregate.index('RUSTUP_TOOLCHAIN: "1.95.0"')
        self.assertLess(pin, toolchain)
        # The pin selects the exact 1.95.0 toolchain, so the components
        # the pregate needs must be installed with it: a bare 1.95.0
        # install ships no rustfmt and the Formatting step fails closed
        # (proven by hosted run 42326701401). Clippy stays out of the
        # pregate; it runs in the Stage 2 heavy lanes.
        self.assertIn("components: rustfmt", pregate)
        self.assertNotIn("clippy", pregate)

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
