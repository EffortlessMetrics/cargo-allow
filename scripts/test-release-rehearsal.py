#!/usr/bin/env python3
"""Fail-closed tests for the release-rehearsal characterization."""

from __future__ import annotations

import contextlib
from collections.abc import Mapping
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
REHEARSAL_PATH = ROOT / "scripts/release-rehearsal.py"
SPEC = importlib.util.spec_from_file_location("release_rehearsal", REHEARSAL_PATH)
if SPEC is None or SPEC.loader is None:
    raise SystemExit("could not load release rehearsal harness")
REHEARSAL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REHEARSAL)

REQUIRED_PHASES = (
    "release_identity",
    "candidate_package_set",
    "shared_prerequisites",
    "publisher_state_machine",
    "docs_and_support_identity",
    "manifest_and_assets",
    "authorization_boundary",
    "workflow_graph_permissions",
    "escaped_defect_corpus",
)


class TokenValueTrap(Mapping[str, str]):
    """Expose environment keys while failing any selected-token value read."""

    def __init__(self, values: dict[str, str]) -> None:
        self.values_by_name = values
        self.value_reads: list[str] = []

    def __iter__(self):
        return iter(self.values_by_name)

    def __len__(self) -> int:
        return len(self.values_by_name)

    def __getitem__(self, name: str) -> str:
        self.value_reads.append(name)
        if name == "CARGO_REGISTRY_TOKEN":
            raise AssertionError("selected-token value was read")
        return self.values_by_name[name]


class TestReceiptOutput(unittest.TestCase):
    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.sandbox = Path(directory.name).resolve()
        self.root = self.sandbox / "repo"
        self.root.mkdir()
        patcher = mock.patch.object(REHEARSAL, "ROOT", self.root)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.real_run = subprocess.run
        selector = mock.patch.object(REHEARSAL, "_select_tool", return_value=sys.executable)
        selector.start()
        self.addCleanup(selector.stop)
        self.tracked = mock.patch.object(
            REHEARSAL.subprocess, "run",
            return_value=subprocess.CompletedProcess([], 0, b"Cargo.toml\0target/tracked.json\0", b""),
        )
        self.tracked.start()
        self.addCleanup(self.tracked.stop)

    def invoke(self, path: Path | None) -> tuple[int, str, str, mock.Mock]:
        argv = ["release-rehearsal"]
        if path is not None:
            argv.extend(["--output", str(path)])
        stdout, stderr = io.StringIO(), io.StringIO()
        with mock.patch.object(sys, "argv", argv), mock.patch.object(
            REHEARSAL, "build_rehearsal_receipt",
            return_value={"aggregate_status": "Incomplete"},
        ) as phases, contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            result = REHEARSAL.main()
        return result, stdout.getvalue(), stderr.getvalue(), phases

    def test_selected_token_rejects_entry_points_before_any_work(self) -> None:
        existing = self.sandbox / "existing.json"
        existing.write_text("prior receipt", encoding="utf-8")
        new_output = self.root / "target/not-created/receipt.json"
        phase_functions = [
            "run_phase_" + ("docs_and_support" if phase == "docs_and_support_identity" else phase)
            for phase in REQUIRED_PHASES
        ]
        for token in ("", "synthetic-private-token"):
            for entry in ("builder", "cli", "cli-new-output", "cli-existing-output"):
                with self.subTest(token_empty=not token, entry=entry):
                    environment = TokenValueTrap({"CARGO_REGISTRY_TOKEN": token})
                    stdout, stderr = io.StringIO(), io.StringIO()
                    argv = [str(REHEARSAL_PATH)]
                    if entry.startswith("cli-"):
                        argv += ["--output", str(new_output if entry == "cli-new-output" else existing)]
                    with contextlib.ExitStack() as stack:
                        stack.enter_context(mock.patch.object(REHEARSAL.os, "environ", environment))
                        stack.enter_context(mock.patch.object(sys, "argv", argv))
                        stack.enter_context(contextlib.redirect_stdout(stdout))
                        stack.enter_context(contextlib.redirect_stderr(stderr))
                        work = [stack.enter_context(mock.patch.object(
                            REHEARSAL, name,
                            side_effect=AssertionError(f"rehearsal work reached: {name}"),
                        )) for name in (
                            "resolve_commit", "require_clean_checkout", "compute_sha256",
                            "_preflight_receipt_output", "_write_receipt", *phase_functions,
                        )]
                        work.append(stack.enter_context(mock.patch.object(REHEARSAL.subprocess, "run")))
                        work.append(stack.enter_context(mock.patch.object(Path, "read_text")))
                        if entry == "builder":
                            with self.assertRaisesRegex(ValueError, "CARGO_REGISTRY_TOKEN"):
                                REHEARSAL.build_rehearsal_receipt("HEAD")
                        else:
                            self.assertEqual(REHEARSAL.main(), 2)
                            self.assertIn("CARGO_REGISTRY_TOKEN", stderr.getvalue())
                        for operation in work:
                            operation.assert_not_called()
                    self.assertNotIn("CARGO_REGISTRY_TOKEN", environment.value_reads)
                    self.assertEqual(stdout.getvalue(), "")
                    self.assertNotIn("synthetic-private-token", stderr.getvalue())
                    self.assertFalse(new_output.parent.exists())
                    self.assertEqual(existing.read_text(encoding="utf-8"), "prior receipt")

    def test_child_environment_filters_selected_token_before_value_lookup(self) -> None:
        environment = TokenValueTrap({
            "CARGO_REGISTRY_TOKEN": "synthetic-private-token", "BENIGN": "retained",
        })
        with mock.patch.object(REHEARSAL.os, "environ", environment):
            self.assertEqual(REHEARSAL._sanitized_environment(), {"BENIGN": "retained"})
        self.assertEqual(environment.value_reads, ["BENIGN"])

    def test_output_git_children_receive_filtered_environment(self) -> None:
        environment = TokenValueTrap({
            "CARGO_REGISTRY_TOKEN": "synthetic-private-token", "BENIGN": "retained",
        })
        with mock.patch.object(REHEARSAL.os, "environ", environment):
            REHEARSAL._preflight_receipt_output(self.root / "target/new.json")
        calls = REHEARSAL.subprocess.run.call_args_list
        self.assertEqual(len(calls), 2)
        for call in calls:
            self.assertEqual(call.kwargs.get("env"), {"BENIGN": "retained"})
        self.assertNotIn("CARGO_REGISTRY_TOKEN", environment.value_reads)

    def test_late_authorization_token_detection_reads_no_value_or_artifact(self) -> None:
        values = {"BENIGN": "retained"}
        environment = TokenValueTrap(values)
        with mock.patch.object(REHEARSAL.os, "environ", environment):
            self.assertEqual(REHEARSAL._sanitized_environment(), values)
            values["CARGO_REGISTRY_TOKEN"] = "synthetic-private-token"
            receipt = {}
            with mock.patch.object(Path, "read_text") as read:
                self.assertEqual(REHEARSAL.run_phase_authorization_boundary(receipt), "InstrumentFailure")
                read.assert_not_called()
        self.assertEqual(receipt, {})
        self.assertNotIn("CARGO_REGISTRY_TOKEN", environment.value_reads)

    def test_token_appearing_during_rehearsal_stays_non_clean(self) -> None:
        values = {"BENIGN": "retained"}
        environment = TokenValueTrap(values)

        def introduce_token(receipt, **kwargs):
            values["CARGO_REGISTRY_TOKEN"] = "synthetic-private-token"
            return "Complete"

        with contextlib.ExitStack() as stack:
            stack.enter_context(mock.patch.object(REHEARSAL.os, "environ", environment))
            stack.enter_context(mock.patch.object(REHEARSAL, "resolve_commit", return_value="a" * 40))
            stack.enter_context(mock.patch.object(REHEARSAL, "require_clean_checkout"))
            stack.enter_context(mock.patch.object(
                REHEARSAL, "compute_sha256", return_value="sha256:v1:" + "b" * 64,
            ))
            read = stack.enter_context(mock.patch.object(Path, "read_text"))
            for phase in REQUIRED_PHASES:
                if phase == "authorization_boundary":
                    continue
                name = "run_phase_" + ("docs_and_support" if phase == "docs_and_support_identity" else phase)
                operation = stack.enter_context(mock.patch.object(REHEARSAL, name, return_value="Complete"))
                if phase == "release_identity":
                    operation.side_effect = introduce_token
            receipt = REHEARSAL.build_rehearsal_receipt("HEAD")
            read.assert_not_called()
        self.assertEqual(receipt["phases"]["authorization_boundary"], "InstrumentFailure")
        self.assertEqual(receipt["aggregate_status"], "InstrumentFailure")
        self.assertTrue(all(value is False for value in receipt["zero_mutation_proof"].values()))
        self.assertNotIn("CARGO_REGISTRY_TOKEN", environment.value_reads)

    def test_final_token_observation_controls_builder_and_written_receipt(self) -> None:
        authorization = self.root / REHEARSAL.AUTHORIZATION_ARTIFACT
        authorization.parent.mkdir(parents=True)
        authorization.write_bytes((ROOT / REHEARSAL.AUTHORIZATION_ARTIFACT).read_bytes())
        output = self.sandbox / "final-receipt.json"

        for boundary in ("workflow", "checkout"):
            for workflow_status in ("Complete", "Mismatch"):
                for entry in ("builder", "cli"):
                    for token_value in (None, "", "synthetic-private-token"):
                        with self.subTest(boundary=boundary, workflow_status=workflow_status,
                                          entry=entry, token_value=token_value):
                            values = {"BENIGN": "retained"}
                            environment = TokenValueTrap(values)
                            checkout_calls = 0

                            def introduce_token() -> None:
                                if token_value is not None:
                                    values["CARGO_REGISTRY_TOKEN"] = token_value

                            def workflow(receipt) -> str:
                                self.assertEqual(
                                    receipt["phases"]["authorization_boundary"], "Incomplete",
                                )
                                self.assertIs(receipt["authorization_boundary"]["token_present"], False)
                                if boundary == "workflow":
                                    introduce_token()
                                return workflow_status

                            def checkout(commit_sha) -> None:
                                nonlocal checkout_calls
                                checkout_calls += 1
                                if checkout_calls == 2 and boundary == "checkout":
                                    introduce_token()

                            stdout, stderr = io.StringIO(), io.StringIO()
                            with contextlib.ExitStack() as stack:
                                stack.enter_context(mock.patch.object(REHEARSAL.os, "environ", environment))
                                stack.enter_context(mock.patch.object(REHEARSAL, "resolve_commit", return_value="a" * 40))
                                stack.enter_context(mock.patch.object(REHEARSAL, "require_clean_checkout", side_effect=checkout))
                                stack.enter_context(mock.patch.object(
                                    REHEARSAL, "compute_sha256", return_value="sha256:v1:" + "b" * 64,
                                ))
                                authorize = stack.enter_context(mock.patch.object(
                                    REHEARSAL, "run_phase_authorization_boundary",
                                    wraps=REHEARSAL.run_phase_authorization_boundary,
                                ))
                                for phase in REQUIRED_PHASES:
                                    if phase in {"authorization_boundary", "workflow_graph_permissions"}:
                                        continue
                                    name = "run_phase_" + ("docs_and_support" if phase == "docs_and_support_identity" else phase)
                                    stack.enter_context(mock.patch.object(REHEARSAL, name, return_value="Complete"))
                                stack.enter_context(mock.patch.object(
                                    REHEARSAL, "run_phase_workflow_graph_permissions", side_effect=workflow,
                                ))
                                stack.enter_context(contextlib.redirect_stdout(stdout))
                                stack.enter_context(contextlib.redirect_stderr(stderr))
                                if entry == "builder":
                                    receipt = REHEARSAL.build_rehearsal_receipt("HEAD")
                                else:
                                    stack.enter_context(mock.patch.object(
                                        sys, "argv", ["release-rehearsal", "--output", str(output)],
                                    ))
                                    self.assertEqual(REHEARSAL.main(), 1)
                                    receipt = json.loads(output.read_text(encoding="utf-8"))
                                authorize.assert_called_once()

                            self.assertEqual(checkout_calls, 2)
                            self.assertEqual(set(receipt["phases"]), set(REQUIRED_PHASES))
                            self.assertEqual(receipt["phases"]["workflow_graph_permissions"], workflow_status)
                            self.assertEqual(len(receipt["zero_mutation_proof"]), 7)
                            self.assertTrue(all(value is False for value in receipt["zero_mutation_proof"].values()))
                            self.assertNotIn("CARGO_REGISTRY_TOKEN", environment.value_reads)
                            self.assertNotIn("synthetic-private-token", stdout.getvalue() + stderr.getvalue())
                            self.assertEqual(stderr.getvalue(), "")
                            if token_value is None:
                                expected = "Incomplete" if workflow_status == "Complete" else "Mismatch"
                                self.assertEqual(receipt["aggregate_status"], expected)
                                self.assertEqual(receipt["phases"]["authorization_boundary"], "Incomplete")
                                self.assertIs(receipt["authorization_boundary"]["token_present"], False)
                            else:
                                self.assertEqual(receipt["aggregate_status"], "InstrumentFailure")
                                self.assertEqual(receipt["phases"]["authorization_boundary"], "InstrumentFailure")
                                self.assertNotIn("authorization_boundary", receipt)

    def test_stdout_and_artifact_destinations_preserve_status_and_source(self) -> None:
        source = self.root / "Cargo.toml"
        source.write_text("source sentinel", encoding="utf-8")
        result, stdout, stderr, phases = self.invoke(None)
        self.assertEqual(result, 1)
        self.assertEqual(json.loads(stdout), {"aggregate_status": "Incomplete"})
        self.assertEqual(stderr, "")
        phases.assert_called_once()
        for output in (self.root / "target/receipts/new.json", self.sandbox / "external.json"):
            with self.subTest(output=output):
                result, stdout, stderr, phases = self.invoke(output)
                self.assertEqual(result, 1)
                self.assertIn("Receipt written", stdout)
                self.assertEqual(stderr, "")
                self.assertEqual(json.loads(output.read_text()), {"aggregate_status": "Incomplete"})
                phases.assert_called_once()
        self.assertEqual(source.read_text(), "source sentinel")

    def test_source_tracked_and_invalid_destinations_reject_before_phases(self) -> None:
        directory = self.sandbox / "directory"
        directory.mkdir()
        parent_file = self.sandbox / "file"
        parent_file.write_text("sentinel")
        for output in (self.root / "Cargo.toml", self.root / "new-source.json",
                       self.root / "target/tracked.json", directory, parent_file / "receipt.json"):
            with self.subTest(output=output):
                result, stdout, stderr, phases = self.invoke(output)
                self.assertEqual(result, 2)
                self.assertEqual(stdout, "")
                self.assertIn("instrumentation failed", stderr)
                phases.assert_not_called()
        self.assertEqual(parent_file.read_text(), "sentinel")

    def test_aliases_reject_before_phases(self) -> None:
        source = self.root / "Cargo.toml"
        source.write_text("sentinel")
        link = self.sandbox / "alias"
        try:
            link.symlink_to(self.root, target_is_directory=True)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"directory symlinks unsupported: {error}")
        result, stdout, _, phases = self.invoke(link / "target/new.json")
        self.assertEqual((result, stdout), (2, ""))
        phases.assert_not_called()

    def test_hardlinked_leaf_rejects_before_phases(self) -> None:
        source = self.root / "Cargo.toml"
        source.write_text("sentinel")
        link = self.sandbox / "alias"
        os.link(source, link)
        result, stdout, _, phases = self.invoke(link)
        self.assertEqual((result, stdout), (2, ""))
        phases.assert_not_called()
        self.assertEqual(source.read_text(), "sentinel")

    @unittest.skipUnless(os.name == "nt", "Windows junction metadata control")
    def test_dangling_junction_rejects_before_phases(self) -> None:
        target = self.sandbox / "junction target with spaces"
        link = self.sandbox / "junction with spaces"
        target.mkdir()

        def remove_junction() -> None:
            if os.path.lexists(link):
                # Remove the reparse entry itself, including a dangling one.
                link.rmdir()

        # Register before setup so a failed or timed-out creator is cleaned up.
        self.addCleanup(remove_junction)
        # Avoid PowerShell setup timeouts (#4439) and its old stdin hang (#4346).
        # Keep this a raw Windows command line: list2cmdline would backslash-escape
        # the quotes, which cmd.exe does not use as its quote-escaping convention.
        # /d suppresses AutoRun, /v:off preserves literal ! in inherited paths,
        # and quoted environment values keep temporary paths out of command text.
        result = self.real_run(
            'cmd.exe /d /v:off /c mklink /J '
            '"%REHEARSAL_TEST_LINK%" "%REHEARSAL_TEST_TARGET%"',
            env={**os.environ, "REHEARSAL_TEST_LINK": str(link),
                 "REHEARSAL_TEST_TARGET": str(target)},
            capture_output=True, text=True, timeout=15, check=False,
            creationflags=subprocess.CREATE_NO_WINDOW,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        target.rmdir()
        self.assertFalse(link.exists())
        self.assertTrue(link.lstat().st_file_attributes & stat.FILE_ATTRIBUTE_REPARSE_POINT)
        for output in (link, link / "receipt.json"):
            with self.subTest(output=output):
                result, stdout, stderr, phases = self.invoke(output)
                self.assertEqual((result, stdout), (2, ""))
                self.assertIn("filesystem alias", stderr)
                phases.assert_not_called()
        self.assertFalse(target.exists())
        remove_junction()
        self.assertFalse(os.path.lexists(link))

    def test_unignored_artifact_and_git_failure_reject_before_phases(self) -> None:
        for failed_command in ("ls-files", "check-ignore"):
            with self.subTest(command=failed_command):
                def inspect(argv, **kwargs):
                    return subprocess.CompletedProcess(
                        argv, 1 if failed_command in argv else 0, b"", b"",
                    )
                with mock.patch.object(REHEARSAL.subprocess, "run", side_effect=inspect):
                    result, stdout, _, phases = self.invoke(self.root / "target/new.json")
                self.assertEqual((result, stdout), (2, ""))
                phases.assert_not_called()

    def test_existing_receipt_is_replaced_only_after_complete_write(self) -> None:
        output = self.sandbox / "receipt.json"
        output.write_text("old receipt")
        result, _, _, _ = self.invoke(output)
        self.assertEqual(result, 1)
        self.assertEqual(json.loads(output.read_text()), {"aggregate_status": "Incomplete"})
        self.assertEqual(list(self.sandbox.glob(".rehearsal-*")), [])

    def test_failed_output_steps_preserve_existing_receipt(self) -> None:
        output = self.sandbox / "receipt.json"
        for operation in ("mkstemp", "fsync", "replace"):
            with self.subTest(operation=operation):
                output.write_text("old receipt")
                owner = REHEARSAL.tempfile if operation == "mkstemp" else REHEARSAL.os
                with mock.patch.object(owner, operation, side_effect=OSError("injected output failure")):
                    result, stdout, stderr, phases = self.invoke(output)
                self.assertEqual((result, stdout), (2, ""))
                self.assertIn("instrumentation failed", stderr)
                phases.assert_called_once()
                self.assertEqual(output.read_text(), "old receipt")
                self.assertEqual(list(self.sandbox.glob(".rehearsal-*")), [])


class TestCandidateIdentity(unittest.TestCase):
    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.candidate = Path(directory.name) / ("candidate.exe" if os.name == "nt" else "candidate")
        self.candidate.write_bytes(b"identified candidate bytes")
        environment = mock.patch.dict(os.environ, {}, clear=True)
        environment.start()
        self.addCleanup(environment.stop)
        self.digest = REHEARSAL.compute_sha256(self.candidate)
        self.version = "0.2.0"
        self.projection = {
            "schema": "cargo-allow.release-identity.v1",
            "result": "validated",
            "version": "0.2.0",
            "tag": "v0.2.0",
            "tag_source": "derived",
            "channel": "stable",
            "rc_ordinal": None,
            "github_prerelease": False,
        }

    def invoke(self, *, result=None, error=None, candidate=None, digest=None):
        receipt = {}
        diagnostic = io.StringIO()
        if result is None:
            result = subprocess.CompletedProcess(
                [], 0, json.dumps(self.projection), "synthetic-private-child-output"
            )
        with (
            mock.patch.object(REHEARSAL, "_workspace_version", return_value=self.version),
            mock.patch.object(REHEARSAL.subprocess, "run", return_value=result,
                              side_effect=error) as run,
            contextlib.redirect_stderr(diagnostic),
        ):
            status = REHEARSAL.run_phase_release_identity(
                receipt, candidate_executable=candidate or self.candidate,
                candidate_sha256=digest or self.digest,
            )
        self.assertNotIn("synthetic-private", diagnostic.getvalue())
        return status, receipt, run, diagnostic.getvalue()

    def test_candidate_runs_from_verified_private_copy(self) -> None:
        observed = []

        def observe_copy(command, **kwargs):
            observed.append(Path(command[0]).read_bytes())
            return subprocess.CompletedProcess([], 0, json.dumps(self.projection), "")

        with mock.patch.dict(os.environ, {"CARGO_REGISTRY_TOKEN": "synthetic-private-token"}):
            status, receipt, run, diagnostic = self.invoke(error=observe_copy)
        self.assertEqual(status, "Complete")
        self.assertEqual(receipt["release_identity"]["version"], "0.2.0")
        launched = Path(run.call_args.args[0][0])
        self.assertNotEqual(launched, self.candidate)
        self.assertEqual(observed, [self.candidate.read_bytes()])
        self.assertEqual(run.call_args.args[0][1:],
                         ["release-identity", "--version", "0.2.0"])
        self.assertFalse(launched.parent.exists())
        self.assertTrue(self.candidate.exists())
        self.assertNotIn("CARGO_REGISTRY_TOKEN", run.call_args.kwargs["env"])
        self.assertIn(self.digest, diagnostic)
        self.assertEqual(run.call_count, 1)

    def test_private_copy_preserves_executable_suffix_and_mode(self) -> None:
        self.candidate = self.candidate.with_suffix(".exe")
        self.candidate.write_bytes(b"identified candidate bytes")
        observed = []

        def observe_copy(command, **kwargs):
            copy = Path(command[0])
            observed.append((copy.suffix, copy.stat().st_mode))
            return subprocess.CompletedProcess([], 0, json.dumps(self.projection), "")

        status, _, _, _ = self.invoke(error=observe_copy)
        self.assertEqual(status, "Complete")
        self.assertEqual(observed[0][0], ".exe")
        if os.name != "nt":
            self.assertEqual(observed[0][1] & 0o777, 0o500)

    def test_private_copy_is_cleaned_after_child_failure_or_timeout(self) -> None:
        for timeout in (False, True):
            with self.subTest(timeout=timeout):
                observed = []

                def fail_child(command, **kwargs):
                    observed.append(Path(command[0]))
                    if timeout:
                        raise subprocess.TimeoutExpired(command, 300)
                    return subprocess.CompletedProcess([], 2, "", "")

                status, receipt, _, _ = self.invoke(error=fail_child)
                self.assertNotEqual(status, "Complete")
                self.assertNotIn("release_identity", receipt)
                self.assertFalse(observed[0].parent.exists())
                self.assertTrue(self.candidate.exists())

    def test_private_copy_failure_does_not_execute_or_expose_paths(self) -> None:
        copies = []

        def fail_copy(source, destination):
            copies.append(destination)
            raise OSError("synthetic-private-copy-path")

        with mock.patch.object(REHEARSAL.shutil, "copyfile", side_effect=fail_copy):
            status, receipt, run, diagnostic = self.invoke()
        self.assertEqual(status, "InstrumentFailure")
        self.assertNotIn("release_identity", receipt)
        self.assertIn("instrument_unavailable", diagnostic)
        run.assert_not_called()
        self.assertFalse(copies[0].parent.exists())

    def test_private_copy_digest_mismatch_does_not_execute(self) -> None:
        copies = []

        def invalid_copy(source, destination):
            copies.append(destination)
            destination.write_bytes(b"different copied bytes")

        with mock.patch.object(REHEARSAL.shutil, "copyfile", side_effect=invalid_copy):
            status, receipt, run, diagnostic = self.invoke()
        self.assertEqual(status, "Mismatch")
        self.assertNotIn("release_identity", receipt)
        self.assertIn("candidate_digest_mismatch", diagnostic)
        run.assert_not_called()
        self.assertFalse(copies[0].parent.exists())

    def test_private_copy_cleanup_failure_cannot_be_complete(self) -> None:
        temporary_directory = tempfile.TemporaryDirectory

        @contextlib.contextmanager
        def cleanup_failure(**kwargs):
            with temporary_directory(**kwargs) as directory:
                yield directory
            raise OSError("synthetic-private-cleanup-path")

        with mock.patch.object(REHEARSAL.tempfile, "TemporaryDirectory",
                               side_effect=cleanup_failure):
            status, receipt, run, diagnostic = self.invoke()
        self.assertEqual(status, "InstrumentFailure")
        self.assertNotIn("release_identity", receipt)
        self.assertIn("instrument_unavailable", diagnostic)
        self.assertFalse(Path(run.call_args.args[0][0]).parent.exists())

    def test_missing_candidate_does_not_fall_back(self) -> None:
        status, receipt, run, diagnostic = self.invoke(
            candidate=self.candidate.with_name("missing")
        )
        self.assertEqual(status, "InstrumentFailure")
        self.assertNotIn("release_identity", receipt)
        run.assert_not_called()
        self.assertIn("candidate_unavailable", diagnostic)

    def test_mismatched_digest_does_not_execute(self) -> None:
        status, receipt, run, diagnostic = self.invoke(digest="sha256:v1:" + "0" * 64)
        self.assertEqual(status, "Mismatch")
        self.assertNotIn("release_identity", receipt)
        run.assert_not_called()
        self.assertIn("candidate_digest_mismatch", diagnostic)

    def test_invalid_selection_does_not_execute(self) -> None:
        for candidate, digest in (
            (Path("relative-candidate"), self.digest),
            (self.candidate, "sha256:v1:" + "G" * 64),
            (self.candidate, "synthetic-private-not-a-digest"),
        ):
            with self.subTest(candidate=candidate, digest=digest):
                status, receipt, run, diagnostic = self.invoke(candidate=candidate, digest=digest)
                self.assertEqual(status, "InstrumentFailure")
                self.assertNotIn("release_identity", receipt)
                run.assert_not_called()
                self.assertIn("candidate_selection_invalid", diagnostic)

    def test_unavailable_instrument_is_redacted(self) -> None:
        status, receipt, _, diagnostic = self.invoke(error=OSError("synthetic-private"))
        self.assertEqual(status, "InstrumentFailure")
        self.assertNotIn("release_identity", receipt)
        self.assertIn("instrument_unavailable", diagnostic)

    def test_changed_candidate_is_not_accepted(self) -> None:
        def change_candidate(*args, **kwargs):
            self.candidate.write_bytes(b"changed bytes")
            return subprocess.CompletedProcess([], 0, json.dumps(self.projection), "")
        status, receipt, _, diagnostic = self.invoke(error=change_candidate)
        self.assertEqual(status, "Mismatch")
        self.assertNotIn("release_identity", receipt)
        self.assertIn("candidate_changed", diagnostic)

    def test_failed_child_is_not_accepted(self) -> None:
        status, receipt, _, diagnostic = self.invoke(
            result=subprocess.CompletedProcess([], 2, "synthetic-private", "synthetic-private")
        )
        self.assertEqual(status, "Mismatch")
        self.assertNotIn("release_identity", receipt)
        self.assertIn("child_failed", diagnostic)

    def test_timeout_is_distinct_and_redacted(self) -> None:
        status, receipt, _, diagnostic = self.invoke(
            error=subprocess.TimeoutExpired(["synthetic-private"], 300,
                                            output="synthetic-private")
        )
        self.assertEqual(status, "InstrumentFailure")
        self.assertNotIn("release_identity", receipt)
        self.assertIn("child_timeout", diagnostic)

    def test_malformed_identity_is_not_accepted(self) -> None:
        for output in ("invalid", "[]", "{}", json.dumps({**self.projection, "version": "9.9.9"}),
                       json.dumps({**self.projection, "github_prerelease": "false"})):
            with self.subTest(output=output):
                status, receipt, _, _ = self.invoke(
                    result=subprocess.CompletedProcess([], 0, output, "")
                )
                self.assertNotEqual(status, "Complete")
                self.assertNotIn("release_identity", receipt)

    def test_inconsistent_identity_is_not_accepted(self) -> None:
        for changes in (
            {"tag": ""}, {"tag": "v9.9.9"},
            {"tag_source": "observed"}, {"tag_source": "unknown"},
            {"rc_ordinal": 1}, {"github_prerelease": True},
            {"channel": "release_candidate", "rc_ordinal": 1, "github_prerelease": True},
        ):
            with self.subTest(changes=changes):
                projection = {**self.projection, **changes}
                status, receipt, _, _ = self.invoke(
                    result=subprocess.CompletedProcess([], 0, json.dumps(projection), "")
                )
                self.assertEqual(status, "Mismatch")
                self.assertNotIn("release_identity", receipt)

    def test_rc_identity_matches_its_ordinal_and_prerelease_posture(self) -> None:
        self.version = "0.2.0-rc.2"
        self.projection.update(
            version=self.version, tag="v" + self.version, channel="release_candidate",
            rc_ordinal=2, github_prerelease=True,
        )
        status, receipt, _, _ = self.invoke()
        self.assertEqual(status, "Complete")
        self.assertEqual(receipt["release_identity"]["rc_ordinal"], 2)
        for changes in (
            {"rc_ordinal": None}, {"rc_ordinal": True}, {"rc_ordinal": 0},
            {"rc_ordinal": 1}, {"rc_ordinal": 4294967296},
            {"github_prerelease": False},
            {"channel": "stable", "rc_ordinal": None, "github_prerelease": False},
        ):
            with self.subTest(changes=changes):
                projection = {**self.projection, **changes}
                status, receipt, _, _ = self.invoke(
                    result=subprocess.CompletedProcess([], 0, json.dumps(projection), "")
                )
                self.assertEqual(status, "Mismatch")
                self.assertNotIn("release_identity", receipt)

    def test_candidate_arguments_must_be_paired(self) -> None:
        for candidate, digest in ((self.candidate, None), (None, self.digest)):
            with self.subTest(candidate=candidate, digest=digest):
                with mock.patch.object(REHEARSAL, "resolve_commit") as resolve:
                    with self.assertRaises(ValueError):
                        REHEARSAL.build_rehearsal_receipt(
                            "HEAD", candidate_executable=candidate, candidate_sha256=digest
                        )
                    resolve.assert_not_called()

    def test_standalone_still_uses_deliberate_cargo_build(self) -> None:
        with (
            mock.patch.object(REHEARSAL, "_select_tool", return_value=sys.executable),
            mock.patch.object(REHEARSAL, "_workspace_version", return_value="0.2.0"),
            mock.patch.object(REHEARSAL.subprocess, "run", return_value=
                              subprocess.CompletedProcess([], 0, json.dumps(self.projection), "")) as run,
        ):
            self.assertEqual(REHEARSAL.run_phase_release_identity({}), "Complete")
        self.assertEqual(run.call_args.args[0][:2], ["cargo", "run"])


class TestEscapedDefectCorpus(unittest.TestCase):
    """The corpus phase's fail-closed row decisions with synthetic instruments."""

    def setUp(self) -> None:
        self.receipt = {
            "phases": {name: "Complete" for name in REQUIRED_PHASES},
            "release_identity": {"version": "0.2.0", "tag": "v0.2.0"},
            "docs_and_support_identity": {
                "release_record": "docs/release/0.2.0.md",
                "github_note": "docs/release/github/v0.2.0.md",
            },
            "workflow_graph_permissions": {
                "top_level_read_scoped": True, "top_level_write_scoped": True,
                "github_release_scoped": True, "authorized_namespace_mode": True,
            },
            "publisher_state_machine": {
                "fixture_matrix": "scripts/test-release-topology-publisher.py",
            },
        }
        self.rc_projection = {
            "schema": "cargo-allow.release-identity.v1", "result": "validated",
            "version": "0.2.0-rc.10", "tag": "v0.2.0-rc.10",
            "channel": "release_candidate", "rc_ordinal": 10, "github_prerelease": True,
        }
        self.child_rows = [
            {"incident_id": f"RC1-ESCAPE-{number:03d}", "result": "proven", "detail": "x"}
            for number in (3, 4, 5, 6, 7, 10, 16)
        ]

    def run_phase(self, *, rc_output=None, rc_status=0, rc_zero_accepted=False,
                  child_output=None, child_status=0):
        calls = []

        def instrument(command, *, operation, selected=None, **options):
            calls.append(operation)
            if operation == "release-identity":
                version = selected[0]
                if version == "0.2.0-rc.0":
                    if rc_zero_accepted:
                        return subprocess.CompletedProcess(
                            [], 0, json.dumps(self.rc_projection), "")
                    return subprocess.CompletedProcess([], 2, "", "rejected")
                output = json.dumps(self.rc_projection) if rc_output is None else rc_output
                return subprocess.CompletedProcess([], rc_status, output, "")
            if operation == "corpus-proof":
                output = json.dumps({"schema": "fixture", "rows": self.child_rows}) \
                    if child_output is None else child_output
                return subprocess.CompletedProcess([], child_status, output, "")
            raise AssertionError(f"unselected corpus operation {operation}")

        with mock.patch.object(REHEARSAL, "_run_process", side_effect=instrument):
            status = REHEARSAL.run_phase_escaped_defect_corpus(self.receipt)
        return status, calls

    def test_complete_corpus_records_all_sixteen_rows_with_owner_status(self) -> None:
        status, calls = self.run_phase()
        self.assertEqual(status, "Complete")
        self.assertEqual(calls, ["release-identity", "release-identity", "corpus-proof"])
        record = self.receipt["escaped_defect_corpus"]
        self.assertEqual(
            record["schema"], "cargo-allow.release-rehearsal-escaped-defect-corpus.v1"
        )
        self.assertEqual(
            record["fixture_matrix"], "scripts/test-release-escaped-defect-corpus.py"
        )
        self.assertEqual(len(record["rows"]), 16)
        by_id = {row["incident_id"]: row for row in record["rows"]}
        self.assertEqual(
            sorted(by_id), [f"RC1-ESCAPE-{number:03d}" for number in range(1, 17)]
        )
        for incident_id, row in by_id.items():
            self.assertTrue(row["owner"].startswith("#"), incident_id)
            if row["status"] == "pending_upstream":
                self.assertEqual(row["result"], "recorded_pending", incident_id)
                self.assertTrue(row["claim_boundary"])
            else:
                self.assertEqual(row["result"], "proven", incident_id)
        self.assertEqual(by_id["RC1-ESCAPE-003"]["status"], "fault_injected")
        self.assertEqual(by_id["RC1-ESCAPE-011"]["status"], "typed_identity")
        self.assertEqual(by_id["RC1-ESCAPE-012"]["status"], "phase_binding")
        self.assertEqual(by_id["RC1-ESCAPE-015"]["owner"], "#2496/#2497/#3785")

    def test_rc_projection_regressions_are_mismatches(self) -> None:
        for changes, expected_rows in (
            ({"rc_ordinal": 2}, ("RC1-ESCAPE-002",)),
            ({"github_prerelease": False}, ("RC1-ESCAPE-002", "RC1-ESCAPE-011")),
            ({"channel": "stable", "rc_ordinal": None}, ("RC1-ESCAPE-002",)),
            ({"tag": "v0.2.0-rc.2"}, ("RC1-ESCAPE-002",)),
        ):
            with self.subTest(changes=changes):
                self.rc_projection = {
                    **{
                        "schema": "cargo-allow.release-identity.v1", "result": "validated",
                        "version": "0.2.0-rc.10", "tag": "v0.2.0-rc.10",
                        "channel": "release_candidate", "rc_ordinal": 10,
                        "github_prerelease": True,
                    },
                    **changes,
                }
                status, _ = self.run_phase()
                self.assertEqual(status, "Mismatch")
                self.assertNotIn("escaped_defect_corpus", self.receipt)

    def test_accepted_rc_zero_ordinal_is_a_mismatch(self) -> None:
        status, _ = self.run_phase(rc_zero_accepted=True)
        self.assertEqual(status, "Mismatch")
        self.assertNotIn("escaped_defect_corpus", self.receipt)

    def test_rejected_corpus_fixture_row_is_a_mismatch(self) -> None:
        self.child_rows[3] = {
            "incident_id": "RC1-ESCAPE-006", "result": "rejected", "detail": "accepted",
        }
        status, _ = self.run_phase(child_status=1)
        self.assertEqual(status, "Mismatch")
        self.assertNotIn("escaped_defect_corpus", self.receipt)

    def test_invalid_corpus_child_output_is_not_proven(self) -> None:
        for output, status_value, expected in (
            ("", 1, "InstrumentFailure"),
            ("not-json", 0, "Mismatch"),
            (json.dumps({"rows": []}), 0, "Mismatch"),
        ):
            with self.subTest(output=output[:20], expected=expected):
                status, _ = self.run_phase(
                    child_output=output, child_status=status_value,
                )
                self.assertEqual(status, expected)
                self.assertNotIn("escaped_defect_corpus", self.receipt)

    def test_missing_corpus_child_instrument_is_instrument_failure(self) -> None:
        with mock.patch.object(
            REHEARSAL, "_run_process",
            side_effect=OSError("fixture corpus child unavailable"),
        ):
            status = REHEARSAL.run_phase_escaped_defect_corpus(self.receipt)
        self.assertEqual(status, "InstrumentFailure")
        self.assertNotIn("escaped_defect_corpus", self.receipt)

    def test_blocked_binding_phase_is_a_mismatch(self) -> None:
        for phase in ("docs_and_support_identity", "workflow_graph_permissions",
                      "publisher_state_machine"):
            with self.subTest(phase=phase):
                self.receipt["phases"][phase] = "Mismatch"
                status, _ = self.run_phase()
                self.assertEqual(status, "Mismatch")
                self.assertNotIn("escaped_defect_corpus", self.receipt)

    def test_record_note_and_permission_bindings_are_exact(self) -> None:
        self.receipt["docs_and_support_identity"]["release_record"] = "docs/release/9.9.9.md"
        status, _ = self.run_phase()
        self.assertEqual(status, "Mismatch")
        self.assertNotIn("escaped_defect_corpus", self.receipt)
        self.receipt["docs_and_support_identity"]["release_record"] = "docs/release/0.2.0.md"
        self.receipt["workflow_graph_permissions"]["github_release_scoped"] = False
        status, _ = self.run_phase()
        self.assertEqual(status, "Mismatch")
        self.assertNotIn("escaped_defect_corpus", self.receipt)

    def test_changed_candidate_is_not_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = Path(directory) / ("candidate.exe" if os.name == "nt" else "candidate")
            candidate.write_bytes(b"corpus candidate bytes")
            digest = REHEARSAL.compute_sha256(candidate)
            with mock.patch.object(REHEARSAL, "_run_process") as terminal:
                status = REHEARSAL.run_phase_escaped_defect_corpus(
                    self.receipt, candidate_executable=candidate,
                    candidate_sha256="sha256:v1:" + "0" * 64,
                )
            self.assertEqual(status, "Mismatch")
            terminal.assert_not_called()
        self.assertNotIn("escaped_defect_corpus", self.receipt)


class TestReleaseRehearsal(unittest.TestCase):
    def test_characterization_cannot_report_complete(self) -> None:
        receipt = REHEARSAL.build_rehearsal_receipt("HEAD")

        self.assertEqual(receipt["schema_version"], "1.0")
        self.assertTrue(
            receipt["subject_lockfile_digest"].startswith("sha256:v1:")
        )
        self.assertTrue(
            receipt["subject_topology_digest"].startswith("sha256:v1:")
        )
        self.assertNotEqual(receipt["aggregate_status"], "Complete")

        proof = receipt["zero_mutation_proof"]
        self.assertTrue(proof)
        self.assertFalse(any(proof.values()))

        phases = receipt["phases"]
        self.assertEqual(set(phases), set(REQUIRED_PHASES))
        for phase_name in REHEARSAL.CHARACTERIZATION_PHASES:
            self.assertNotEqual(
                phases[phase_name],
                "Complete",
                "characterization-only phases cannot manufacture completion",
            )

        assets = receipt.get("manifest_and_assets")
        if phases["manifest_and_assets"] == "Complete":
            self.assertIsInstance(assets, dict)
            self.assertEqual(
                assets["fixture_matrix"],
                "scripts/test-final-packaged-surface.py",
            )

        workflow = receipt.get("workflow_graph_permissions")
        if phases["workflow_graph_permissions"] == "Complete":
            self.assertIsInstance(workflow, dict)
            self.assertIn(workflow["mode"], {"yaml", "text"})
            self.assertTrue(workflow["top_level_read_scoped"])
            self.assertTrue(workflow["top_level_write_scoped"])
            self.assertTrue(workflow["github_release_scoped"])
            self.assertTrue(workflow["authorized_namespace_mode"])

        authorization = receipt.get("authorization_boundary")
        self.assertEqual(phases["authorization_boundary"], "Incomplete")
        self.assertIsInstance(authorization, dict)
        self.assertEqual(authorization["named_release"], "v0.2.0")
        self.assertFalse(authorization["token_present"])

        identity = receipt.get("release_identity")
        docs = receipt.get("docs_and_support_identity")
        if phases["docs_and_support_identity"] == "Complete":
            self.assertIsInstance(docs, dict)
            self.assertTrue(docs["release_record"].endswith(f"/{identity['version']}.md"))
            self.assertTrue(
                docs["github_note"].endswith(f"/github/{identity['tag']}.md")
            )
            self.assertEqual(
                docs["history_check"],
                "scripts/generate-changie-history.py --check",
            )

        packages = receipt.get("candidate_package_set")
        if phases["candidate_package_set"] == "Complete":
            self.assertIsInstance(packages, dict)
            rows = packages["rows"]
            self.assertEqual(len(rows), 10)
            identity_version = identity["version"]
            for row in rows:
                self.assertEqual(row["version"], identity_version)
                self.assertTrue(row["sha256"].startswith("sha256:"))
                self.assertGreater(row["size_bytes"], 0)

        machine = receipt.get("publisher_state_machine")
        if phases["publisher_state_machine"] == "Complete":
            self.assertIsInstance(machine, dict)
            self.assertEqual(
                machine["fixture_matrix"],
                "scripts/test-release-topology-publisher.py",
            )

        shared = receipt.get("shared_prerequisites")
        if phases["shared_prerequisites"] == "Complete":
            self.assertIsInstance(shared, list)
            self.assertEqual(len(shared), 3)
            for row in shared:
                self.assertEqual(row["state"], "already_published_exact")
                self.assertTrue(row["registry_checksum"].startswith("sha256:"))

        corpus = receipt.get("escaped_defect_corpus")
        if phases["escaped_defect_corpus"] == "Complete":
            self.assertIsInstance(corpus, dict)
            self.assertEqual(
                corpus["schema"],
                "cargo-allow.release-rehearsal-escaped-defect-corpus.v1",
            )
            self.assertEqual(
                corpus["fixture_matrix"],
                "scripts/test-release-escaped-defect-corpus.py",
            )
            rows = corpus["rows"]
            self.assertEqual(len(rows), 16)
            self.assertEqual(
                [row["incident_id"] for row in rows],
                [f"RC1-ESCAPE-{number:03d}" for number in range(1, 17)],
            )
            for row in rows:
                self.assertTrue(row["owner"].startswith("#"))
                self.assertIn(row["status"], {
                    "fault_injected", "typed_identity", "phase_binding", "pending_upstream",
                })
                if row["status"] == "pending_upstream":
                    self.assertEqual(row["result"], "recorded_pending")
                else:
                    self.assertEqual(row["result"], "proven")

        self.assertIsInstance(identity, dict)
        self.assertEqual(identity["schema"], "cargo-allow.release-identity.v1")
        self.assertEqual(identity["result"] if "result" in identity else "validated", "validated")
        self.assertTrue(identity["version"])
        self.assertTrue(identity["tag"].startswith("v"))
        self.assertIn(identity["channel"], {"stable", "release_candidate"})
        self.assertIsInstance(identity["github_prerelease"], bool)
        if identity["channel"] == "release_candidate":
            self.assertTrue(identity["github_prerelease"])
            self.assertIsNotNone(identity["rc_ordinal"])
        else:
            self.assertFalse(identity["github_prerelease"])
            self.assertIsNone(identity["rc_ordinal"])

    def test_arbitrary_nonexistent_commit_is_rejected(self) -> None:
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt(
                "0123456789abcdef0123456789abcdef01234567"
            )

    def test_option_like_commit_is_rejected_before_git(self) -> None:
        with self.assertRaises(ValueError):
            REHEARSAL.resolve_commit("--help")

    def test_receipt_output_rejects_symlink_leaf(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target.json"
            target.write_text("sentinel", encoding="utf-8")
            output = root / "receipt.json"
            output.symlink_to(target)

            with self.assertRaises(OSError):
                REHEARSAL._write_receipt(output, "{}")
            self.assertEqual(target.read_text(encoding="utf-8"), "sentinel")

    def test_registry_token_presence_is_non_clean(self) -> None:
        old_value = os.environ.get("CARGO_REGISTRY_TOKEN")
        try:
            os.environ["CARGO_REGISTRY_TOKEN"] = "synthetic-secret"
            self.assertEqual(
                REHEARSAL.run_phase_authorization_boundary({}),
                "InstrumentFailure",
            )
        finally:
            if old_value is None:
                os.environ.pop("CARGO_REGISTRY_TOKEN", None)
            else:
                os.environ["CARGO_REGISTRY_TOKEN"] = old_value

    def test_cli_exits_nonzero_with_machine_readable_characterization(self) -> None:
        result = subprocess.run(
            [sys.executable, str(REHEARSAL_PATH), "--commit", "HEAD"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn('"aggregate_status"', result.stdout)
        self.assertNotIn('"aggregate_status": "Complete"', result.stdout)


class TestRehearsalSubjectBinding(unittest.TestCase):
    """Exercise checkout admission with real Git and no real phase execution."""

    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory(prefix="rehearsal-subject-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.git("init", "--quiet")
        files = {
            ".gitignore": "target/\n__pycache__/\n",
            "Cargo.lock": "version = 4\n",
            "policy/product-package-topology-v2.toml": "schema_version = '2.0'\n",
            ".github/workflows/release.yml": "name: fixture\n",
        }
        for name, content in files.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
        self.git("add", ".")
        self.git("commit", "--quiet", "-m", "first subject")
        self.previous = self.git("rev-parse", "HEAD").strip()
        (self.root / "Cargo.lock").write_text("version = 4\n# current\n", encoding="utf-8")
        self.git("add", "Cargo.lock")
        self.git("commit", "--quiet", "-m", "current subject")
        self.head = self.git("rev-parse", "HEAD").strip()
        root_patch = mock.patch.object(REHEARSAL, "ROOT", self.root)
        root_patch.start()
        self.addCleanup(root_patch.stop)
        self.phases = []
        for name in (
            "run_phase_release_identity",
            "run_phase_candidate_package_set",
            "run_phase_shared_prerequisites",
            "run_phase_publisher_state_machine",
            "run_phase_docs_and_support",
            "run_phase_manifest_and_assets",
            "run_phase_authorization_boundary",
            "run_phase_workflow_graph_permissions",
            "run_phase_escaped_defect_corpus",
        ):
            patch = mock.patch.object(REHEARSAL, name, return_value="Incomplete")
            self.phases.append(patch.start())
            self.addCleanup(patch.stop)

    def git(self, *args: str) -> str:
        return subprocess.run(
            [
                "git", "-c", "core.autocrlf=false", "-c", "commit.gpgsign=false",
                "-c", f"core.hooksPath={self.root / 'no-hooks'}",
                "-c", "user.name=Rehearsal fixture",
                "-c", "user.email=rehearsal@example.invalid", *args,
            ],
            cwd=self.root, capture_output=True, text=True, timeout=15, check=True,
        ).stdout

    def require_no_phases(self) -> None:
        for phase in self.phases:
            phase.assert_not_called()

    def test_clean_head_and_ignored_artifacts_preserve_characterization(self) -> None:
        for name in (
            "target/cargo-allow/prior.json", "crates/fixture/target/debug/output",
            "__pycache__/fixture.pyc", "scripts/__pycache__/fixture.pyc",
        ):
            artifact = self.root / name
            artifact.parent.mkdir(parents=True, exist_ok=True)
            artifact.write_text("{}", encoding="utf-8")
        receipt = REHEARSAL.build_rehearsal_receipt("HEAD")
        self.assertEqual(receipt["commit_sha"], self.head)
        self.assertEqual(receipt["aggregate_status"], "Incomplete")
        self.assertTrue(all(value is False for value in receipt["zero_mutation_proof"].values()))
        for phase in self.phases:
            phase.assert_called_once()

    def test_existing_foreign_commit_is_rejected_before_phases(self) -> None:
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt(self.previous)
        self.require_no_phases()

    def test_clean_non_ascii_linked_worktree_is_admitted(self) -> None:
        directory = tempfile.TemporaryDirectory(prefix="rehearsal-\u00e9-")
        self.addCleanup(directory.cleanup)
        worktree = Path(directory.name) / "checkout"
        self.git("worktree", "add", "--quiet", "--detach", str(worktree), self.head)
        self.addCleanup(self.git, "worktree", "remove", str(worktree))
        with mock.patch.object(REHEARSAL, "ROOT", worktree):
            receipt = REHEARSAL.build_rehearsal_receipt("HEAD")
        self.assertEqual(receipt["commit_sha"], self.head)
        self.assertEqual(receipt["aggregate_status"], "Incomplete")
        for phase in self.phases:
            phase.assert_called_once()

    def test_repository_selection_environment_is_rejected_before_git(self) -> None:
        for name in (
            "GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR", "GIT_INDEX_FILE", "GIT_NAMESPACE",
        ):
            with self.subTest(variable=name), mock.patch.dict(os.environ, {name: "fixture"}):
                with mock.patch.object(REHEARSAL.subprocess, "run") as run:
                    with self.assertRaisesRegex(ValueError, "repository-selection environment"):
                        REHEARSAL.build_rehearsal_receipt("HEAD")
                    run.assert_not_called()
                self.require_no_phases()

    def test_admission_git_calls_use_supported_inspection_settings(self) -> None:
        with mock.patch.object(REHEARSAL.subprocess, "run", wraps=subprocess.run) as run:
            receipt = REHEARSAL.build_rehearsal_receipt("HEAD")
        self.assertEqual(receipt["commit_sha"], self.head)
        self.assertTrue(run.call_args_list)
        for call in run.call_args_list:
            self.assertEqual(call.args[0][:2], ["git", "--no-replace-objects"])
            self.assertIsInstance(call.kwargs.get("env"), dict)
            self.assertNotIn("CARGO_REGISTRY_TOKEN", call.kwargs["env"])
            if "status" in call.args[0]:
                for setting in ("core.checkStat=default", "core.ignoreStat=false", "core.trustctime=true"):
                    self.assertIn(setting, call.args[0])

    def test_git_configuration_environment_is_rejected_before_git(self) -> None:
        for name in (
            "GIT_CONFIG", "GIT_CONFIG_COUNT", "GIT_CONFIG_KEY_0", "GIT_CONFIG_VALUE_0",
            "GIT_CONFIG_PARAMETERS", "GIT_CONFIG_GLOBAL", "GIT_CONFIG_SYSTEM",
            "GIT_CONFIG_NOSYSTEM",
        ):
            for value in ("", "fixture"):
                with self.subTest(variable=name, value=value), mock.patch.dict(os.environ, {name: value}):
                    with mock.patch.object(REHEARSAL.subprocess, "run") as run:
                        with self.assertRaisesRegex(ValueError, "GIT_CONFIG"):
                            REHEARSAL.build_rehearsal_receipt("HEAD")
                        run.assert_not_called()
                    self.require_no_phases()

    def test_attribute_source_environment_is_rejected_before_git(self) -> None:
        for value in ("", "HEAD"):
            with self.subTest(value=value), mock.patch.dict(os.environ, {"GIT_ATTR_SOURCE": value}):
                with mock.patch.object(REHEARSAL.subprocess, "run") as run:
                    with self.assertRaisesRegex(ValueError, "GIT_ATTR_SOURCE"):
                        REHEARSAL.build_rehearsal_receipt("HEAD")
                    run.assert_not_called()
                self.require_no_phases()

    def test_disabled_or_malformed_ctime_is_rejected_before_phases(self) -> None:
        for value, diagnostic in (
            ("false", "core.trustctime"),
            ("invalid-boolean", "not an exact repository commit"),
        ):
            with self.subTest(value=value):
                self.git("config", "core.trustctime", value)
                with self.assertRaisesRegex(ValueError, diagnostic):
                    REHEARSAL.build_rehearsal_receipt("HEAD")
                self.require_no_phases()
                self.assertEqual(self.git("config", "--get", "core.trustctime").strip(), value)

    def test_explicit_ctime_trust_preserves_characterization(self) -> None:
        self.git("config", "core.trustctime", "true")
        receipt = REHEARSAL.build_rehearsal_receipt("HEAD")
        self.assertEqual(receipt["commit_sha"], self.head)
        for phase in self.phases:
            phase.assert_called_once()

    def test_ctime_inspection_failure_is_rejected_without_raw_output(self) -> None:
        run = subprocess.run

        def fail_config(command, **kwargs):
            if "config" in command:
                return subprocess.CompletedProcess(command, 128, b"secret-canary", b"secret-canary")
            return run(command, **kwargs)

        with mock.patch.object(REHEARSAL.subprocess, "run", side_effect=fail_config):
            with self.assertRaisesRegex(ValueError, "core.trustctime") as caught:
                REHEARSAL.build_rehearsal_receipt("HEAD")
        self.assertNotIn("secret-canary", str(caught.exception))
        self.require_no_phases()

    def test_disabled_ctime_after_phases_cannot_return_a_receipt(self) -> None:
        def change_config(receipt, *, candidate_executable=None, candidate_sha256=None):
            self.git("config", "core.trustctime", "false")
            return "Incomplete"

        self.phases[0].side_effect = change_config
        with self.assertRaisesRegex(ValueError, "core.trustctime"):
            REHEARSAL.build_rehearsal_receipt("HEAD")

    def require_transformation_rejected_before_status(self) -> None:
        run = subprocess.run

        def inspect_without_status(command, **kwargs):
            if "status" in command:
                return subprocess.CompletedProcess(command, 0, b"", b"")
            return run(command, **kwargs)

        with mock.patch.object(REHEARSAL.subprocess, "run", side_effect=inspect_without_status) as calls:
            with self.assertRaisesRegex(ValueError, "content transformation"):
                REHEARSAL.build_rehearsal_receipt("HEAD")
        self.assertFalse(any("status" in call.args[0] for call in calls.call_args_list))
        self.require_no_phases()

    def test_content_transform_attributes_are_rejected_before_status(self) -> None:
        for attribute in (
            "filter", "filter=fixture", "filter=unspecified", "filter=unset", "-filter",
            "ident", "working-tree-encoding=UTF-8",
        ):
            with self.subTest(attribute=attribute):
                (self.root / ".git/info/attributes").write_text(
                    f"Cargo.lock {attribute}\n", encoding="utf-8",
                )
                self.require_transformation_rejected_before_status()

    def test_external_content_attributes_are_rejected_before_status(self) -> None:
        attributes = self.root / "target/fixture-attributes"
        attributes.parent.mkdir()
        attributes.write_text("Cargo.lock filter=fixture\n", encoding="utf-8")
        self.git("config", "core.attributesFile", str(attributes))
        self.require_transformation_rejected_before_status()

    def test_non_transform_attributes_preserve_characterization(self) -> None:
        (self.root / ".git/info/attributes").write_text(
            "Cargo.lock rehearsal-marker=present\n", encoding="utf-8",
        )
        receipt = REHEARSAL.build_rehearsal_receipt("HEAD")
        self.assertEqual(receipt["commit_sha"], self.head)
        for phase in self.phases:
            phase.assert_called_once()

    def test_content_attribute_inspection_is_complete_and_redacted(self) -> None:
        run = subprocess.run
        for mutation in ("failed", "empty", "terminator", "extra", "path", "attribute"):
            with self.subTest(mutation=mutation):
                def corrupt_attributes(command, **kwargs):
                    result = run(command, **kwargs)
                    if "check-attr" not in command:
                        return result
                    output = result.stdout
                    if mutation == "empty":
                        output = b""
                    elif mutation == "terminator":
                        output = output.removesuffix(b"\0")
                    elif mutation == "extra":
                        output += b"extra\0"
                    elif mutation == "path":
                        output = output.replace(b".github/workflows/release.yml", b"other", 1)
                    elif mutation == "attribute":
                        output = output.replace(b"\0filter\0", b"\0other\0", 1)
                    return subprocess.CompletedProcess(
                        command, 128 if mutation == "failed" else 0, output, b"secret-canary",
                    )

                with mock.patch.object(REHEARSAL.subprocess, "run", side_effect=corrupt_attributes):
                    with self.assertRaisesRegex(ValueError, "content transformation") as caught:
                        REHEARSAL.build_rehearsal_receipt("HEAD")
                self.assertNotIn("secret-canary", str(caught.exception))
                self.require_no_phases()

    def test_defined_attribute_inspection_is_validated(self) -> None:
        (self.root / ".git/info/attributes").write_text(
            "Cargo.lock rehearsal-marker=present\n", encoding="utf-8",
        )
        run = subprocess.run
        for mutation in ("failed", "terminator", "extra", "path", "duplicate"):
            with self.subTest(mutation=mutation):
                def corrupt_defined_attributes(command, **kwargs):
                    result = run(command, **kwargs)
                    if "check-attr" not in command or "--all" not in command:
                        return result
                    output = result.stdout
                    if mutation == "terminator":
                        output = output.removesuffix(b"\0")
                    elif mutation == "extra":
                        output += b"extra\0"
                    elif mutation == "path":
                        output = output.replace(b"Cargo.lock", b"other", 1)
                    elif mutation == "duplicate":
                        output += output
                    return subprocess.CompletedProcess(
                        command, 128 if mutation == "failed" else 0, output, b"secret-canary",
                    )

                with mock.patch.object(REHEARSAL.subprocess, "run", side_effect=corrupt_defined_attributes):
                    with self.assertRaisesRegex(ValueError, "content transformation") as caught:
                        REHEARSAL.build_rehearsal_receipt("HEAD")
                self.assertNotIn("secret-canary", str(caught.exception))
                self.require_no_phases()

    def test_content_attributes_added_by_phase_prevent_receipt_return(self) -> None:
        def change_attributes(receipt, *, candidate_executable=None, candidate_sha256=None):
            (self.root / ".git/info/attributes").write_text("Cargo.lock filter\n", encoding="utf-8")
            return "Incomplete"

        self.phases[0].side_effect = change_attributes
        with self.assertRaisesRegex(ValueError, "content transformation"):
            REHEARSAL.build_rehearsal_receipt("HEAD")

    def test_different_discovered_root_is_rejected_before_phases(self) -> None:
        result = subprocess.CompletedProcess([], 0, os.fsencode(self.root.parent) + b"\n")
        with mock.patch.object(REHEARSAL, "resolve_commit", return_value=self.head):
            with mock.patch.object(REHEARSAL.subprocess, "run", return_value=result):
                with self.assertRaisesRegex(ValueError, "root does not match source root"):
                    REHEARSAL.build_rehearsal_receipt("HEAD")
        self.require_no_phases()

    def test_missing_or_relative_discovered_root_is_rejected(self) -> None:
        for code, output in ((1, b""), (0, b""), (0, b"relative-root\n")):
            with self.subTest(code=code, output=output):
                result = subprocess.CompletedProcess([], code, output)
                with mock.patch.object(REHEARSAL, "resolve_commit", return_value=self.head):
                    with mock.patch.object(REHEARSAL.subprocess, "run", return_value=result):
                        with self.assertRaisesRegex(ValueError, "checkout root"):
                            REHEARSAL.build_rehearsal_receipt("HEAD")
                self.require_no_phases()

    def test_unstaged_selected_source_is_rejected_before_phases(self) -> None:
        for name in (
            "Cargo.lock", "policy/product-package-topology-v2.toml",
            ".github/workflows/release.yml",
        ):
            with self.subTest(path=name):
                path = self.root / name
                original = path.read_bytes()
                path.write_bytes(original + b"# uncommitted\n")
                try:
                    with self.assertRaisesRegex(ValueError, "requires a clean checkout"):
                        REHEARSAL.build_rehearsal_receipt("HEAD")
                    self.require_no_phases()
                finally:
                    path.write_bytes(original)

    def test_staged_source_is_rejected_before_phases(self) -> None:
        (self.root / "Cargo.lock").write_text("staged change\n", encoding="utf-8")
        self.git("add", "Cargo.lock")
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")
        self.require_no_phases()

    def test_index_flags_are_rejected_without_mutating_the_index(self) -> None:
        source = self.root / "Cargo.lock"
        original = source.read_bytes()
        for flag in ("assume-unchanged", "skip-worktree"):
            for changed in (False, True):
                with self.subTest(flag=flag, changed=changed):
                    self.git("update-index", f"--{flag}", "Cargo.lock")
                    if changed:
                        source.write_bytes(original + b"# hidden change\n")
                    # Prove this is the status blind spot, not ordinary dirt.
                    self.assertEqual(self.git("status", "--porcelain"), "")
                    index = (self.root / ".git/index").read_bytes()
                    try:
                        with self.assertRaisesRegex(ValueError, "index flags"):
                            REHEARSAL.build_rehearsal_receipt("HEAD")
                        self.require_no_phases()
                        self.assertEqual((self.root / ".git/index").read_bytes(), index)
                    finally:
                        source.write_bytes(original)
                        self.git("update-index", f"--no-{flag}", "Cargo.lock")

    def add_untracked_source(self) -> None:
        path = self.root / ".changes/untracked.md"
        path.parent.mkdir(exist_ok=True)
        path.write_text("untracked release input\n", encoding="utf-8")

    def test_untracked_phase_input_is_rejected_before_phases(self) -> None:
        self.git("config", "status.showUntrackedFiles", "no")
        self.add_untracked_source()
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")
        self.require_no_phases()

    def test_ignored_phase_input_is_rejected_before_phases(self) -> None:
        self.add_untracked_source()
        (self.root / ".git/info/exclude").write_text(".changes/*.md\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "requires a clean checkout"):
            REHEARSAL.build_rehearsal_receipt("HEAD")
        self.require_no_phases()

    def test_external_ignore_cannot_hide_phase_input(self) -> None:
        self.add_untracked_source()
        exclude = self.root / "target/fixture-excludes"
        exclude.parent.mkdir()
        exclude.write_text(".changes/*.md\n", encoding="utf-8")
        self.git("config", "core.excludesFile", str(exclude))
        with self.assertRaisesRegex(ValueError, "requires a clean checkout"):
            REHEARSAL.build_rehearsal_receipt("HEAD")
        self.require_no_phases()

    def test_ignored_artifact_name_requires_the_exact_directory(self) -> None:
        exclude = self.root / ".git/info/exclude"
        for name in ("target", "target-lookalike.txt", "source with spaces.py"):
            with self.subTest(path=name):
                path = self.root / name
                path.write_text("source, not an artifact directory\n", encoding="utf-8")
                exclude.write_text(f"/{name}\n", encoding="utf-8")
                try:
                    with self.assertRaises(ValueError):
                        REHEARSAL.build_rehearsal_receipt("HEAD")
                    self.require_no_phases()
                finally:
                    path.unlink()

    def test_repository_tooling_state_directory_is_admitted(self) -> None:
        # `.zcode/` is the repository's own ignored agent-tooling state
        # directory (.gitignore); carrying it locally must not fail the
        # clean-checkout preflight (#4690).
        (self.root / ".gitignore").write_text(
            "target/\n__pycache__/\n.zcode/\n", encoding="utf-8"
        )
        self.git("add", ".gitignore")
        self.git("commit", "--quiet", "-m", "ignore agent tooling state")
        tooling = self.root / ".zcode/workflow-runs"
        tooling.mkdir(parents=True)
        (tooling / "state.json").write_text("{}\n", encoding="utf-8")

        receipt = REHEARSAL.build_rehearsal_receipt("HEAD")

        self.assertEqual(receipt["aggregate_status"], "Incomplete")
        self.assertTrue(
            all(value is False for value in receipt["zero_mutation_proof"].values())
        )

    def test_ignored_artifact_classifier_admits_tooling_state_only_at_root(self) -> None:
        self.assertTrue(REHEARSAL._is_ignored_rehearsal_artifact(b"!! .zcode/"))
        for entry in (
            b"!! .zcode-lookalike/",
            b"!! .zcode/nested/",
            b"!! src/.zcode/",
            b"?? .zcode/",
        ):
            with self.subTest(entry=entry):
                self.assertFalse(REHEARSAL._is_ignored_rehearsal_artifact(entry))

    def test_staged_rename_is_rejected_before_phases(self) -> None:
        self.git("mv", "Cargo.lock", "renamed lock")
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")
        self.require_no_phases()

    def test_status_inspection_failure_is_rejected_without_raw_output(self) -> None:
        run = subprocess.run

        def fail_status(command, **kwargs):
            if "status" in command:
                return subprocess.CompletedProcess(command, 128, b"", b"secret-canary")
            return run(command, **kwargs)

        with mock.patch.object(REHEARSAL.subprocess, "run", side_effect=fail_status):
            with self.assertRaisesRegex(ValueError, "could not inspect") as caught:
                REHEARSAL.build_rehearsal_receipt("HEAD")
        self.assertNotIn("secret-canary", str(caught.exception))
        self.require_no_phases()

    def test_index_inspection_failures_reach_cli_without_a_receipt(self) -> None:
        run = subprocess.run
        output = self.root / "target/rejected.json"
        for failure in (
            subprocess.CompletedProcess([], 128, b"secret-canary", b"secret-canary"),
            OSError("fixture Git unavailable"),
            subprocess.TimeoutExpired("git ls-files", 15),
        ):
            with self.subTest(failure=type(failure).__name__):
                def fail_index(command, **kwargs):
                    if "ls-files" in command:
                        if isinstance(failure, Exception):
                            raise failure
                        return failure
                    return run(command, **kwargs)

                stderr = io.StringIO()
                stdout = io.StringIO()
                with mock.patch.object(REHEARSAL.subprocess, "run", side_effect=fail_index), \
                     mock.patch.object(sys, "argv", [
                         str(REHEARSAL_PATH), "--commit", "HEAD", "--output", str(output),
                     ]), contextlib.redirect_stderr(stderr), contextlib.redirect_stdout(stdout):
                    self.assertEqual(REHEARSAL.main(), 2)
                self.assertFalse(output.exists())
                self.assertEqual(stdout.getvalue(), "")
                self.assertIn("instrumentation failed", stderr.getvalue())
                self.assertNotIn("secret-canary", stderr.getvalue())
                self.require_no_phases()

    def test_head_movement_during_admission_stops_before_phases(self) -> None:
        run = subprocess.run

        def move_head_after_status(command, **kwargs):
            result = run(command, **kwargs)
            if "status" in command:
                self.git("commit", "--quiet", "--allow-empty", "-m", "moved during check")
            return result

        with mock.patch.object(REHEARSAL.subprocess, "run", side_effect=move_head_after_status):
            with self.assertRaisesRegex(ValueError, "HEAD moved"):
                REHEARSAL.build_rehearsal_receipt("HEAD")
        self.require_no_phases()

    def test_dirty_source_after_phases_cannot_return_a_receipt(self) -> None:
        def change_source(receipt, *, candidate_executable=None, candidate_sha256=None):
            (self.root / "Cargo.lock").write_text("changed by phase\n", encoding="utf-8")
            return "Incomplete"

        self.phases[0].side_effect = change_source
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")

    def test_hidden_source_after_phases_cannot_return_a_receipt(self) -> None:
        source = self.root / "Cargo.lock"
        original = source.read_bytes()
        for flag in ("assume-unchanged", "skip-worktree"):
            with self.subTest(flag=flag):
                def change_source(receipt, *, candidate_executable=None, candidate_sha256=None):
                    self.git("update-index", f"--{flag}", "Cargo.lock")
                    source.write_bytes(original + b"# hidden phase change\n")
                    return "Incomplete"

                self.phases[0].side_effect = change_source
                try:
                    with self.assertRaisesRegex(ValueError, "index flags"):
                        REHEARSAL.build_rehearsal_receipt("HEAD")
                finally:
                    source.write_bytes(original)
                    self.git("update-index", f"--no-{flag}", "Cargo.lock")

    def test_untracked_source_after_phases_cannot_return_a_receipt(self) -> None:
        def change_source(receipt, *, candidate_executable=None, candidate_sha256=None):
            self.add_untracked_source()
            return "Incomplete"

        self.phases[0].side_effect = change_source
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")

    def test_staged_source_after_phases_cannot_return_a_receipt(self) -> None:
        def change_source(receipt, *, candidate_executable=None, candidate_sha256=None):
            (self.root / "Cargo.lock").write_text("staged by phase\n", encoding="utf-8")
            self.git("add", "Cargo.lock")
            return "Incomplete"

        self.phases[0].side_effect = change_source
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")

    def test_clean_commit_movement_after_phases_cannot_return_a_receipt(self) -> None:
        def change_head(receipt, *, candidate_executable=None, candidate_sha256=None):
            self.git("commit", "--quiet", "--allow-empty", "-m", "moved subject")
            return "Incomplete"

        self.phases[0].side_effect = change_head
        with self.assertRaises(ValueError):
            REHEARSAL.build_rehearsal_receipt("HEAD")

    def test_rejected_subject_cli_writes_no_receipt(self) -> None:
        output = self.root / "target/rejected.json"
        stderr = io.StringIO()
        stdout = io.StringIO()
        with mock.patch.object(sys, "argv", [
            str(REHEARSAL_PATH), "--commit", self.previous, "--output", str(output),
        ]), contextlib.redirect_stderr(stderr), contextlib.redirect_stdout(stdout):
            self.assertEqual(REHEARSAL.main(), 2)
        self.assertFalse(output.exists())
        self.assertEqual(stdout.getvalue(), "")
        self.assertIn("instrumentation failed", stderr.getvalue())
        self.require_no_phases()



class TestInvocationBoundary(unittest.TestCase):
    """Actual owned adapters, with harmless child canaries and separate bypasses."""

    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory(prefix="rehearsal-invocation-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name).resolve()
        self.canary = self.root / "forbidden-child.txt"
        self.real_run = subprocess.run
        self.environment = {"PATH": str(Path(sys.executable).parent), "BENIGN": "retained"}
        if "SYSTEMROOT" in os.environ:
            self.environment["SYSTEMROOT"] = os.environ["SYSTEMROOT"]
        self.stack = contextlib.ExitStack()
        self.addCleanup(self.stack.close)
        self.stack.enter_context(mock.patch.object(REHEARSAL, "ROOT", self.root))
        self.stack.enter_context(mock.patch.object(
            REHEARSAL, "os", SimpleNamespace(environ=self.environment),
        ))

    def canary_command(self) -> list[str]:
        return [
            sys.executable, "-c",
            "from pathlib import Path; import sys; Path(sys.argv[1]).write_text('attempted', encoding='utf-8')",
            str(self.canary),
        ]

    @staticmethod
    def bypass(command, *, operation, selected=None, **options):
        # Deliberate mutant: remove only the owned invocation check.
        return REHEARSAL.subprocess.run(command, **options)

    def test_generic_characterization_has_no_arbitrary_child_permission(self) -> None:
        with mock.patch.object(REHEARSAL.subprocess, "run", wraps=self.real_run) as terminal:
            status = REHEARSAL._run_characterization(self.canary_command())
            self.assertEqual(status, "InstrumentFailure", "arbitrary characterization reached a child")
            terminal.assert_not_called()
            self.assertFalse(self.canary.exists())
            with mock.patch.object(REHEARSAL, "_run_process", side_effect=self.bypass):
                self.assertEqual(REHEARSAL._run_characterization(self.canary_command()), "Incomplete")
            terminal.assert_called_once()
            self.assertEqual(self.canary.read_text(encoding="utf-8"), "attempted")

    def test_real_proof_route_denies_replaced_argv_and_its_bypass_is_detected(self) -> None:
        proof = REHEARSAL._run_proof

        def changed(command, **selection):
            return proof(self.canary_command(), **selection)

        def require_denial() -> None:
            self.assertEqual(
                REHEARSAL.run_phase_publisher_state_machine({}), "InstrumentFailure",
                "publisher proof reached a forbidden child",
            )
            self.assertFalse(self.canary.exists(), "publisher proof changed the canary")

        with mock.patch.object(REHEARSAL, "_run_proof", side_effect=changed), \
                mock.patch.object(REHEARSAL.subprocess, "run", wraps=self.real_run) as terminal:
            require_denial()
            terminal.assert_not_called()
            with mock.patch.object(REHEARSAL, "_run_process", side_effect=self.bypass):
                with self.assertRaisesRegex(AssertionError, "publisher proof reached"):
                    require_denial()
            terminal.assert_called_once()
            self.assertEqual(self.canary.read_text(encoding="utf-8"), "attempted")

    def test_history_phase_requires_its_check_form(self) -> None:
        script = self.root / "scripts/generate-changie-history.py"
        script.parent.mkdir()
        script.write_text(
            "from pathlib import Path\nimport sys\n"
            "if sys.argv[1:] != ['--check']:\n"
            "    Path(__file__).resolve().parents[1].joinpath('forbidden-child.txt').write_text('attempted')\n",
            encoding="utf-8",
        )
        with mock.patch.object(REHEARSAL, "_select_tool", return_value=None), \
                mock.patch.object(REHEARSAL.subprocess, "run", wraps=self.real_run) as terminal:
            # The actual history child succeeds; missing identity remains its
            # existing downstream Incomplete result, not a fabricated Complete.
            self.assertEqual(REHEARSAL.run_phase_docs_and_support({}), "Incomplete")
            terminal.assert_called_once()
            self.assertEqual(terminal.call_args.args[0][-1], "--check")
            self.assertEqual(terminal.call_args.kwargs["executable"], str(Path(sys.executable).absolute()))
            self.assertFalse(self.canary.exists())
            terminal.reset_mock()
            proof = REHEARSAL._run_proof

            def writing_form(command, **selection):
                return proof(command[:-1], **selection)

            with mock.patch.object(REHEARSAL, "_run_proof", side_effect=writing_form):
                self.assertEqual(REHEARSAL.run_phase_docs_and_support({}), "InstrumentFailure")
            terminal.assert_not_called()
            self.assertFalse(self.canary.exists())

    def test_full_git_argv_root_and_process_options_are_closed(self) -> None:
        command = ["git", "--no-replace-objects", "rev-parse", "--verify", "HEAD^{commit}"]
        options = dict(cwd=self.root, env=self.environment, capture_output=True,
                       text=True, timeout=15, check=False)
        cases = [
            (["git", "tag", "v-fixture"], {}),
            (["git", "update-ref", "refs/tags/fixture", "a" * 40], {}),
            (["git", "push", "fixture"], {}),
            ([sys.executable, *command[1:]], {}),
            ([*command, "--quiet"], {}),
            (command, {"cwd": self.root.parent}),
            (command, {"shell": True}),
            (command, {"executable": sys.executable}),
            (command, {"timeout": 301}),
            (command, {"check": True}),
            (command, {"input": b"unselected"}),
            (command, {"env": None}),
        ]
        with mock.patch.object(REHEARSAL, "_select_tool", return_value=sys.executable), \
                REHEARSAL._invocation_scope(), \
                mock.patch.object(REHEARSAL.subprocess, "run") as terminal:
            for argv, extra in cases:
                with self.subTest(argv=argv, extra=tuple(extra)):
                    with self.assertRaisesRegex(OSError, "not selected"):
                        REHEARSAL._run_process(
                            argv, operation="resolve-commit", selected="HEAD", **(options | extra),
                        )
                    terminal.assert_not_called()
            with self.assertRaisesRegex(OSError, "not selected"):
                REHEARSAL._run_process(command, operation="unselected", **options)
            terminal.assert_not_called()

    def test_captured_selection_survives_path_movement_and_scope_is_restored(self) -> None:
        with mock.patch.object(REHEARSAL, "_select_tool", return_value=sys.executable) as lookup, \
                mock.patch.object(REHEARSAL.subprocess, "run", return_value=
                                  subprocess.CompletedProcess([], 0, "a" * 40, "")) as terminal:
            with REHEARSAL._invocation_scope() as selected:
                original_root = selected.root
                self.assertEqual(lookup.call_count, 2)
                self.environment["PATH"] = str(self.root / "unselected-tools")
                self.assertEqual(REHEARSAL.resolve_commit("HEAD"), "a" * 40)
                self.assertEqual(lookup.call_count, 2)
                self.assertEqual(terminal.call_args.kwargs["executable"], sys.executable)
                self.assertEqual(terminal.call_args.kwargs["cwd"], original_root)
                self.assertEqual(terminal.call_args.args[0][0], "git")
                terminal.reset_mock()
                with mock.patch.object(REHEARSAL, "ROOT", self.root / "other"):
                    self.assertEqual(REHEARSAL.run_phase_publisher_state_machine({}), "InstrumentFailure")
                terminal.assert_not_called()
            self.assertIsNone(REHEARSAL._INVOCATION.get())
            with REHEARSAL._invocation_scope():
                self.assertEqual(lookup.call_count, 4)
            self.assertIsNone(REHEARSAL._INVOCATION.get())

    def test_verified_candidate_does_not_authorize_another_executable(self) -> None:
        candidate = self.root / ("candidate.exe" if os.name == "nt" else "candidate")
        candidate.write_bytes(b"selected candidate bytes")
        digest = REHEARSAL.compute_sha256(candidate)
        invoke = REHEARSAL._run_process

        def substituted(command, **selection):
            return invoke([sys.executable, *command[1:]], **selection)

        with mock.patch.object(REHEARSAL, "_workspace_version", return_value="0.2.0"), \
                mock.patch.object(REHEARSAL, "_run_process", side_effect=substituted), \
                mock.patch.object(REHEARSAL.subprocess, "run") as terminal, \
                contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(REHEARSAL.run_phase_release_identity(
                {}, candidate_executable=candidate, candidate_sha256=digest,
            ), "InstrumentFailure")
            terminal.assert_not_called()
        self.assertEqual(REHEARSAL.compute_sha256(candidate), digest)


    def test_direct_helpers_and_nested_scopes_restore_selection_after_failure(self) -> None:
        observed = []

        def terminal(command, **options):
            observed.append(REHEARSAL._INVOCATION.get())
            if len(observed) == 1:
                raise OSError("fixture terminal failure")
            self.assertEqual(command[0], "git")
            self.assertEqual(options["executable"], sys.executable)
            return subprocess.CompletedProcess(command, 0, "a" * 40, "")

        with mock.patch.object(REHEARSAL, "_select_tool", return_value=sys.executable) as lookup, \
                mock.patch.object(REHEARSAL.subprocess, "run", side_effect=terminal):
            self.assertIsNone(REHEARSAL._INVOCATION.get())
            with self.assertRaisesRegex(OSError, "fixture terminal failure"):
                REHEARSAL.resolve_commit("HEAD")
            self.assertIsNone(REHEARSAL._INVOCATION.get())
            self.assertEqual(lookup.call_count, 2)
            with mock.patch.object(REHEARSAL, "ROOT", self.root / "later-root"):
                self.assertEqual(REHEARSAL.resolve_commit("HEAD"), "a" * 40)
            self.assertIsNone(REHEARSAL._INVOCATION.get())
            self.assertEqual(lookup.call_count, 4)
            self.assertIsNot(observed[0], observed[1])
            self.assertEqual(observed[0].root, self.root)
            self.assertEqual(observed[1].root, self.root / "later-root")
            with REHEARSAL._invocation_scope() as outer:
                with self.assertRaisesRegex(OSError, "nested failure"):
                    with REHEARSAL._invocation_scope() as inner:
                        self.assertIs(inner, outer)
                        raise OSError("nested failure")
                self.assertIs(REHEARSAL._INVOCATION.get(), outer)
                self.assertEqual(REHEARSAL.resolve_commit("HEAD"), "a" * 40)
                self.assertIs(observed[-1], outer)
            self.assertIsNone(REHEARSAL._INVOCATION.get())
            self.assertEqual(lookup.call_count, 6)

    def test_public_entries_restore_context_after_nested_builder_failure(self) -> None:
        observed = []

        def failed_builder(*args, **kwargs):
            selected = REHEARSAL._INVOCATION.get()
            self.assertIsNotNone(selected)
            observed.append(selected)
            with REHEARSAL._invocation_scope() as nested:
                self.assertIs(nested, selected)
            raise OSError("fixture builder failure")

        with mock.patch.object(REHEARSAL, "_select_tool", return_value=sys.executable) as lookup, \
                mock.patch.object(REHEARSAL, "_build_rehearsal_receipt", side_effect=failed_builder), \
                mock.patch.object(REHEARSAL, "_write_receipt") as write, \
                mock.patch.object(sys, "argv", ["release-rehearsal.py", "--commit", "HEAD"]), \
                contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(REHEARSAL.main(), 2)
            self.assertIsNone(REHEARSAL._INVOCATION.get())
            self.assertEqual(lookup.call_count, 2)
            with self.assertRaisesRegex(OSError, "fixture builder failure"):
                REHEARSAL.build_rehearsal_receipt("HEAD")
            self.assertIsNone(REHEARSAL._INVOCATION.get())
            self.assertEqual(lookup.call_count, 4)
            self.assertIsNot(observed[0], observed[1])
            write.assert_not_called()

    def test_missing_and_unsupported_executables_fail_at_the_terminal_boundary(self) -> None:
        values = [None, "", 7, "git", str(self.root / "invalid\nexecutable")]
        for value in values:
            with self.subTest(executable=value), \
                    mock.patch.object(REHEARSAL, "_select_tool", return_value=value), \
                    mock.patch.object(REHEARSAL.subprocess, "run") as terminal:
                # Capturing an unavailable unused tool is not itself fatal.
                with REHEARSAL._invocation_scope():
                    with self.assertRaisesRegex(OSError, "executable"):
                        REHEARSAL.resolve_commit("HEAD")
                terminal.assert_not_called()
                self.assertIsNone(REHEARSAL._INVOCATION.get())
        with mock.patch.object(REHEARSAL, "_PLATFORM_NAME", "nt"), \
                mock.patch.object(REHEARSAL, "_select_tool", return_value=str(self.root / "git.cmd")), \
                mock.patch.object(REHEARSAL.subprocess, "run") as terminal:
            with self.assertRaisesRegex(OSError, "executable form"):
                REHEARSAL.resolve_commit("HEAD")
            terminal.assert_not_called()
            self.assertIsNone(REHEARSAL._INVOCATION.get())

    def test_tag_mutation_invocation_boundary_and_bypass_falsifier(self) -> None:
        with mock.patch.object(REHEARSAL.subprocess, "run") as terminal:
            denied, commands = REHEARSAL._check_tag_invocation_boundary("a" * 40)
            self.assertTrue(denied)
            self.assertEqual(commands, ["git_tag", "git_update_ref", "git_push"])
            terminal.assert_not_called()

        def bypassed(command, *, operation, selected=None, **options):
            return subprocess.CompletedProcess(command, 0, "", "")

        with mock.patch.object(REHEARSAL, "_run_process", side_effect=bypassed):
            denied, commands = REHEARSAL._check_tag_invocation_boundary("a" * 40)
            self.assertFalse(denied)
            self.assertEqual(commands, [])

    def test_unconfined_tag_mutation_proof_stays_false_without_mutating_repo(self) -> None:
        with mock.patch.object(REHEARSAL.subprocess, "run") as terminal:
            prevented, evidence = REHEARSAL._evaluate_tag_mutation_proof("a" * 40)
        self.assertFalse(prevented)
        self.assertTrue(evidence["argv_denied"])
        self.assertFalse(evidence["runtime_confined"])
        self.assertFalse(evidence["prevented"])
        self.assertIn(
            evidence["reason"],
            {"unsupported_platform", "unconfined_runtime_boundary"},
        )
        terminal.assert_not_called()
        self.assertEqual(list(self.root.iterdir()), [])

    def test_linux_worker_launcher_refuses_selected_token_and_unavailable_host(self) -> None:
        trap = TokenValueTrap({"CARGO_REGISTRY_TOKEN": "synthetic-private-token"})
        with mock.patch.object(REHEARSAL.os, "environ", trap):
            with self.assertRaisesRegex(ValueError, "CARGO_REGISTRY_TOKEN"):
                REHEARSAL.run_in_linux_worker("HEAD")
        self.assertNotIn("CARGO_REGISTRY_TOKEN", trap.value_reads)

        with mock.patch.object(
            REHEARSAL, "linux_worker_capability",
            return_value=(False, "unshare_or_setpriv_unavailable"),
        ), mock.patch.object(REHEARSAL.subprocess, "run") as terminal:
            with self.assertRaisesRegex(OSError, "unshare_or_setpriv_unavailable"):
                REHEARSAL.run_in_linux_worker("HEAD")
            terminal.assert_not_called()

        with self.assertRaisesRegex(ValueError, "timeout"):
            REHEARSAL.run_in_linux_worker("HEAD", timeout=0)

        dirty_env = {
            "PATH": "/usr/bin",
            "CARGO_REGISTRY_TOKEN": "secret",
            "LD_PRELOAD": "/tmp/hook.so",
            "LD_LIBRARY_PATH": "/tmp/libs",
            "PYTHONPATH": "/tmp/py",
            "PYTHONSTARTUP": "/tmp/startup.py",
            "GIT_OBJECT_DIRECTORY": "/tmp/evil-obj",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES": "/tmp/evil-alt",
            "GIT_EXEC_PATH": "/tmp/evil-exec",
            "GIT_SSH": "/tmp/evil-ssh",
            "GIT_TEMPLATE_DIR": "/tmp/evil-tpl",
            "GIT_PROXY_COMMAND": "/tmp/evil-proxy",
            "GIT_EXTERNAL_DIFF": "/tmp/evil-diff",
            "GIT_CONFIG_NOSYSTEM": "1",
        }
        with mock.patch.object(REHEARSAL.os, "environ", dirty_env):
            cleaned = REHEARSAL._linux_worker_env(
                empty_home=self.root / "h",
                empty_xdg=self.root / "x",
                work_tmp=self.root / "t",
            )
        for forbidden in (
            "CARGO_REGISTRY_TOKEN",
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "PYTHONPATH",
            "PYTHONSTARTUP",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_EXEC_PATH",
            "GIT_SSH",
            "GIT_TEMPLATE_DIR",
            "GIT_PROXY_COMMAND",
            "GIT_EXTERNAL_DIFF",
            "GIT_CONFIG_NOSYSTEM",
        ):
            self.assertNotIn(forbidden, cleaned)
        self.assertEqual(cleaned["TMPDIR"], str(self.root / "t"))
        self.assertEqual(cleaned["TEMP"], str(self.root / "t"))
        self.assertEqual(cleaned["TMP"], str(self.root / "t"))
        self.assertEqual(cleaned["GIT_TERMINAL_PROMPT"], "0")
        self.assertEqual(cleaned["GIT_SSH_COMMAND"], "/bin/false")
        self.assertEqual(cleaned["GIT_ASKPASS"], "/bin/false")


class TestLinuxTagConfinement(unittest.TestCase):
    """End-to-end Linux worker profile and tag-mutation denial falsifiers (#3792)."""

    UNPROVEN_SIX = (
        "token_read_prevented",
        "cargo_publish_prevented",
        "registry_mutation_prevented",
        "github_release_mutation_prevented",
        "live_setting_mutation_prevented",
        "external_repository_mutation_prevented",
    )

    @classmethod
    def setUpClass(cls) -> None:
        capable, reason = REHEARSAL.linux_worker_capability()
        if not capable:
            raise unittest.SkipTest(f"Linux worker confinement unavailable on host: {reason}")

    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory(prefix="rehearsal-linux-worker-")
        self.addCleanup(directory.cleanup)
        self.sandbox = Path(directory.name).resolve()
        self.repo = self.sandbox / "repo"
        self.repo.mkdir()
        self.git(self.repo, "init", "--quiet")
        files = {
            ".gitignore": "target/\n__pycache__/\n",
            "Cargo.lock": "version = 4\n",
            "policy/product-package-topology-v2.toml": "schema_version = '2.0'\n",
            ".github/workflows/release.yml": "name: fixture\n",
            "scripts/release-rehearsal.py": REHEARSAL_PATH.read_text(encoding="utf-8"),
        }
        for rel, content in files.items():
            path = self.repo / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
        self.git(self.repo, "add", ".")
        self.git(self.repo, "commit", "--quiet", "-m", "fixture subject")
        self.head = self.git(self.repo, "rev-parse", "HEAD").strip()
        self.script = self.repo / "scripts/release-rehearsal.py"

    @staticmethod
    def git(cwd: Path, *args: str, check: bool = True) -> str:
        result = subprocess.run(
            [
                "git", "-c", "core.autocrlf=false", "-c", "commit.gpgsign=false",
                "-c", f"core.hooksPath={cwd / 'no-hooks'}",
                "-c", "user.name=Rehearsal fixture",
                "-c", "user.email=rehearsal@example.invalid",
                *args,
            ],
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=15,
            check=check,
        )
        return result.stdout

    def init_bare_remote(self, name: str = "canary-remote.git") -> Path:
        remote = self.sandbox / name
        subprocess.run(
            ["git", "init", "--bare", "--quiet", str(remote)],
            capture_output=True,
            text=True,
            timeout=15,
            check=True,
        )
        return remote

    def show_tags(self, path: Path, *, bare: bool = False) -> str:
        cmd = ["git", "--no-replace-objects"]
        if bare:
            cmd.extend(["--git-dir", str(path)])
        cmd.extend(["show-ref", "--tags"])
        res = subprocess.run(
            cmd,
            cwd=None if bare else path,
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
        return res.stdout

    def test_linux_worker_produces_first_true_flag_and_leaves_six_false(self) -> None:
        target_receipt = self.repo / "target/cargo-allow/worker-receipt.json"
        external_receipt = self.sandbox / "external-worker-receipt.json"
        for output_path in (target_receipt, external_receipt):
            with self.subTest(output=output_path.name):
                launched = REHEARSAL.run_in_linux_worker(
                    "HEAD",
                    output=output_path,
                    repo_root=self.repo,
                    script_path=self.script,
                    capture_output=True,
                )
                self.assertEqual(
                    launched.returncode, 1,
                    f"expected non-Complete exit 1, got {launched.returncode}: {launched.stderr}",
                )
                receipt = json.loads(output_path.read_text(encoding="utf-8"))
                self.assertEqual(receipt["commit_sha"], self.head)
                self.assertNotEqual(receipt["aggregate_status"], "Complete")
                self.assertIn("cannot satisfy a release gate", receipt["claim_boundary"])
                self.assertIn(REHEARSAL.LINUX_TAG_WORKER_PROFILE, receipt["claim_boundary"])

                proof = receipt["zero_mutation_proof"]
                self.assertEqual(len(proof), 7)
                self.assertIs(proof["tag_mutation_prevented"], True)
                for flag in self.UNPROVEN_SIX:
                    self.assertIs(proof[flag], False, f"{flag} must remain unproven False")

                evidence = receipt["zero_mutation_evidence"]["tag_mutation_prevented"]
                self.assertEqual(evidence["worker_profile"], REHEARSAL.LINUX_TAG_WORKER_PROFILE)
                self.assertEqual(evidence["reason"], "confined_tag_mutation_denied")
                for key in (
                    "argv_denied",
                    "runtime_confined",
                    "all_mounts_ro",
                    "capabilities_dropped",
                    "remount_denied_eperm",
                    "network_egress_denied",
                    "direct_fs_write_denied_erofs",
                    "direct_git_tag_denied",
                    "direct_git_update_ref_denied",
                    "direct_git_push_denied",
                    "tags_unchanged",
                    "canary_remote_tags_unchanged",
                    "prevented",
                ):
                    self.assertIs(evidence[key], True, f"expected {key}=True, got {evidence}")
                self.assertTrue(all(err == "EPERM" for err in evidence["remount_errnos"]))
                self.assertEqual(evidence["network_probe_errno"], "ENETUNREACH")
                self.assertTrue(all(err == "EROFS" for err in evidence["direct_write_errnos"].values()))
                self.assertEqual(self.show_tags(self.repo), "")

    def test_linked_worktree_confinement_protects_external_common_dir(self) -> None:
        worktree = self.sandbox / "linked-wt"
        self.git(self.repo, "worktree", "add", "--quiet", "--detach", str(worktree), self.head)
        self.addCleanup(self.git, self.repo, "worktree", "remove", "--force", str(worktree), check=False)
        wt_script = worktree / "scripts/release-rehearsal.py"
        output_path = self.sandbox / "linked-wt-receipt.json"

        launched = REHEARSAL.run_in_linux_worker(
            "HEAD",
            output=output_path,
            repo_root=worktree,
            script_path=wt_script,
            capture_output=True,
        )
        self.assertEqual(launched.returncode, 1, launched.stderr)
        receipt = json.loads(output_path.read_text(encoding="utf-8"))
        self.assertIs(receipt["zero_mutation_proof"]["tag_mutation_prevented"], True)
        for flag in self.UNPROVEN_SIX:
            self.assertIs(receipt["zero_mutation_proof"][flag], False)

        # Falsifier: mount only the worktree RO while leaving external git_common_dir RW.
        remote = self.init_bare_remote("wt-partial-remote.git")
        partial_out = self.sandbox / "wt-partial-receipt.json"
        sup = self.sandbox / "partial-wt.sh"
        sup.write_text(
            "#!/usr/bin/env bash\n"
            "set -euo pipefail\n"
            'wt="$1"; remote="$2"; shift 2\n'
            "mount --make-rprivate / 2>/dev/null || true\n"
            'mount --bind "$wt" "$wt"\n'
            'mount -o remount,bind,ro,nosuid,nodev "$wt"\n'
            'mount --bind "$wt/target" "$wt/target"\n'
            'mount -o remount,bind,rw "$wt/target"\n'
            'mount --bind "$remote" "$remote"\n'
            'mount -o remount,bind,ro,nosuid,nodev "$remote"\n'
            'cd "$wt"\n'
            'exec setpriv --bounding-set=-all --inh-caps=-all --ambient-caps=-all --no-new-privs -- "$@"\n',
            encoding="utf-8",
        )
        partial = subprocess.run(
            [
                "unshare", "-Urmn", "--", "bash", str(sup), str(worktree), str(remote),
                sys.executable, str(wt_script), "--commit", "HEAD",
                "--tag-canary-remote", str(remote), "--output", str(partial_out),
            ],
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
        self.assertEqual(partial.returncode, 1, partial.stderr)
        partial_receipt = json.loads(partial_out.read_text(encoding="utf-8"))
        self.assertIs(partial_receipt["zero_mutation_proof"]["tag_mutation_prevented"], False)
        self.assertEqual(
            partial_receipt["zero_mutation_evidence"]["tag_mutation_prevented"]["reason"],
            "git_metadata_not_read_only",
        )

    def test_falsifier_removing_ro_mount_allows_tag_mutation_and_flips_flag_false(self) -> None:
        remote = self.init_bare_remote("unconfined-remote.git")
        output_path = self.sandbox / "unconfined-receipt.json"
        unconfined = subprocess.run(
            [
                sys.executable, str(self.script), "--commit", "HEAD",
                "--tag-canary-remote", str(remote), "--output", str(output_path),
            ],
            cwd=self.repo,
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
        self.assertEqual(unconfined.returncode, 1, unconfined.stderr)
        receipt = json.loads(output_path.read_text(encoding="utf-8"))
        self.assertIs(receipt["zero_mutation_proof"]["tag_mutation_prevented"], False)
        self.assertEqual(
            receipt["zero_mutation_evidence"]["tag_mutation_prevented"]["reason"],
            "unconfined_runtime_boundary",
        )
        self.assertEqual(self.show_tags(self.repo), "")
        self.assertEqual(self.show_tags(remote, bare=True), "")

        # Prove the forbidden effects actually succeed when the boundary is removed.
        self.git(self.repo, "tag", "falsifier-local-tag", "HEAD")
        self.assertIn("refs/tags/falsifier-local-tag", self.show_tags(self.repo))
        self.git(self.repo, "push", str(remote), "HEAD:refs/tags/falsifier-pushed-tag")
        self.assertIn("refs/tags/falsifier-pushed-tag", self.show_tags(remote, bare=True))

    def test_falsifier_omitting_setpriv_cap_drop_flips_flag_false(self) -> None:
        remote = self.init_bare_remote("nocapdrop-remote.git")
        output_path = self.sandbox / "nocapdrop-receipt.json"
        (self.repo / "target").mkdir(exist_ok=True)
        sup = self.sandbox / "nocapdrop.sh"
        sup.write_text(
            "#!/usr/bin/env bash\n"
            "set -euo pipefail\n"
            'repo="$1"; remote="$2"; shift 2\n'
            "mount --make-rprivate / 2>/dev/null || true\n"
            'mount --bind "$repo" "$repo"\n'
            'mount -o remount,bind,ro,nosuid,nodev "$repo"\n'
            'mount --bind "$repo/target" "$repo/target"\n'
            'mount -o remount,bind,rw "$repo/target"\n'
            'mount --bind "$remote" "$remote"\n'
            'mount -o remount,bind,ro,nosuid,nodev "$remote"\n'
            'cd "$repo"\n'
            'exec "$@"\n',
            encoding="utf-8",
        )
        run = subprocess.run(
            [
                "unshare", "-Urmn", "--", "bash", str(sup), str(self.repo), str(remote),
                sys.executable, str(self.script), "--commit", "HEAD",
                "--tag-canary-remote", str(remote), "--output", str(output_path),
            ],
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
        self.assertEqual(run.returncode, 1, run.stderr)
        receipt = json.loads(output_path.read_text(encoding="utf-8"))
        self.assertIs(receipt["zero_mutation_proof"]["tag_mutation_prevented"], False)
        evidence = receipt["zero_mutation_evidence"]["tag_mutation_prevented"]
        self.assertIs(evidence["capabilities_dropped"], False)
        self.assertEqual(evidence["reason"], "privileges_not_dropped")

        # Also prove libc.mount remount succeeds when CAP_SYS_ADMIN is kept.
        probe = subprocess.run(
            [
                "unshare", "-Urmn", "--", "bash", str(sup), str(self.repo), str(remote),
                sys.executable, "-c",
                "import importlib.util, sys; from pathlib import Path; "
                "s = importlib.util.spec_from_file_location('r', sys.argv[1]); "
                "m = importlib.util.module_from_spec(s); s.loader.exec_module(m); "
                "ok, reason = m._probe_remount_rw_eperm(Path(sys.argv[2])); "
                "print(f'{ok}:{reason}')",
                str(self.script), str(self.repo),
            ],
            capture_output=True,
            text=True,
            timeout=30,
            check=True,
        )
        self.assertEqual(probe.stdout.strip(), "False:remount_succeeded")

    def test_falsifier_omitting_network_namespace_flips_flag_false(self) -> None:
        # Skip only if the ambient test host itself has no default route.
        ambient_denied, _ = REHEARSAL._probe_network_egress_denied()
        if ambient_denied:
            self.skipTest("host itself has no default IPv4 route to distinguish -n omission")
        remote = self.init_bare_remote("nonetns-remote.git")
        output_path = self.sandbox / "nonetns-receipt.json"
        (self.repo / "target").mkdir(exist_ok=True)
        sup = self.sandbox / "nonetns.sh"
        sup.write_text(
            "#!/usr/bin/env bash\n"
            "set -euo pipefail\n"
            'repo="$1"; remote="$2"; shift 2\n'
            "mount --make-rprivate / 2>/dev/null || true\n"
            'mount --bind "$repo" "$repo"\n'
            'mount -o remount,bind,ro,nosuid,nodev "$repo"\n'
            'mount --bind "$repo/target" "$repo/target"\n'
            'mount -o remount,bind,rw "$repo/target"\n'
            'mount --bind "$remote" "$remote"\n'
            'mount -o remount,bind,ro,nosuid,nodev "$remote"\n'
            'cd "$repo"\n'
            'exec setpriv --bounding-set=-all --inh-caps=-all --ambient-caps=-all --no-new-privs -- "$@"\n',
            encoding="utf-8",
        )
        run = subprocess.run(
            [
                "unshare", "-Urm", "--", "bash", str(sup), str(self.repo), str(remote),
                sys.executable, str(self.script), "--commit", "HEAD",
                "--tag-canary-remote", str(remote), "--output", str(output_path),
            ],
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
        self.assertEqual(run.returncode, 1, run.stderr)
        receipt = json.loads(output_path.read_text(encoding="utf-8"))
        self.assertIs(receipt["zero_mutation_proof"]["tag_mutation_prevented"], False)
        evidence = receipt["zero_mutation_evidence"]["tag_mutation_prevented"]
        self.assertIs(evidence["network_egress_denied"], False)
        self.assertEqual(evidence["reason"], "network_not_isolated")

    def test_falsifier_writable_canary_remote_allows_push_and_flips_flag_false(self) -> None:
        remote = self.init_bare_remote("writable-remote.git")
        output_path = self.sandbox / "writable-remote-receipt.json"
        (self.repo / "target").mkdir(exist_ok=True)
        sup = self.sandbox / "writable-remote.sh"
        sup.write_text(
            "#!/usr/bin/env bash\n"
            "set -euo pipefail\n"
            'repo="$1"; shift 1\n'
            "mount --make-rprivate / 2>/dev/null || true\n"
            'mount --bind "$repo" "$repo"\n'
            'mount -o remount,bind,ro,nosuid,nodev "$repo"\n'
            'mount --bind "$repo/target" "$repo/target"\n'
            'mount -o remount,bind,rw "$repo/target"\n'
            'cd "$repo"\n'
            'exec setpriv --bounding-set=-all --inh-caps=-all --ambient-caps=-all --no-new-privs -- "$@"\n',
            encoding="utf-8",
        )
        run = subprocess.run(
            [
                "unshare", "-Urmn", "--", "bash", str(sup), str(self.repo),
                sys.executable, str(self.script), "--commit", "HEAD",
                "--tag-canary-remote", str(remote), "--output", str(output_path),
            ],
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
        self.assertEqual(run.returncode, 1, run.stderr)
        receipt = json.loads(output_path.read_text(encoding="utf-8"))
        self.assertIs(receipt["zero_mutation_proof"]["tag_mutation_prevented"], False)
        self.assertEqual(
            receipt["zero_mutation_evidence"]["tag_mutation_prevented"]["reason"],
            "canary_remote_invalid_or_writable",
        )
        # Prove a direct git push to that unconfined remote succeeds even with repo RO.
        push = subprocess.run(
            [
                "unshare", "-Urmn", "--", "bash", str(sup), str(self.repo),
                "git", "--no-replace-objects", "push", str(remote),
                "HEAD:refs/tags/falsifier-writable-remote",
            ],
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
        self.assertEqual(push.returncode, 0, push.stderr)
        self.assertIn("refs/tags/falsifier-writable-remote", self.show_tags(remote, bare=True))

    def test_falsifier_bypassing_invocation_boundary_flips_flag_false_even_when_confined(self) -> None:
        remote = self.init_bare_remote("bypass-argv-remote.git")
        (self.repo / "target").mkdir(exist_ok=True)
        driver = self.sandbox / "bypass_driver.py"
        driver.write_text(
            "import importlib.util, json, subprocess, sys\n"
            "from pathlib import Path\n"
            "from unittest import mock\n"
            "spec = importlib.util.spec_from_file_location('r', sys.argv[1])\n"
            "mod = importlib.util.module_from_spec(spec)\n"
            "spec.loader.exec_module(mod)\n"
            "real_run_process = mod._run_process\n"
            "def bypass(command, *, operation, selected=None, **options):\n"
            "    if command[:2] in (['git', 'tag'], ['git', 'update-ref'], ['git', 'push']):\n"
            "        return subprocess.CompletedProcess(command, 1, '', 'bypassed')\n"
            "    return real_run_process(command, operation=operation, selected=selected, **options)\n"
            "with mod._invocation_scope(), mock.patch.object(mod, '_run_process', side_effect=bypass):\n"
            "    prevented, evidence = mod._evaluate_tag_mutation_proof(\n"
            "        sys.argv[2], tag_canary_remote=Path(sys.argv[3])\n"
            "    )\n"
            "print(json.dumps({'prevented': prevented, 'evidence': evidence}))\n",
            encoding="utf-8",
        )
        sup = self.sandbox / "full-confinement.sh"
        sup.write_text(REHEARSAL._LINUX_WORKER_SUPERVISOR, encoding="utf-8")
        run = subprocess.run(
            [
                "unshare", "-Urmn", "--", "bash", str(sup),
                str(self.repo), "2", str(self.repo), str(remote),
                str(self.repo / "target"),
                sys.executable, str(driver), str(self.script), self.head, str(remote),
            ],
            capture_output=True,
            text=True,
            timeout=60,
            check=True,
        )
        payload = json.loads(run.stdout)
        self.assertIs(payload["prevented"], False)
        self.assertIs(payload["evidence"]["argv_denied"], False)
        self.assertIs(payload["evidence"]["runtime_confined"], True)
        self.assertEqual(payload["evidence"]["reason"], "invocation_boundary_bypassed")

    def test_cli_linux_worker_flag_end_to_end_and_conflicting_remote_rejected(self) -> None:
        output_path = self.sandbox / "cli-worker-receipt.json"
        cli = subprocess.run(
            [
                sys.executable, str(self.script),
                "--linux-worker", "--commit", "HEAD", "--output", str(output_path),
            ],
            cwd=self.repo,
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
        self.assertEqual(cli.returncode, 1, cli.stderr)
        receipt = json.loads(output_path.read_text(encoding="utf-8"))
        self.assertIs(receipt["zero_mutation_proof"]["tag_mutation_prevented"], True)
        for flag in self.UNPROVEN_SIX:
            self.assertIs(receipt["zero_mutation_proof"][flag], False)

        conflict = subprocess.run(
            [
                sys.executable, str(self.script),
                "--linux-worker", "--commit", "HEAD",
                "--tag-canary-remote", str(self.sandbox / "unused.git"),
            ],
            cwd=self.repo,
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
        self.assertEqual(conflict.returncode, 2)
        self.assertIn("manages its own canary remote", conflict.stderr)

    def test_linux_worker_rejects_symlinked_output_parent_without_creating_dirs(self) -> None:
        escape_target = self.sandbox / "escape-target"
        escape_target.mkdir()
        symlink_dir = self.sandbox / "symlink-dir"
        symlink_dir.symlink_to(escape_target, target_is_directory=True)
        output_path = symlink_dir / "created-through-symlink" / "receipt.json"

        with self.assertRaisesRegex(OSError, "filesystem alias"):
            REHEARSAL.run_in_linux_worker(
                "HEAD",
                output=output_path,
                repo_root=self.repo,
                script_path=self.script,
                capture_output=True,
            )
        self.assertEqual(list(escape_target.iterdir()), [])

    def test_linux_worker_rejects_symlinked_target_dir_without_mounting(self) -> None:
        external_dir = self.sandbox / "external-target"
        external_dir.mkdir()
        for target_dest in (self.repo / ".git/refs/tags", external_dir):
            with self.subTest(dest=target_dest.name):
                repo_target = self.repo / "target"
                if repo_target.is_symlink() or repo_target.exists():
                    if repo_target.is_symlink() or repo_target.is_file():
                        repo_target.unlink()
                    else:
                        shutil.rmtree(repo_target)
                repo_target.symlink_to(target_dest, target_is_directory=True)
                with self.assertRaisesRegex(OSError, "filesystem alias"):
                    REHEARSAL.run_in_linux_worker(
                        "HEAD",
                        repo_root=self.repo,
                        script_path=self.script,
                        capture_output=True,
                    )
                repo_target.unlink()
        self.assertEqual(self.show_tags(self.repo), "")
        self.assertEqual(list(external_dir.iterdir()), [])

    def test_linux_worker_enforces_timeout_and_reaps_descendants(self) -> None:
        marker = self.sandbox / "descendant-survived.txt"
        hanging_script = self.sandbox / "hang_worker.py"
        hanging_script.write_text(
            "import os, subprocess, sys, time\n"
            "from pathlib import Path\n"
            "marker = Path(sys.argv[1])\n"
            "# Spawn a detached background child that would write after 2s if not reaped.\n"
            "subprocess.Popen([\n"
            "    sys.executable, '-c',\n"
            "    'import sys, time; from pathlib import Path; time.sleep(2.0); Path(sys.argv[1]).write_text(\"survived\")',\n"
            "    str(marker),\n"
            "], start_new_session=True)\n"
            "time.sleep(30)\n",
            encoding="utf-8",
        )
        # Verify PID namespace + --kill-child reaping on launcher timeout.
        wrapper = self.sandbox / "wrap_hang.py"
        wrapper.write_text(
            "import subprocess, sys\n"
            f"subprocess.run([sys.executable, {str(hanging_script)!r}, {str(marker)!r}], check=False)\n",
            encoding="utf-8",
        )
        with self.assertRaises(subprocess.TimeoutExpired):
            REHEARSAL.run_in_linux_worker(
                "HEAD",
                repo_root=self.repo,
                script_path=wrapper,
                capture_output=True,
                timeout=1,
            )
        # Wait past the child's 2s sleep to prove --kill-child reaped the entire PID namespace.
        import time
        time.sleep(2.5)
        self.assertFalse(marker.exists(), "detached descendant survived PID-namespace termination")


if __name__ == "__main__":
    unittest.main()
