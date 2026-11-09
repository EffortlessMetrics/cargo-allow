#!/usr/bin/env python3
"""Fail-closed controls for the bounded release-identity projection reader (#3787).

The fixtures mirror the exact JSON the typed authority emits for
`cargo-allow release-identity --version <V>` (stable 0.2.0 and the RC.1,
RC.2, RC.10 candidate line). They differ only in the fields the authority
produced; no fixture invents grammar, ordering, or channel semantics. The
controls pin envelope validation, exact field extraction, expected-value
binding, and fail-closed behavior for corrupt, incomplete, foreign, and
mistyped projections.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

READER = Path(__file__).with_name("release_identity_projection.py")

# Captured from `cargo run -p cargo-allow -- release-identity --version V`.
STABLE_PROJECTION = {
    "schema": "cargo-allow.release-identity.v1",
    "result": "validated",
    "version": "0.2.0",
    "tag": "v0.2.0",
    "tag_source": "derived",
    "channel": "stable",
    "rc_ordinal": None,
    "github_prerelease": False,
    "precedence": {
        "major": 0,
        "minor": 2,
        "patch": 0,
        "is_stable": True,
        "rc_ordinal": None,
    },
}
RC1_PROJECTION = {
    "schema": "cargo-allow.release-identity.v1",
    "result": "validated",
    "version": "0.2.0-rc.1",
    "tag": "v0.2.0-rc.1",
    "tag_source": "derived",
    "channel": "release_candidate",
    "rc_ordinal": 1,
    "github_prerelease": True,
    "precedence": {
        "major": 0,
        "minor": 2,
        "patch": 0,
        "is_stable": False,
        "rc_ordinal": 1,
    },
}
RC2_PROJECTION = {
    "schema": "cargo-allow.release-identity.v1",
    "result": "validated",
    "version": "0.2.0-rc.2",
    "tag": "v0.2.0-rc.2",
    "tag_source": "derived",
    "channel": "release_candidate",
    "rc_ordinal": 2,
    "github_prerelease": True,
    "precedence": {
        "major": 0,
        "minor": 2,
        "patch": 0,
        "is_stable": False,
        "rc_ordinal": 2,
    },
}
RC10_PROJECTION = {
    "schema": "cargo-allow.release-identity.v1",
    "result": "validated",
    "version": "0.2.0-rc.10",
    "tag": "v0.2.0-rc.10",
    "tag_source": "derived",
    "channel": "release_candidate",
    "rc_ordinal": 10,
    "github_prerelease": True,
    "precedence": {
        "major": 0,
        "minor": 2,
        "patch": 0,
        "is_stable": False,
        "rc_ordinal": 10,
    },
}


def run_reader(*arguments: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(READER), *arguments],
        capture_output=True,
        text=True,
    )


class ReaderTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = os.path.join(self.directory.name, "projection.json")

    def write(self, payload) -> None:
        if isinstance(payload, str):
            Path(self.path).write_text(payload, encoding="utf-8")
        else:
            Path(self.path).write_text(
                json.dumps(payload, indent=2) + "\n", encoding="utf-8"
            )

    def test_stable_projection_extracts_exact_fields(self) -> None:
        self.write(STABLE_PROJECTION)
        result = run_reader(
            self.path,
            "--require",
            "version,tag,github_prerelease",
            "--expect",
            "version=0.2.0",
            "--expect",
            "tag=v0.2.0",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout.splitlines(),
            ["version=0.2.0", "tag=v0.2.0", "github_prerelease=false"],
        )

    def test_rc_line_fixtures_carry_distinct_semantic_ordinals(self) -> None:
        for projection, ordinal in (
            (RC1_PROJECTION, "1"),
            (RC2_PROJECTION, "2"),
            (RC10_PROJECTION, "10"),
        ):
            self.write(projection)
            result = run_reader(
                self.path,
                "--require",
                "version,rc_ordinal,github_prerelease,precedence",
                "--expect",
                f"version={projection['version']}",
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                result.stdout.splitlines(),
                [
                    f"version={projection['version']}",
                    f"rc_ordinal={ordinal}",
                    "github_prerelease=true",
                    "precedence.major=0",
                    "precedence.minor=2",
                    "precedence.patch=0",
                    "precedence.is_stable=false",
                    f"precedence.rc_ordinal={ordinal}",
                ],
            )

    def test_rc_ordering_follows_typed_precedence_not_lexical_filenames(self) -> None:
        # The sort key every consumer derives from this projection is
        # (major, minor, patch, stable-before-prerelease, rc_ordinal).
        # Lexically "0.2.0-rc.10" sorts before "0.2.0-rc.2"; the typed
        # precedence must order rc.10 after rc.2.
        self.assertLess("0.2.0-rc.10", "0.2.0-rc.2")  # lexical trap documented

        def sort_key(projection):
            precedence = projection["precedence"]
            return (
                precedence["major"],
                precedence["minor"],
                precedence["patch"],
                1 if precedence["is_stable"] else 0,
                precedence["rc_ordinal"] or 0,
            )

        ordered = sorted(
            [RC2_PROJECTION, RC10_PROJECTION],
            key=sort_key,
            reverse=True,
        )
        self.assertEqual(
            [projection["version"] for projection in ordered],
            ["0.2.0-rc.10", "0.2.0-rc.2"],
        )
        self.assertNotEqual(
            sorted(["0.2.0-rc.10", "0.2.0-rc.2"], reverse=True),
            ["0.2.0-rc.10", "0.2.0-rc.2"],
        )

    def test_missing_required_field_fails_closed(self) -> None:
        corrupt = dict(RC1_PROJECTION)
        del corrupt["github_prerelease"]
        self.write(corrupt)
        result = run_reader(self.path, "--require", "github_prerelease")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("github_prerelease", result.stderr)

    def test_mistyped_prerelease_posture_fails_closed(self) -> None:
        corrupt = dict(STABLE_PROJECTION)
        corrupt["github_prerelease"] = "false"
        self.write(corrupt)
        result = run_reader(self.path, "--require", "github_prerelease")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not a Boolean", result.stderr)

    def test_non_validated_result_fails_closed(self) -> None:
        corrupt = dict(STABLE_PROJECTION)
        corrupt["result"] = "rejected"
        self.write(corrupt)
        result = run_reader(self.path, "--require", "version")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not a validated result", result.stderr)

    def test_foreign_schema_generation_fails_without_fallback(self) -> None:
        corrupt = dict(STABLE_PROJECTION)
        corrupt["schema"] = "cargo-allow.release-identity.v0"
        self.write(corrupt)
        result = run_reader(self.path, "--require", "version")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unsupported schema", result.stderr)

    def test_malformed_json_fails_closed(self) -> None:
        self.write('{"schema": "cargo-allow.release-identity.v1", "result":')
        result = run_reader(self.path, "--require", "version")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not valid JSON", result.stderr)

    def test_missing_projection_file_fails_closed(self) -> None:
        result = run_reader(
            os.path.join(self.directory.name, "absent.json"),
            "--require",
            "version",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("cannot read projection", result.stderr)

    def test_expected_binding_catches_raw_input_divergence(self) -> None:
        # Typed output is internally correct, but the caller's raw input
        # differs from the projected canonical field: fail, never default.
        self.write(RC2_PROJECTION)
        result = run_reader(
            self.path,
            "--require",
            "version",
            "--expect",
            "version=0.2.0-rc.3",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("caller expected", result.stderr)

    def test_unknown_required_field_fails_closed(self) -> None:
        self.write(STABLE_PROJECTION)
        result = run_reader(self.path, "--require", "semver")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unknown projected field", result.stderr)

    def test_precedence_expectation_is_rejected(self) -> None:
        self.write(STABLE_PROJECTION)
        result = run_reader(
            self.path, "--require", "version", "--expect", "precedence=0"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not expectation-bound", result.stderr)

    def test_malformed_expect_binding_fails_closed(self) -> None:
        self.write(STABLE_PROJECTION)
        result = run_reader(self.path, "--require", "version", "--expect", "version")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("malformed --expect binding", result.stderr)

    def test_missing_require_option_fails_closed(self) -> None:
        self.write(STABLE_PROJECTION)
        result = run_reader(self.path)
        self.assertEqual(result.returncode, 2)
        self.assertIn("--require", result.stderr)


if __name__ == "__main__":
    unittest.main(verbosity=2)
