"""Drive the actual floor producer with strict synthetic process observations."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest


REPO = Path(__file__).resolve().parent.parent
SKIPS = [
    "minimum_direct_version_drift_retained_receipts_are_current_with_the_live_tree",
    "check_exits_zero_when_every_release_set_receipt_is_current",
    "minimum_direct_version_fixtures_retained_proof_receipt_is_law_clean",
    "minimum_direct_version_products_retained_advisory_receipts_are_clean",
    "minimum_direct_version_products_cargo_allow_receipt_certifies_its_closure",
]

WORKSPACE_TESTS = [
    "ci_lane_topology_tests::crate_sets_partition_the_workspace_exactly",
    "package_topology_enforcement_tests::topology_classifies_every_workspace_package_exactly",
    "product_package_topology_tests::current_v2_authorities_drive_package_candidate",
    "publish_order_validation_tests::publish_order_covers_all_workspace_members",
    "release_prep_tests::published_release_versions_match_workspace",
]


def floor_classes(calls):
    return [call["arguments"] for call in calls if call["program"] == "cargo"
            and call["arguments"][0] in ("check", "test", "package")
            and "--bin" not in call["arguments"]]


# There is deliberately no real-process fallback in either shim. Python child
# calls see the same recorder through a fixture-only subprocess facade; this
# avoids depending on Windows executable/PATHEXT behavior for shell shims.
SHIM = r'''
import json
import os
from pathlib import Path
import shutil
import sys

def invoke(program, arguments):
    case = json.loads(Path(os.environ["FLOOR_PROTOCOL_CASE_FILE"]).read_text())
    executable = program
    program = Path(program).name.removesuffix(".exe")
    with Path(case["calls"]).open("a", encoding="utf-8") as output:
        output.write(json.dumps({"program": program, "arguments": arguments,
                                 "executable": executable,
                                 "projected": Path("target/floor-proof/product-workspace-identity.json").exists(),
                                 "rustc": os.environ.get("RUSTC"),
                                 "rustdoc": os.environ.get("RUSTDOC")}) + "\n")
    if program == "git":
        if arguments == ["rev-parse", "HEAD"]:
            derived = Path("target/floor-proof/fixture-derived").exists()
            return 0, ("2" if derived else "1") * 40 + "\n", ""
        if arguments == ["branch", "--show-current"]:
            return 0, "", ""
        if arguments == ["show", "1" * 40 + ":Cargo.toml"]:
            return 0, (Path(case["fixture"]) / "Cargo.toml").read_text(encoding="utf-8"), ""
        if arguments == ["ls-files", "-v", "-z"]:
            return 0, "H Cargo.lock\0H Cargo.toml\0", ""
        if arguments == ["ls-files", "--others", "--ignored", "--exclude-standard", "-z"]:
            return 0, "target/floor-proof/execution-identity.json\0", ""
        if arguments == ["status", "--porcelain=v1", "--untracked-files=all", "-z"]:
            if case.get("dirty_source"):
                return 0, " M source.rs\0", ""
            if Path("target/floor-proof/fixture-derived").exists():
                return 0, "", ""
            fixture = Path(case["fixture"])
            changed = []
            for relative in ("Cargo.lock", "Cargo.toml"):
                if Path(relative).read_bytes() != (fixture / relative).read_bytes():
                    changed.append(" M " + relative)
            return 0, "\0".join(changed) + ("\0" if changed else ""), ""
        if arguments[:2] == ["add", "--"] and set(arguments[2:]).issubset({"Cargo.lock", "Cargo.toml"}):
            return 0, "", ""
        if arguments == ["-c", "core.hooksPath=", "-c", "commit.gpgSign=false",
                         "-c", "user.name=cargo-allow floor proof",
                         "-c", "user.email=floor-proof@example.invalid", "commit",
                         "-m", "chore(proof): derive product-scoped direct-floor subject"]:
            if case.get("fail_derivation"):
                return 45, "", "simulated derived commit failure"
            Path("target/floor-proof/fixture-derived").write_text("derived")
            return 0, "", ""
        if arguments == ["rev-list", "--parents", "-n", "1", "HEAD"]:
            return 0, "2" * 40 + " " + "1" * 40 + "\n", ""
        if arguments == ["diff", "--name-only", "-z", "1" * 40, "2" * 40]:
            fixture = Path(case["fixture"])
            changed = [relative for relative in ("Cargo.lock", "Cargo.toml")
                       if Path(relative).read_bytes() != (fixture / relative).read_bytes()]
            return 0, "\0".join(changed) + ("\0" if changed else ""), ""
        if arguments == ["rev-parse", "HEAD^{tree}"]:
            return 0, "3" * 40 + "\n", ""
        if arguments[:3] == ["worktree", "add", "--detach"]:
            destination = Path(arguments[3]).resolve()
            destination.relative_to(Path(case["worktrees"]).resolve())
            shutil.copytree(case["fixture"], destination, dirs_exist_ok=True)
            return 0, "", ""
        if arguments[:3] == ["worktree", "remove", "--force"]:
            Path(arguments[3]).resolve().relative_to(Path(case["worktrees"]).resolve())
            return 0, "", ""  # the owning TemporaryDirectory performs cleanup
    if program in ("rustc", "rustdoc", "cargo") and arguments == ["-vV"]:
        release = case.get(program + "_release", "1.95.2")
        host = case.get(program + "_host", "x86_64-pc-windows-msvc")
        return 0, "release: " + release + "\nhost: " + host + "\n", ""
    if program == "cargo" and len(arguments) == 5 and arguments[0] == "update":
        if arguments[1] != "-p" or arguments[3] != "--precise":
            return 70, "", "unexpected simulated update arguments"
        package_spec = arguments[2]
        package = package_spec.split("@", 1)[0]
        if package == case.get("fail_pin"):
            return 42, "", "simulated floor pin failure"
        lock = 'version = 4\n[[package]]\nname = ' + json.dumps(package)
        lock += '\nversion = ' + json.dumps(arguments[4])
        lock += '\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n'
        Path("Cargo.lock").write_text(lock, encoding="utf-8", newline="\n")
        return 0, "", ""
    if program == "cargo" and arguments and arguments[0] in ("check", "test", "package"):
        if case.get("require_bound_tools"):
            for tool in ("rustc", "rustdoc"):
                configured = os.environ.get("CARGO_BUILD_" + tool.upper(), case["config_tools"][tool])
                actual = os.environ.get(tool.upper(), configured)
                if Path(actual) != Path(case["selected_tools"][tool]):
                    return 43, "", "simulated Cargo selected an unobserved " + tool
            for wrapper in ("RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
                configured = os.environ.get("CARGO_BUILD_" + wrapper, case["config_wrapper"])
                if os.environ.get(wrapper, configured) != "":
                    return 44, "", "simulated Cargo compiler wrapper remained active"
        if arguments[0] == "test" and "--bin" in arguments:
            expected = ["test", "--locked", "--target", "x86_64-pc-windows-msvc",
                        "--target-dir", "target/floor-proof/source-workspace-target",
                        "-p", "cargo-allow", "--bin", "cargo-allow", "--", "--format",
                        "pretty", "--color", "never", *case["workspace_tests"]]
            if arguments != expected:
                return 70, "", "unexpected source-workspace preflight command"
            if Path("target/floor-proof/product-workspace-identity.json").exists():
                return 70, "", "source-workspace preflight ran after projection"
            for name in ("Cargo.toml", "Cargo.lock"):
                if Path(name).read_bytes() != (Path(case["fixture"]) / name).read_bytes():
                    return 70, "", "source-workspace preflight ran after input mutation"
            mode = case.get("workspace_result", "passed")
            names = list(case["workspace_tests"])
            if mode == "wrong":
                names[0] = "foreign::test"
            if mode == "duplicate":
                names[0] = names[1]
            if mode == "extra":
                names.append(names[0] + "_extra")
            output = "running 5 tests\n"
            output += "".join("test " + name + " ... " +
                              ("ignored" if mode == "ignored" else "ok") + "\n" for name in names)
            output += ("test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; "
                       "1556 filtered out; finished in 0.01s\n")
            if mode == "empty":
                output = ""
            if mode == "source-moved":
                with Path("Cargo.lock").open("a") as lock:
                    lock.write("\n# changed during preflight\n")
            return (41 if mode == "failed" else 0), output, ""
        if arguments[0] == "test":
            expected = [value for name in case["skips"] + case["workspace_tests"] for value in ("--skip", name)]
            if "--" not in arguments or arguments[arguments.index("--") + 1:] != expected:
                return 39, "", "simulated retained-output skip contract mismatch"
        failed = arguments[0] == case.get("fail_class")
        return (41 if failed else 0), "", ("simulated class failure" if failed else "")
    return 70, "", "unexpected simulated command: " + program + " " + repr(arguments)

if __name__ == "__main__":
    status, output, error = invoke(sys.argv[1], sys.argv[2:])
    sys.stdout.write(output)
    sys.stderr.write(error)
    raise SystemExit(status)
'''

SUBPROCESS_FACADE = r'''
from types import SimpleNamespace
from floor_protocol_shim import invoke

class SubprocessError(Exception):
    pass

def run(arguments, check=False, **options):
    status, output, error = invoke(arguments[0], list(arguments[1:]))
    if check and status:
        raise SubprocessError(error)
    return SimpleNamespace(returncode=status, stdout=output, stderr=error)
'''


def shell_path(path):
    value = path.as_posix()
    return "/" + value[0].lower() + value[2:] if os.name == "nt" else value


class FloorProtocolTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="cargo-allow-floor-protocol-")
        self.root = Path(directory.name).resolve()
        if self.root.parent != Path(tempfile.gettempdir()).resolve():
            raise RuntimeError("fixture cleanup root escaped the admitted temporary directory")
        self.addCleanup(directory.cleanup)
        self.sequence = 0

    def run_producer(
        self, floor="0.2", *, overrides=None, product="cargo-allow",
        classes="check,test,package", extra_floor=False,
    ):
        self.sequence += 1
        run_root = self.root / str(self.sequence)
        fixture = run_root / "fixture"
        scripts = fixture / "scripts"
        scripts.mkdir(parents=True)
        for name in (
            "proof-direct-floors.sh",
            "floor_pin_settlement.py",
            "floor_product_workspace.py",
            "floor_execution_identity.py",
            "floor_source_identity.py",
            "floor_workspace_contract.py",
        ):
            shutil.copyfile(REPO / "scripts" / name, scripts / name)
        (fixture / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["crates/cargo-allow", "crates/fixture-helper"]\n'
            '[workspace.package]\nversion = "0.2.0"\nrust-version = "1.95"\n'
            '[workspace.dependencies]\nfixture-helper = { path = "crates/fixture-helper", features = ["activated"] }\n',
            encoding="utf-8", newline="\n",
        )
        manifests = {
            "cargo-allow": '[package]\nname = "cargo-allow"\nversion = "0.2.0"\n'
                           '[dependencies]\nfixture-helper.workspace = true\n',
            "fixture-helper": '[package]\nname = "fixture-helper"\nversion = "0.1.0"\n'
                              '[features]\nactivated = ["dep:enabled"]\n'
                              '[dependencies]\nenabled = { version = ' + json.dumps(floor) + ', optional = true }\n'
                              'inactive = { version = "0.8", optional = true }\n',
        }
        if extra_floor:
            manifests["fixture-helper"] += 'second = "0.3"\n'
        for name, body in manifests.items():
            member = fixture / "crates" / name
            member.mkdir(parents=True)
            (member / "Cargo.toml").write_text(body, encoding="utf-8", newline="\n")
        (fixture / "Cargo.lock").write_text(
            'version = 4\n[[package]]\nname = "enabled"\nversion = "0.1.0"\n'
            'source = "registry+https://github.com/rust-lang/crates.io-index"\n',
            encoding="utf-8", newline="\n",
        )
        imports = run_root / "imports"
        imports.mkdir()
        (imports / "floor_protocol_shim.py").write_text(SHIM, encoding="utf-8", newline="\n")
        (imports / "subprocess.py").write_text(SUBPROCESS_FACADE, encoding="utf-8", newline="\n")
        fake_bin = run_root / "bin"
        fake_bin.mkdir()
        for program in ("git", "cargo", "rustc", "rustdoc"):
            executable = fake_bin / program
            executable.write_text(
                "#!/bin/bash\nexec " + shlex.quote(sys.executable) + " "
                + shlex.quote(str(imports / "floor_protocol_shim.py")) + " " + program + ' "$@"\n',
                encoding="utf-8", newline="\n",
            )
            executable.chmod(0o755)
        python = fake_bin / "python3"
        python.write_text(
            "#!/bin/bash\nexec " + shlex.quote(sys.executable) + ' "$@"\n',
            encoding="utf-8", newline="\n",
        )
        python.chmod(0o755)
        worktrees = run_root / "worktrees"
        worktrees.mkdir()
        calls_path = run_root / "calls.jsonl"
        case = {"fixture": str(fixture), "worktrees": str(worktrees), "calls": str(calls_path), "skips": SKIPS,
                "workspace_tests": WORKSPACE_TESTS,
                "selected_tools": {tool: str(fake_bin / tool) for tool in ("rustc", "rustdoc")},
                "config_tools": {tool: str(run_root / ("configured-" + tool)) for tool in ("rustc", "rustdoc")},
                "config_wrapper": str(run_root / "configured-wrapper")}
        case.update(overrides or {})
        case_path = run_root / "case.json"
        case_path.write_text(json.dumps(case), encoding="utf-8")
        receipt_path = run_root / "out" / "receipt.json"
        environment = {key: value for key, value in os.environ.items() if key.upper() in {
            "PATH", "SYSTEMROOT", "WINDIR", "COMSPEC", "TEMP", "TMP", "HOME", "USERPROFILE", "PATHEXT", "LANG",
        }}
        environment.update({
            "PATH": str(fake_bin) + os.pathsep + os.environ.get("PATH", ""),
            "PYTHONPATH": str(imports), "FLOOR_PROTOCOL_CASE_FILE": str(case_path),
            "TMPDIR": shell_path(worktrees), "CI_PROOF_OUT": shell_path(receipt_path),
            "CI_PROOF_MSRV": "1.95", "CI_PROOF_PRODUCT": product, "CI_PROOF_CLASSES": classes,
            "CARGO_NET_OFFLINE": "true", "CARGO_HOME": str(run_root / "empty-cargo-home"),
            "RUSTC": str(run_root / "forbidden-real-rustc"),
        })
        if case.get("require_bound_tools"):
            environment.pop("RUSTC", None)
            mode = case.get("override_mode", "environment")
            for variable in ("RUSTC", "RUSTDOC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
                if mode == "environment":
                    environment[variable] = str(run_root / ("inherited-" + variable.lower()))
                elif mode == "config_environment":
                    environment["CARGO_BUILD_" + variable] = str(run_root / ("inherited-" + variable.lower()))
        # Git Bash can reorder the inherited Windows PATH during startup.
        # Establish and verify fixture command resolution inside the shell.
        result = subprocess.run(
            [BASH, "-c", 'export PATH="$1:$PATH"; shift; '
             'for tool in git cargo rustc rustdoc python3; do '
             '[ "$(command -v "$tool")" = "${PATH%%:*}/$tool" ] || exit 71; '
             'done; exec bash "$1"', "floor-protocol", shell_path(fake_bin),
             shell_path(scripts / "proof-direct-floors.sh")], cwd=fixture,
            env=environment, capture_output=True, text=True, timeout=45, check=False,
        )
        calls = [json.loads(line) for line in calls_path.read_text().splitlines()] if calls_path.exists() else []
        receipt = json.loads(receipt_path.read_bytes()) if receipt_path.exists() else None
        return result, calls, receipt, receipt_path

    def test_receipt_binds_the_product_projected_derived_commit(self):
        for floor in ("0.1", "0.2"):
            with self.subTest(floor=floor):
                result, calls, receipt, _ = self.run_producer(floor=floor)
                self.assertEqual(result.returncode, 0, result.stderr)
                subject = next(text for text in receipt["limitations"]
                               if text.startswith("local floor subject:"))
                projection = next(text for text in receipt["limitations"]
                                  if text.startswith("product workspace projection:"))
                self.assertIn("source " + "1" * 40, subject)
                self.assertIn("executed commit " + "2" * 40, subject)
                self.assertIn("not an upstream commit", subject)
                self.assertIn("cargo-allow.direct-floor-product-workspace.v1", projection)
                commits = [call for call in calls if call["program"] == "git"
                           and "commit" in call["arguments"]]
                self.assertEqual(len(commits), 1)

    def test_failed_source_derivation_prevents_classes_and_receipt(self):
        failures = {
            "dirty_source": "source-workspace preflight requires clean source",
            "fail_derivation": "simulated derived commit failure",
        }
        for failure, diagnostic in failures.items():
            with self.subTest(failure=failure):
                result, calls, receipt, _ = self.run_producer(overrides={failure: True})
                self.assertEqual(result.returncode, 1)
                self.assertIn(diagnostic, result.stderr)
                self.assertIsNone(receipt)
                self.assertFalse(floor_classes(calls))

    def test_cargo_uses_observed_tools_despite_inherited_and_configured_overrides(self):
        for mode in ("environment", "config_environment", "configuration"):
            with self.subTest(mode=mode):
                result, calls, receipt, _ = self.run_producer(
                    overrides={"require_bound_tools": True, "override_mode": mode},
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                observed = {call["program"]: call["executable"] for call in calls
                            if call["program"] in ("rustc", "rustdoc") and call["arguments"] == ["-vV"]}
                self.assertEqual(set(observed), {"rustc", "rustdoc"})
                for tool, executable in observed.items():
                    self.assertTrue(Path(executable).is_absolute())
                    for call in calls:
                        if call["program"] == "cargo":
                            self.assertEqual(call[tool], executable)
                self.assertTrue(all(row["result"] == "proven" for row in receipt["rows"]))

    def test_same_and_changed_floors_pin_the_selected_floor(self):
        for floor in ("0.1", "0.2"):
            with self.subTest(floor=floor):
                result, calls, receipt, _ = self.run_producer(floor)
                self.assertEqual(result.returncode, 0, result.stderr)
                updates = [call["arguments"] for call in calls if call["program"] == "cargo" and call["arguments"][0] == "update"]
                self.assertEqual(updates, [["update", "-p", "enabled", "--precise", floor + ".0"]])
                self.assertEqual([row["package"] for row in receipt["rows"]], ["enabled"])
                self.assertEqual(receipt["package_roots"], ["cargo-allow"])
                self.assertEqual(receipt["rows"][0]["tested_floor"], floor + ".0")
                self.assertEqual(receipt["rows"][0]["result"], "proven")
                self.assertEqual(receipt["toolchain"], "1.95.2")
                self.assertEqual(receipt["target"], "host:x86_64-pc-windows-msvc")
                classes = floor_classes(calls)
                self.assertEqual([arguments[0] for arguments in classes], ["check", "test", "package"])
                for arguments in classes:
                    self.assertEqual(arguments[arguments.index("--target") + 1], "x86_64-pc-windows-msvc")
                test_arguments = classes[1]
                self.assertEqual(test_arguments[test_arguments.index("--") + 1:],
                                 [value for name in SKIPS + WORKSPACE_TESTS for value in ("--skip", name)])
                self.assertTrue(any(" ".join(test_arguments) == command.removeprefix("cargo ") for command in receipt["commands"]))

    def test_bad_product_and_unknown_classes_reject_before_worktree(self):
        for options, reason in (({"product": "unknown"}, "unknown product"), ({"classes": "unknown"}, "unknown proof class")):
            with self.subTest(options=options):
                result, calls, receipt, _ = self.run_producer(**options)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(reason, result.stderr)
                self.assertEqual(calls, [])
                self.assertIsNone(receipt)

    def test_wrong_observed_toolchain_never_runs_a_proof_class(self):
        for tool in ("rustc", "rustdoc", "cargo"):
            with self.subTest(tool=tool):
                result, calls, receipt, _ = self.run_producer(overrides={tool + "_release": "1.96.0"})
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("does not match MSRV", result.stderr)
                self.assertIsNone(receipt)
                self.assertFalse(any(call["program"] == "cargo" and call["arguments"][0] != "-vV" for call in calls))

    def test_rustdoc_patch_must_match_the_observed_compiler(self):
        result, calls, receipt, _ = self.run_producer(overrides={"rustdoc_release": "1.95.1"})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("rustc and rustdoc releases differ", result.stderr)
        self.assertIsNone(receipt)
        self.assertFalse(any(call["program"] == "cargo" and call["arguments"][0] != "-vV" for call in calls))

    def test_failed_class_cannot_become_proven(self):
        result, _, receipt, _ = self.run_producer(overrides={"fail_class": "test"})
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(receipt["rows"])
        self.assertTrue(all(row["result"] == "instrument_failure" for row in receipt["rows"]))
        self.assertTrue(all("bounded proof class failed" in row["limitation"] for row in receipt["rows"]))

    def test_failed_pin_cannot_become_proven_when_classes_succeed(self):
        result, calls, receipt, _ = self.run_producer(
            overrides={"fail_pin": "second"}, extra_floor=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            [arguments[0] for arguments in floor_classes(calls)],
            ["check", "test", "package"],
        )
        failed = next(row for row in receipt["rows"] if row["package"] == "second")
        self.assertEqual(failed["result"], "resolver_failure")
        self.assertIn("simulated floor pin failure", failed["limitation"])

    def test_selection_companion_binds_witnesses_and_exact_receipt_bytes(self):
        result, _, _, receipt_path = self.run_producer()
        self.assertEqual(result.returncode, 0, result.stderr)
        raw = receipt_path.read_bytes()
        self.assertNotIn(b"\r", raw)
        companion = receipt_path.with_suffix(".selection.md").read_text(encoding="utf-8")
        self.assertIn(hashlib.sha256(raw).hexdigest(), companion)
        self.assertIn("fixture-helper/activated -> fixture-helper/dep:enabled", companion)
        self.assertIn("inactive | excluded | no selected feature enables", companion)
        self.assertIn("Starting source commit: " + "1" * 40, companion)
        self.assertIn("Workspace projection: cargo-allow.direct-floor-product-workspace.v1", companion)
        self.assertIn("Execution member paths:", companion)


    def test_original_workspace_pass_precedes_projection_and_floor_resolution(self):
        result, calls, receipt, receipt_path = self.run_producer()
        self.assertEqual(result.returncode, 0, result.stderr)
        preflights = [(index, call) for index, call in enumerate(calls)
                      if call["program"] == "cargo" and "--bin" in call["arguments"]]
        self.assertEqual(len(preflights), 1)
        index, call = preflights[0]
        self.assertFalse(call["projected"])
        updates = [(position, item) for position, item in enumerate(calls)
                   if item["program"] == "cargo" and item["arguments"][0] == "update"]
        self.assertTrue(updates)
        self.assertTrue(all(position > index and item["projected"] for position, item in updates))
        self.assertTrue(all("--bin" not in command for command in receipt["commands"]))
        self.assertTrue(any("original-workspace topology preflight:" in limitation
                            for limitation in receipt["limitations"]))
        companion = receipt_path.with_suffix(".selection.md").read_text(encoding="utf-8")
        self.assertIn("## Original-workspace topology preflight", companion)
        self.assertIn("not direct-floor proof", companion)
        self.assertEqual(companion.count("Passed before projection:"), 5)
        self.assertIn("Source commit: " + "1" * 40, companion)

    def test_invalid_workspace_preflight_prevents_projection_floor_classes_and_receipt(self):
        for mode in ("failed", "empty", "ignored", "wrong", "duplicate", "extra", "source-moved"):
            with self.subTest(mode=mode):
                result, calls, receipt, _ = self.run_producer(overrides={"workspace_result": mode})
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("source-workspace", result.stderr)
                self.assertIsNone(receipt)
                self.assertFalse(floor_classes(calls))
                self.assertFalse(any(call["projected"] for call in calls))
                self.assertFalse(any(call["program"] == "cargo" and call["arguments"][0] == "update"
                                     for call in calls))

    def test_check_only_proof_has_no_topology_test_preflight(self):
        result, calls, receipt, receipt_path = self.run_producer(classes="check")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any("--bin" in call["arguments"] for call in calls))
        self.assertEqual([arguments[0] for arguments in floor_classes(calls)], ["check"])
        self.assertFalse(any("topology preflight" in text for text in receipt["limitations"]))
        self.assertNotIn("Original-workspace topology preflight",
                         receipt_path.with_suffix(".selection.md").read_text(encoding="utf-8"))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--bash", required=True)
    options, remaining = parser.parse_known_args()
    BASH = options.bash
    if os.name == "nt" and not Path(BASH).is_file() and Path(BASH + ".exe").is_file():
        BASH += ".exe"
    unittest.main(argv=[sys.argv[0], *remaining])
