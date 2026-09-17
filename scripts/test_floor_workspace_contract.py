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
        self.assertEqual(call.kwargs["timeout"], 3600)
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


class WorkspaceSourceAdmissionTests(unittest.TestCase):
    """Real Git controls; Cargo is intercepted and must not run on bad source."""

    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="floor-workspace-source-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.real_run = subprocess.run
        self.git("init", "-q")
        self.git("config", "user.name", "floor source fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "core.autocrlf", "false")
        (self.root / ".gitignore").write_text("target/\n.cargo/\ntarget-other/\n", encoding="utf-8", newline="\n")
        (self.root / "Cargo.toml").write_text('[workspace]\nmembers = []\n', encoding="utf-8", newline="\n")
        (self.root / "Cargo.lock").write_text("version = 4\n", encoding="utf-8", newline="\n")
        (self.root / "source.rs").write_text("// committed source\n", encoding="utf-8", newline="\n")
        self.git("add", ".")
        self.git("-c", "core.hooksPath=", "-c", "commit.gpgSign=false",
                 "commit", "-qm", "source admission fixture")
        self.source = self.git("rev-parse", "HEAD").strip()
        self.git("checkout", "--detach", "-q", self.source)
        self.output = self.root / "target/floor-proof/source-workspace-contract.json"

    def git(self, *arguments):
        return self.real_run(["git", "-C", str(self.root), *arguments],
                             capture_output=True, text=True, check=True, timeout=30).stdout

    def identity(self):
        with contextlib.chdir(self.root):
            return contract.source_identity(self.source)

    def assert_rejected_before_cargo(self, message):
        cargo_calls = []
        self.output.parent.mkdir(parents=True, exist_ok=True)
        self.output.write_text('{"result":"passed"}')

        def run(arguments, **kwargs):
            if arguments[0] == "cargo":
                cargo_calls.append(arguments)
                return SimpleNamespace(stdout=passing_output(), stderr="", returncode=0)
            self.assertEqual(kwargs.get("timeout"), 30)
            return self.real_run(arguments, **kwargs)

        with contextlib.chdir(self.root), patch.object(contract.subprocess, "run", side_effect=run), \
             self.assertRaisesRegex(ValueError, message):
            contract.run_preflight(self.source, "x86_64-pc-windows-msvc", self.output)
        self.assertEqual(cargo_calls, [])
        self.assertFalse(self.output.exists())

    def test_clean_detached_source_is_admitted(self):
        identity = self.identity()
        self.assertEqual(identity["source_commit"], self.source)
        self.assertEqual(identity["source_tree"], self.git("rev-parse", "HEAD^{tree}").strip())
        self.assertEqual(identity["manifest_digest"],
                         contract.digest((self.root / "Cargo.toml").read_bytes().replace(b"\r", b"")))
        self.assertEqual(identity["lock_digest"],
                         contract.digest((self.root / "Cargo.lock").read_bytes().replace(b"\r", b"")))

    def test_crlf_checkout_preserves_source_identity(self):
        before = self.identity()
        self.git("config", "core.autocrlf", "true")
        for name in ("Cargo.toml", "Cargo.lock"):
            path = self.root / name
            path.write_bytes(path.read_bytes().replace(b"\r", b"").replace(b"\n", b"\r\n"))
        self.git("add", "--", "Cargo.toml", "Cargo.lock")
        self.assertEqual(self.git("diff", "--cached", "--name-only"), "")
        self.assertEqual(self.git("status", "--porcelain=v1", "--untracked-files=all"), "")
        self.assertEqual(self.identity(), before)

    def test_attached_source_is_rejected_before_cargo(self):
        self.git("checkout", "-qb", "attached-fixture")
        self.assert_rejected_before_cargo("detached")

    def test_skip_worktree_manifest_is_rejected_before_cargo(self):
        self.git("update-index", "--skip-worktree", "Cargo.toml")
        (self.root / "Cargo.toml").write_text('[workspace]\nmembers = ["changed"]\n')
        self.assertEqual(self.git("status", "--porcelain=v1", "--untracked-files=all"), "")
        self.assert_rejected_before_cargo("hidden index")

    def test_assume_unchanged_source_is_rejected_before_cargo(self):
        self.git("update-index", "--assume-unchanged", "source.rs")
        (self.root / "source.rs").write_text("// different source\n")
        self.assertEqual(self.git("status", "--porcelain=v1", "--untracked-files=all"), "")
        self.assert_rejected_before_cargo("hidden index")

    def test_ignored_cargo_config_is_rejected_before_cargo(self):
        (self.root / ".cargo").mkdir()
        (self.root / ".cargo/config.toml").write_text('[build]\nrustflags = ["--cfg", "changed"]\n')
        self.assertEqual(self.git("status", "--porcelain=v1", "--untracked-files=all"), "")
        self.assert_rejected_before_cargo("ignored files outside root target")

    def test_ignored_root_target_scratch_remains_allowed(self):
        before = self.identity()
        (self.root / "target/floor-proof").mkdir(parents=True)
        (self.root / "target/floor-proof/scratch.txt").write_text("diagnostics")
        self.assertEqual(self.identity(), before)

    def test_ignored_sibling_of_target_is_not_scratch(self):
        (self.root / "target-other").mkdir()
        (self.root / "target-other/config.toml").write_text("unadmitted")
        self.assert_rejected_before_cargo("ignored files outside root target")

    def test_staged_source_is_rejected_before_cargo(self):
        (self.root / "source.rs").write_text("// staged source\n")
        self.git("add", "source.rs")
        self.assert_rejected_before_cargo("clean source")

    def test_unstaged_source_is_rejected_before_cargo(self):
        (self.root / "source.rs").write_text("// unstaged source\n")
        self.assert_rejected_before_cargo("clean source")

    def test_untracked_source_is_rejected_before_cargo(self):
        (self.root / "untracked.rs").write_text("// untracked source\n")
        self.assert_rejected_before_cargo("clean source")

    def test_passing_cargo_cannot_hide_a_source_edit(self):
        cargo_calls = []

        def run(arguments, **kwargs):
            if arguments[0] == "cargo":
                cargo_calls.append(arguments)
                (self.root / "source.rs").write_text("// changed during execution\n")
                return SimpleNamespace(stdout=passing_output(), stderr="", returncode=0)
            return self.real_run(arguments, **kwargs)

        with contextlib.chdir(self.root), patch.object(contract.subprocess, "run", side_effect=run), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()), \
             self.assertRaisesRegex(ValueError, "clean source"):
            contract.run_preflight(self.source, "x86_64-pc-windows-msvc", self.output)
        self.assertEqual(len(cargo_calls), 1)
        self.assertFalse(self.output.exists())


class WorkspaceTimeoutTests(unittest.TestCase):
    def test_git_timeout_is_bounded_and_non_clean(self):
        error = subprocess.TimeoutExpired(["git", "rev-parse", "HEAD"], 30)
        with patch.object(contract.subprocess, "run", side_effect=error) as run:
            with self.assertRaises(subprocess.TimeoutExpired):
                contract.source_identity("1" * 40)
        self.assertEqual(run.call_args.kwargs["timeout"], 30)

    def test_cargo_timeout_clears_stale_admission_and_fails_cli(self):
        with tempfile.TemporaryDirectory(prefix="floor-workspace-timeout-") as directory:
            output = Path(directory) / "contract.json"
            output.write_text('{"result":"passed"}')
            error = subprocess.TimeoutExpired(["cargo", "test"], 3600)
            with patch.object(contract, "source_identity", return_value={}), \
                 patch.object(contract.subprocess, "run", side_effect=error) as run, \
                 contextlib.redirect_stderr(io.StringIO()) as diagnostic:
                result = contract.main(["floor_workspace_contract.py", "1" * 40,
                                        "x86_64-pc-windows-msvc", str(output)])
            self.assertEqual(result, 1)
            self.assertEqual(run.call_args.kwargs["timeout"], 3600)
            self.assertIn("timed out", diagnostic.getvalue())
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
