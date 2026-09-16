#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

MODULE_PATH = Path(__file__).with_name("floor_pin_settlement.py")
spec = importlib.util.spec_from_file_location("floor_pin_settlement", MODULE_PATH)
assert spec and spec.loader
settlement = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = settlement
spec.loader.exec_module(settlement)

REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"


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
        resolved = {
            name: (settlement.LockedPackage(version, REGISTRY),)
            for name, version in self.state.items()
        }
        return settlement.LockSnapshot(resolved, identity)


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

    def test_duplicate_package_identities_are_not_treated_as_exact(self):
        calls = []

        def attempt(row):
            calls.append(row["package"])
            return 0, ""

        def snapshot():
            return settlement.LockSnapshot(
                {
                    "alpha": (
                        settlement.LockedPackage("1.0.0", REGISTRY),
                        settlement.LockedPackage("2.0.0", REGISTRY),
                    )
                },
                "two-alpha-identities",
            )

        failures = settlement.settle_floors(
            [{"package": "alpha", "floor": "1.0.0"}],
            attempt,
            snapshot,
        )

        self.assertIn("multiple locked identities", failures["alpha"])
        self.assertIn("1.0.0", failures["alpha"])
        self.assertIn("2.0.0", failures["alpha"])
        self.assertEqual(["alpha", "alpha"], calls)

    def test_lock_snapshot_retains_version_and_source_for_duplicate_rows(self):
        with tempfile.TemporaryDirectory(prefix="floor-lock-snapshot-") as directory:
            lock_path = Path(directory) / "Cargo.lock"
            lock_path.write_text(
                'version = 4\n'
                '[[package]]\nname = "alpha"\nversion = "2.0.0"\n'
                'source = "git+https://example.invalid/alpha"\n'
                '[[package]]\nname = "alpha"\nversion = "1.0.0"\n'
                f'source = "{REGISTRY}"\n',
                encoding="utf-8",
                newline="\n",
            )
            snapshot = settlement.read_lock_snapshot(lock_path)

        self.assertEqual(
            (
                settlement.LockedPackage("1.0.0", REGISTRY),
                settlement.LockedPackage(
                    "2.0.0", "git+https://example.invalid/alpha"
                ),
            ),
            snapshot.resolved["alpha"],
        )
        self.assertNotEqual("missing", snapshot.identity)

    def test_same_version_mixed_sources_sort_without_type_coercion(self):
        with tempfile.TemporaryDirectory(prefix="floor-lock-source-sort-") as directory:
            lock_path = Path(directory) / "Cargo.lock"
            lock_path.write_text(
                'version = 4\n'
                '[[package]]\nname = "alpha"\nversion = "1.0.0"\n'
                '[[package]]\nname = "alpha"\nversion = "1.0.0"\n'
                f'source = "{REGISTRY}"\n',
                encoding="utf-8",
                newline="\n",
            )
            snapshot = settlement.read_lock_snapshot(lock_path)

        self.assertEqual(
            (
                settlement.LockedPackage("1.0.0", None),
                settlement.LockedPackage("1.0.0", REGISTRY),
            ),
            snapshot.resolved["alpha"],
        )

    def test_source_identity_uses_only_canonical_crates_io_urls(self):
        for source in settlement.CRATES_IO_SOURCES:
            with self.subTest(source=source):
                package = settlement.LockedPackage("1.0.0", source)
                self.assertEqual(
                    "registry:crates.io", settlement.source_identity(package)
                )

        custom = "registry+https://mirror.example/index.crates.io"
        package = settlement.LockedPackage("1.0.0", custom)
        self.assertEqual(custom, settlement.source_identity(package))

    def test_update_spec_is_qualified_and_ambiguous_sources_fail_closed(self):
        row = {"package": "alpha", "floor": "1.0.0"}
        empty = settlement.LockSnapshot({}, "missing")
        self.assertEqual(("alpha", None), settlement.cargo_update_spec(empty, row))

        unique = settlement.LockSnapshot(
            {"alpha": (settlement.LockedPackage("2.0.0", REGISTRY),)},
            "unique",
        )
        self.assertEqual(
            ("alpha@2.0.0", None),
            settlement.cargo_update_spec(unique, row),
        )

        non_registry = settlement.LockSnapshot(
            {
                "alpha": (
                    settlement.LockedPackage(
                        "2.0.0", "git+https://example.invalid/alpha"
                    ),
                )
            },
            "git",
        )
        package_spec, issue = settlement.cargo_update_spec(non_registry, row)
        self.assertIsNone(package_spec)
        self.assertIn("non-registry", issue)

        duplicate = settlement.LockSnapshot(
            {
                "alpha": (
                    settlement.LockedPackage("1.0.0", REGISTRY),
                    settlement.LockedPackage(
                        "1.0.0", "git+https://example.invalid/alpha"
                    ),
                )
            },
            "duplicate",
        )
        package_spec, issue = settlement.cargo_update_spec(duplicate, row)
        self.assertIsNone(package_spec)
        self.assertIn("multiple locked identities", issue)

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


    def test_producer_identity_import_disables_python_bytecode(self):
        producer = MODULE_PATH.with_name("proof-direct-floors.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            'floor_move_failures="$(python3 -B - <<\'PY\'',
            producer,
        )

        with tempfile.TemporaryDirectory(prefix="floor-bytecode-") as directory:
            root = Path(directory)
            scripts = root / "scripts"
            scripts.mkdir()
            shutil.copyfile(MODULE_PATH, scripts / MODULE_PATH.name)
            environment = os.environ.copy()
            environment.pop("PYTHONDONTWRITEBYTECODE", None)
            environment.pop("PYTHONPYCACHEPREFIX", None)
            import_code = (
                "from pathlib import Path\n"
                "import sys\n"
                'sys.path.insert(0, str(Path("scripts").resolve()))\n'
                "import floor_pin_settlement\n"
            )

            subprocess.run(
                [sys.executable, "-B", "-c", import_code],
                cwd=root,
                env=environment,
                check=True,
                capture_output=True,
                text=True,
            )

            self.assertFalse((scripts / "__pycache__").exists())
            self.assertEqual([], list(root.rglob("*.pyc")))

if __name__ == "__main__":
    unittest.main()
