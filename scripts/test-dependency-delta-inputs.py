#!/usr/bin/env python3
"""Real-Git input-boundary tests; stop at Cargo without minting a receipt.

The captured argv/files prove only the shell producer's selected input bytes.
Rust delta semantics, hosted CI, Windows and release qualification are separate.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib
import unittest

SCRIPT = Path(os.environ.get("DELTA_SCRIPT", Path(__file__).with_name("check-dependency-graph-delta.sh"))).resolve()
STOP = 97


class InputBoundary(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="delta-inputs-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.home = self.root / "home"
        self.home.mkdir()
        # Do not inherit a caller's worktree, index, object store or config.
        self.env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
        for key in ("GITHUB_BASE_SHA", "DEPENDENCY_GRAPH_BUNDLE", "TARGET_DIR"):
            self.env.pop(key, None)
        self.env.update(HOME=str(self.home), GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
                        GIT_AUTHOR_NAME="Fixture", GIT_AUTHOR_EMAIL="fixture@example.invalid",
                        GIT_COMMITTER_NAME="Fixture", GIT_COMMITTER_EMAIL="fixture@example.invalid")
        self.git("init", "-q")
        self.put("Cargo.toml", '[workspace]\nmembers = ["crates/member"]\n[workspace.dependencies]\nrootdep = "1"\n')
        self.put("Cargo.lock", 'version = 4\n# base\n')
        self.put("crates/member/Cargo.toml", '[package]\nname="member"\nversion="0.1.0"\n[dependencies]\nmemberdep="1"\n')
        self.base = self.commit()
        self.put("Cargo.lock", 'version = 4\n# committed head\n')
        self.head = self.commit()
        self.capture = self.root / "capture.json"
        bin_dir = self.root / "bin"
        bin_dir.mkdir()
        cargo = bin_dir / "cargo"
        cargo.write_text('''#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
args = sys.argv[1:]
keys = ("--base-manifest", "--head-manifest", "--base-lock", "--head-lock")
values = {key: Path(args[args.index(key)+1]).read_text(encoding="utf-8") for key in keys}
Path(os.environ["DELTA_CAPTURE"]).write_text(json.dumps({"argv":args,"files":values}), encoding="utf-8")
# Intentional stop: no success receipt or semantic verdict is fabricated.
raise SystemExit(97)
''', encoding="utf-8")
        cargo.chmod(0o755)
        self.env.update(PATH=str(bin_dir)+os.pathsep+self.env["PATH"], DELTA_CAPTURE=str(self.capture))

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.repo, env=self.env, stderr=subprocess.PIPE, timeout=30).decode().strip()

    def put(self, path, text):
        out = self.repo / path
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")

    def commit(self):
        self.git("add", ".")
        self.git("commit", "-qm", "fixture")
        return self.git("rev-parse", "HEAD")

    def run_script(self, base=None):
        env = self.env.copy()
        if base is not None:
            env["GITHUB_BASE_SHA"] = base
        return subprocess.run(["bash", str(SCRIPT)], cwd=self.repo, env=env, capture_output=True, text=True, timeout=60)

    def captured(self):
        result = self.run_script(self.base)
        self.assertEqual(result.returncode, STOP, result.stdout+result.stderr)
        return json.loads(self.capture.read_text(encoding="utf-8"))

    def test_explicit_sha_works_without_origin_main(self):
        captured = self.captured()
        args = captured["argv"]
        self.assertEqual(args[args.index("--base-commit")+1], self.base)
        self.assertEqual(args[args.index("--head-commit")+1], self.head)

    def test_both_sides_keep_committed_member_dependencies(self):
        files = self.captured()["files"]
        for side in ("base", "head"):
            self.assertEqual(tomllib.loads(files[f"--{side}-manifest"])["dependencies"], {"memberdep":"1"})

    def test_dirty_root_cannot_borrow_head_identity(self):
        self.put("Cargo.toml", '[workspace.dependencies]\nrootdep="99"\n')
        files = self.captured()["files"]
        self.assertEqual(tomllib.loads(files["--head-manifest"])["workspace"]["dependencies"]["rootdep"], "1")

    def test_dirty_lock_cannot_borrow_head_identity(self):
        self.put("Cargo.lock", 'version=4\n# uncommitted substitution\n')
        files = self.captured()["files"]
        self.assertEqual(files["--base-lock"], 'version = 4\n# base\n')
        self.assertEqual(files["--head-lock"], 'version = 4\n# committed head\n')

    def test_dirty_member_cannot_borrow_head_identity(self):
        self.put("crates/member/Cargo.toml", '[dependencies]\nmemberdep="99"\n')
        files = self.captured()["files"]
        self.assertEqual(tomllib.loads(files["--head-manifest"])["dependencies"], {"memberdep":"1"})

    def test_untracked_member_is_not_in_committed_denominator(self):
        self.put("crates/untracked/Cargo.toml", '[dependencies]\nphantom="9"\n')
        files = self.captured()["files"]
        self.assertNotIn("phantom", tomllib.loads(files["--head-manifest"])["dependencies"])

    def test_deleted_worktree_member_does_not_omit_committed_dependency(self):
        shutil.rmtree(self.repo / "crates/member")
        files = self.captured()["files"]
        self.assertEqual(tomllib.loads(files["--head-manifest"])["dependencies"], {"memberdep":"1"})

    def test_real_added_member_exists_only_at_head(self):
        self.put("crates/added/Cargo.toml", '[dependencies]\naddeddep="2"\n')
        self.head = self.commit()
        files = self.captured()["files"]
        self.assertNotIn("addeddep", tomllib.loads(files["--base-manifest"])["dependencies"])
        self.assertEqual(tomllib.loads(files["--head-manifest"])["dependencies"]["addeddep"], "2")

    def test_real_removed_member_remains_in_base(self):
        shutil.rmtree(self.repo / "crates/member")
        self.head = self.commit()
        files = self.captured()["files"]
        self.assertEqual(tomllib.loads(files["--base-manifest"])["dependencies"], {"memberdep":"1"})
        self.assertEqual(tomllib.loads(files["--head-manifest"])["dependencies"], {})

    def test_non_manifest_crate_files_are_not_members(self):
        self.put("crates/README.md", "not a member\n")
        self.head = self.commit()
        files = self.captured()["files"]
        self.assertEqual(tomllib.loads(files["--head-manifest"])["dependencies"], {"memberdep":"1"})

    def test_staged_member_edit_cannot_borrow_head_identity(self):
        self.put("crates/member/Cargo.toml", '[dependencies]\nmemberdep="99"\n')
        self.git("add", "crates/member/Cargo.toml")
        files = self.captured()["files"]
        self.assertEqual(tomllib.loads(files["--head-manifest"])["dependencies"], {"memberdep":"1"})

    def test_nul_framing_preserves_member_path(self):
        self.put("crates/member with tab\tand newline\n/Cargo.toml", '[dependencies]\nunusual="2"\n')
        self.head = self.commit()
        files = self.captured()["files"]
        self.assertEqual(tomllib.loads(files["--head-manifest"])["dependencies"]["unusual"], "2")

    def test_invalid_explicit_base_refuses_before_cargo(self):
        result = self.run_script("missing-ref")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.capture.exists())

    def test_missing_implicit_base_refuses_before_cargo(self):
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.capture.exists())

    def test_default_base_resolves_existing_origin_main(self):
        self.git("update-ref", "refs/remotes/origin/main", self.base)
        result = self.run_script()
        self.assertEqual(result.returncode, STOP, result.stdout+result.stderr)

    def test_docs_only_committed_change_is_not_applicable(self):
        self.base = self.head
        self.put("README.md", "docs only\n")
        self.head = self.commit()
        result = self.run_script(self.base)
        self.assertEqual(result.returncode, 0, result.stdout+result.stderr)
        self.assertIn("not applicable", result.stdout)
        self.assertFalse(self.capture.exists())

    def test_git_diff_instrument_failure_is_not_mistaken_for_no_change(self):
        # Keep valid refs and only fail the router's Git diff invocation.
        # All earlier Git initialization and future verification still work.
        real_git = shutil.which("git", path=self.env["PATH"])
        self.assertIsNotNone(real_git)
        shim_dir = self.root / "git-shim"
        shim_dir.mkdir()
        shim = shim_dir / "git"
        shim.write_text(
            '#!/usr/bin/env python3\n'
            'import os, sys\n'
            'if sys.argv[1:2] == ["diff"]: raise SystemExit(128)\n'
            f'os.execv({real_git!r}, ["git", *sys.argv[1:]])\n',
            encoding="utf-8",
        )
        shim.chmod(0o755)
        self.env["PATH"] = str(shim_dir) + os.pathsep + self.env["PATH"]
        result = self.run_script(self.base)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("input routing failed (git diff exit 128)", result.stderr)
        self.assertNotIn("not applicable", result.stdout)
        self.assertFalse(self.capture.exists())

    def test_malformed_committed_base_member_refuses(self):
        self.put("crates/member/Cargo.toml", "[broken\n")
        self.base = self.commit()
        self.put("crates/member/Cargo.toml", '[dependencies]\nmemberdep="2"\n')
        self.head = self.commit()
        result = self.run_script(self.base)
        self.assertNotEqual(result.returncode, STOP)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.capture.exists())

    def test_invalid_utf8_committed_base_member_refuses(self):
        (self.repo / "crates/member/Cargo.toml").write_bytes(b'[dependencies]\nmemberdep="\xff"\n')
        self.base = self.commit()
        self.put("crates/member/Cargo.toml", '[dependencies]\nmemberdep="2"\n')
        self.head = self.commit()
        result = self.run_script(self.base)
        self.assertNotEqual(result.returncode, STOP)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.capture.exists())


if __name__ == "__main__":
    unittest.main(verbosity=2)
