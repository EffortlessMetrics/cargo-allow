"""Negative controls for the original-workspace floor-producer preflight."""

import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
import floor_workspace_contract as contract


def passing_output():
    return ("running 5 tests\n"
            + "".join(f"test {name} ... ok\n" for name in contract.TESTS)
            + "test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; "
              "1556 filtered out; finished in 0.01s\n")


class WorkspaceOutputTests(unittest.TestCase):
    def test_exact_pass_and_crlf_pass(self):
        output = passing_output()
        contract.admit_output(output, 0)
        contract.admit_output(output.replace("\n", "\r\n"), 0)

    def test_rejects_nonzero_even_with_passing_text(self):
        with self.assertRaisesRegex(ValueError, "exit 101"):
            contract.admit_output(passing_output(), 101)

    def test_rejects_empty_wrong_duplicate_ignored_and_extra_observations(self):
        output = passing_output()
        mutations = {
            "empty": "",
            "no tests": "running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; "
                        "0 measured; 1561 filtered out; finished in 0.01s\n",
            "missing": output.replace(f"test {contract.TESTS[0]} ... ok\n", ""),
            "wrong": output.replace(contract.TESTS[0], "other::test"),
            "duplicate": output.replace(contract.TESTS[0], contract.TESTS[1]),
            "ignored": output.replace(f"test {contract.TESTS[0]} ... ok",
                                      f"test {contract.TESTS[0]} ... ignored"),
            "extra match": output + f"test {contract.TESTS[0]}_extra ... ok\n",
            "no summary": "\n".join(output.splitlines()[:-1]) + "\n",
            "ignored summary": output.replace("0 ignored", "1 ignored"),
            "duplicate suite": output + output,
            "failed summary": output.replace("test result: ok.", "test result: FAILED."),
        }
        for name, changed in mutations.items():
            with self.subTest(name=name), self.assertRaises(ValueError):
                contract.admit_output(changed, 0)


class WorkspaceExecutionTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="floor-workspace-contract-")
        self.addCleanup(directory.cleanup)
        self.output = Path(directory.name) / "workspace-contract.json"
        self.identity = {"source_commit": "1" * 40, "source_tree": "2" * 40,
                         "manifest_digest": "sha256:v1:" + "3" * 64,
                         "lock_digest": "sha256:v1:" + "4" * 64}

    def invoke(self, stdout=None, status=0, identities=None):
        result = SimpleNamespace(stdout=passing_output() if stdout is None else stdout,
                                 stderr="", returncode=status)
        identities = identities or [self.identity, self.identity]
        with patch.object(contract, "source_identity", side_effect=identities), \
             patch.object(contract.subprocess, "run", return_value=result) as run, \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            observation = contract.run_preflight("1" * 40, "x86_64-pc-windows-msvc", self.output)
        return observation, run.call_args

    def test_pass_is_original_locked_subject_with_isolated_outputs(self):
        observation, call = self.invoke()
        arguments = call.args[0]
        self.assertEqual(arguments[:2], ["cargo", "test"])
        self.assertIn("--locked", arguments)
        self.assertEqual(arguments[arguments.index("--target-dir") + 1], contract.TARGET_DIR)
        self.assertEqual(arguments[arguments.index("--bin") + 1], "cargo-allow")
        self.assertEqual(arguments[-len(contract.TESTS):], list(contract.TESTS))
        self.assertNotIn("--exact", arguments)  # same matching law as projected --skip
        self.assertEqual(call.kwargs["env"]["RUST_TEST_NOCAPTURE"], "0")
        self.assertEqual(observation["source_commit"], "1" * 40)
        self.assertEqual(observation["result"], "passed")
        self.assertIn("not direct-floor", observation["claim_boundary"])
        self.assertEqual(observation["stdout_digest"], contract.digest(passing_output().encode()))
        self.assertEqual(json.loads(self.output.read_bytes()), observation)

    def test_failed_rerun_removes_prior_admission_and_retains_diagnostic(self):
        self.output.write_text('{"result":"passed"}')
        with self.assertRaisesRegex(ValueError, "exit 101"):
            self.invoke(status=101)
        self.assertFalse(self.output.exists())
        self.assertEqual(self.output.with_suffix(".stdout.log").read_text(), passing_output())

    def test_missing_test_never_writes_admission(self):
        with self.assertRaises(ValueError):
            self.invoke(stdout="")
        self.assertFalse(self.output.exists())

    def test_source_movement_never_writes_admission(self):
        for field in self.identity:
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "changed its source"):
                self.invoke(identities=[self.identity, {**self.identity, field: "changed"}])
            self.assertFalse(self.output.exists())

    def test_wrong_or_dirty_source_never_runs_cargo(self):
        with patch.object(contract, "source_identity", side_effect=ValueError("bad source")), \
             patch.object(contract.subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "bad source"):
                contract.run_preflight("1" * 40, "host", self.output)
        run.assert_not_called()
        self.assertFalse(self.output.exists())

    def test_git_observation_rejects_wrong_commit_dirty_tree_and_malformed_tree(self):
        for outputs in (
            ["2" * 40, "3" * 40],
            ["1" * 40, "3" * 40, " M Cargo.toml\0"],
            ["1" * 40, "not-a-tree"],
        ):
            with self.subTest(outputs=outputs), \
                 patch.object(contract.subprocess, "run", side_effect=[
                     SimpleNamespace(stdout=value, returncode=0) for value in outputs
                 ]), self.assertRaises(ValueError):
                contract.source_identity("1" * 40)


if __name__ == "__main__":
    unittest.main()
