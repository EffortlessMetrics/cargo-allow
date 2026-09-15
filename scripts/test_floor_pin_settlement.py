#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys
import unittest

MODULE_PATH = Path(__file__).with_name("floor_pin_settlement.py")
spec = importlib.util.spec_from_file_location("floor_pin_settlement", MODULE_PATH)
assert spec and spec.loader
settlement = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = settlement
spec.loader.exec_module(settlement)


class FakeResolver:
    def __init__(self, attempt):
        self.state: dict[str, str] = {}
        self.attempt_impl = attempt
        self.calls: list[str] = []

    def attempt(self, row):
        self.calls.append(row["package"])
        return self.attempt_impl(self.state, row)

    def snapshot(self):
        identity = ";".join(
            f"{name}={version}" for name, version in sorted(self.state.items())
        ) or "missing"
        return settlement.LockSnapshot(dict(self.state), identity)


class SettlementTests(unittest.TestCase):
    def test_early_failure_is_retried_after_sibling_floor_settles(self):
        def attempt(state, row):
            if row["package"] == "alpha":
                state.setdefault("alpha", "2.0.0")
                state.setdefault("beta", "2.0.0")
                if state["beta"] != "1.0.0":
                    return 1, "beta 2 requires alpha 2"
                state["alpha"] = "1.0.0"
                return 0, ""
            state.setdefault("alpha", "2.0.0")
            state["beta"] = "1.0.0"
            return 0, ""

        floors = [
            {"package": "alpha", "floor": "1.0.0"},
            {"package": "beta", "floor": "1.0.0"},
        ]
        resolver = FakeResolver(attempt)

        failures = settlement.settle_floors(
            floors,
            resolver.attempt,
            resolver.snapshot,
        )

        self.assertEqual({}, failures)
        self.assertEqual(
            {"alpha": "1.0.0", "beta": "1.0.0"},
            resolver.state,
        )
        self.assertEqual(["alpha", "beta", "alpha"], resolver.calls)

    def test_input_order_does_not_change_final_identity(self):
        def run(floors):
            def attempt(state, row):
                state[row["package"]] = row["floor"]
                return 0, ""

            resolver = FakeResolver(attempt)
            failures = settlement.settle_floors(
                floors,
                resolver.attempt,
                resolver.snapshot,
            )
            return failures, resolver.snapshot().identity, resolver.calls

        floors = [
            {"package": "zeta", "floor": "1.0.0"},
            {"package": "alpha", "floor": "2.0.0"},
        ]
        forward = run(floors)
        reverse = run(list(reversed(floors)))

        self.assertEqual(forward, reverse)
        self.assertEqual(["alpha", "zeta"], forward[2])

    def test_genuinely_incompatible_set_terminates_with_bounded_failure(self):
        def attempt(state, row):
            state.setdefault("alpha", "2.0.0")
            state.setdefault("beta", "2.0.0")
            return 1, f"cannot pin {row['package']}"

        floors = [
            {"package": "alpha", "floor": "1.0.0"},
            {"package": "beta", "floor": "1.0.0"},
        ]
        resolver = FakeResolver(attempt)

        failures = settlement.settle_floors(
            floors,
            resolver.attempt,
            resolver.snapshot,
        )

        self.assertEqual(
            {"alpha": "cannot pin alpha", "beta": "cannot pin beta"},
            failures,
        )
        self.assertEqual(4, len(resolver.calls))

    def test_empty_failure_stderr_produces_a_bounded_diagnostic(self):
        def attempt(state, row):
            state[row["package"]] = "2.0.0"
            return 17, " \n"

        floors = [{"package": "alpha", "floor": "1.0.0"}]
        resolver = FakeResolver(attempt)

        failures = settlement.settle_floors(
            floors,
            resolver.attempt,
            resolver.snapshot,
        )

        self.assertEqual(
            {
                "alpha": (
                    "cargo update failed with exit code 17 while pinning "
                    "alpha to 1.0.0; stderr was empty"
                )
            },
            failures,
        )
        self.assertEqual(["alpha", "alpha"], resolver.calls)

    def test_later_pin_that_moves_an_earlier_floor_is_not_certified(self):
        def attempt(state, row):
            if row["package"] == "alpha":
                state["alpha"] = "1.0.0"
                state["beta"] = "2.0.0"
            else:
                state["beta"] = "1.0.0"
                state["alpha"] = "2.0.0"
            return 0, ""

        floors = [
            {"package": "alpha", "floor": "1.0.0"},
            {"package": "beta", "floor": "1.0.0"},
        ]
        resolver = FakeResolver(attempt)

        failures = settlement.settle_floors(
            floors,
            resolver.attempt,
            resolver.snapshot,
        )

        self.assertEqual(
            {
                "alpha": (
                    "pin did not remain at declared floor: "
                    "locked at 2.0.0, floor requires 1.0.0"
                )
            },
            failures,
        )
        self.assertEqual(
            ["alpha", "beta", "alpha", "beta"],
            resolver.calls,
        )


if __name__ == "__main__":
    unittest.main()
