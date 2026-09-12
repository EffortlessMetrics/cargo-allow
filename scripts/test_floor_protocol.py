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
    with Path(case["calls"]).open("a", encoding="utf-8") as output:
        output.write(json.dumps({"program": program, "arguments": arguments}) + "\n")
    if program == "git":
        if arguments == ["rev-parse", "HEAD"]:
            return 0, "1" * 40 + "\n", ""
        if arguments[:3] == ["worktree", "add", "--detach"]:
            destination = Path(arguments[3]).resolve()
            destination.relative_to(Path(case["worktrees"]).resolve())
            shutil.copytree(case["fixture"], destination, dirs_exist_ok=True)
            return 0, "", ""
        if arguments[:3] == ["worktree", "remove", "--force"]:
            Path(arguments[3]).resolve().relative_to(Path(case["worktrees"]).resolve())
            return 0, "", ""  # the owning TemporaryDirectory performs cleanup
    if program in ("rustc", "cargo") and arguments == ["-vV"]:
        release = case.get(program + "_release", "1.95.2")
        host = case.get(program + "_host", "x86_64-pc-windows-msvc")
        return 0, "release: " + release + "\nhost: " + host + "\n", ""
    if program == "cargo" and len(arguments) == 5 and arguments[0] == "update":
        if arguments[1] != "-p" or arguments[3] != "--precise":
            return 70, "", "unexpected simulated update arguments"
        lock = 'version = 4\n[[package]]\nname = ' + json.dumps(arguments[2])
        lock += '\nversion = ' + json.dumps(arguments[4]) + '\n'
        Path("Cargo.lock").write_text(lock, encoding="utf-8", newline="\n")
        return 0, "", ""
    if program == "cargo" and arguments and arguments[0] in ("check", "test", "package"):
        if arguments[0] == "test":
            expected = [value for name in case["skips"] for value in ("--skip", name)]
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

    def run_producer(self, floor="0.2", *, overrides=None, product="cargo-allow", classes="check,test,package"):
        self.sequence += 1
        run_root = self.root / str(self.sequence)
        fixture = run_root / "fixture"
        scripts = fixture / "scripts"
        scripts.mkdir(parents=True)
        for name in ("proof-direct-floors.sh", "floor_execution_identity.py"):
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
        for name, body in manifests.items():
            member = fixture / "crates" / name
            member.mkdir(parents=True)
            (member / "Cargo.toml").write_text(body, encoding="utf-8", newline="\n")
        (fixture / "Cargo.lock").write_text(
            'version = 4\n[[package]]\nname = "enabled"\nversion = "0.1.0"\n',
            encoding="utf-8", newline="\n",
        )
        imports = run_root / "imports"
        imports.mkdir()
        (imports / "floor_protocol_shim.py").write_text(SHIM, encoding="utf-8", newline="\n")
        (imports / "subprocess.py").write_text(SUBPROCESS_FACADE, encoding="utf-8", newline="\n")
        fake_bin = run_root / "bin"
        fake_bin.mkdir()
        for program in ("git", "cargo", "rustc"):
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
        case = {"fixture": str(fixture), "worktrees": str(worktrees), "calls": str(calls_path), "skips": SKIPS}
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
        # Git Bash can reorder the inherited Windows PATH during startup.
        # Establish and verify fixture command resolution inside the shell.
        result = subprocess.run(
            [BASH, "-c", 'export PATH="$1:$PATH"; shift; '
             'for tool in git cargo rustc python3; do '
             '[ "$(command -v "$tool")" = "${PATH%%:*}/$tool" ] || exit 71; '
             'done; exec bash "$1"', "floor-protocol", shell_path(fake_bin),
             shell_path(scripts / "proof-direct-floors.sh")], cwd=fixture,
            env=environment, capture_output=True, text=True, timeout=45, check=False,
        )
        calls = [json.loads(line) for line in calls_path.read_text().splitlines()] if calls_path.exists() else []
        receipt = json.loads(receipt_path.read_bytes()) if receipt_path.exists() else None
        return result, calls, receipt, receipt_path

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
                classes = [call["arguments"] for call in calls if call["program"] == "cargo" and call["arguments"][0] in ("check", "test", "package")]
                self.assertEqual([arguments[0] for arguments in classes], ["check", "test", "package"])
                for arguments in classes:
                    self.assertEqual(arguments[arguments.index("--target") + 1], "x86_64-pc-windows-msvc")
                test_arguments = classes[1]
                self.assertEqual(test_arguments[test_arguments.index("--") + 1:],
                                 [value for name in SKIPS for value in ("--skip", name)])
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
        result, calls, receipt, _ = self.run_producer(overrides={"rustc_release": "1.96.0"})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not match MSRV", result.stderr)
        self.assertIsNone(receipt)
        self.assertFalse(any(call["program"] == "cargo" and call["arguments"][0] != "-vV" for call in calls))

    def test_failed_class_cannot_become_proven(self):
        result, _, receipt, _ = self.run_producer(overrides={"fail_class": "test"})
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(receipt["rows"])
        self.assertTrue(all(row["result"] == "instrument_failure" for row in receipt["rows"]))
        self.assertTrue(all("bounded proof class failed" in row["limitation"] for row in receipt["rows"]))

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


if __name__ == "__main__":
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--bash", required=True)
    options, remaining = parser.parse_known_args()
    BASH = options.bash
    if os.name == "nt" and not Path(BASH).is_file() and Path(BASH + ".exe").is_file():
        BASH += ".exe"
    unittest.main(argv=[sys.argv[0], *remaining])
