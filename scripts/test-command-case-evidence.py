#!/usr/bin/env python3
"""Transport controls and actual native admission rejection tests for #3149.

The Python harness checks observations from the Rust reader. It does not
implement a second semantic validator or grant release qualification.
"""

import argparse
import copy
from contextlib import redirect_stderr, redirect_stdout
import errno
import hashlib
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


sys.dont_write_bytecode = True
SCRIPT = Path(__file__).resolve().with_name("command-case-evidence.py")
REPOSITORY = SCRIPT.parent.parent
CATALOGUE = REPOSITORY / "docs/release/core-command-migration-cases.v1.json"
SPEC = importlib.util.spec_from_file_location("command_case_evidence", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise SystemExit("cannot load the production collector")
COLLECT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COLLECT)
NATIVE = None
SOURCE_GENERATION = None
EXPORT = None


def git_path():
    value = shutil.which("git")
    if value is None:
        raise RuntimeError("Git is required for the controlled tracked fixtures")
    return Path(value).resolve()


def arguments(root, binary, cases=None):
    return argparse.Namespace(
        binary=Path(binary), git=git_path(), catalogue=CATALOGUE,
        output_dir=root / "collection", collection_id="command-case-test",
        tool_version="0.2.0", source_generation=SOURCE_GENERATION or "1" * 40,
        provenance="source_build", candidate_identity=None, install_identity=None,
        case=cases, timeout=30.0,
    )


def report_identity(detail):
    # Adversarial fixtures deliberately rebind the existing canonical JSON
    # subject too. A transport-only mismatch must not be the rejection oracle.
    def scrub(value):
        if isinstance(value, dict):
            return {key: scrub(item) for key, item in value.items()
                    if key not in ("root", "source_tree_root", "policy_config", "started_at", "run_id")}
        if isinstance(value, list):
            return [scrub(item) for item in value]
        return value
    encoded = json.dumps(scrub(detail), sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode()
    return "local-repository:" + COLLECT.digest(encoded)


def preserve_candidate_profiles(collection, runner_profile):
    """Keep actual candidate profiles available to the coverage runner.

    Candidate execution uses only its owned fixture output directory. The
    test harness exports completed files before deleting that fixture;
    tarpaulin recursively collects its runner-owned profile directory.
    """
    profiles = sorted((collection / "fixtures").glob("*/target/command-case/profiles/*"))
    if not profiles:
        return None
    if runner_profile is None:
        raise ValueError("candidate profiles require an explicit runner LLVM_PROFILE_FILE")
    destination = Path(runner_profile).parent
    COLLECT.reject_symlinks(destination)
    if "%" in str(destination) or not destination.is_dir():
        raise ValueError("runner profile directory must be an existing concrete directory")
    # An exclusive child preserves any prior owner's files even when multiple
    # harnesses export profiles concurrently. Do not write the runner's own
    # pattern or rely on a timestamp/PID filename to establish ownership.
    exported = Path(tempfile.mkdtemp(prefix="command-case-", dir=destination))
    store = COLLECT.Store(exported)
    for source in profiles:
        if not source.name.startswith("candidate-") or source.suffix != ".profraw":
            raise ValueError("unexpected candidate profile output: " + str(source))
        data = COLLECT.regular_bytes(source, COLLECT.BINARY_LIMIT)
        store.put(source.relative_to(collection / "fixtures").as_posix(), data)
    print(f"retained {len(profiles)} candidate LLVM profiles in {exported}", file=sys.stderr)
    return exported


def remove_owned_fixture(root, identity):
    """Remove an owned tree, or explicitly retain it after a readonly refusal."""
    class _RetainReadonlyFixture(Exception):
        pass

    def inspect_owned(path):
        COLLECT.reject_symlinks(root)
        if not path.is_absolute() or ".." in path.parts:
            raise ValueError("cleanup requires an absolute owned path")
        relative = path.relative_to(root)
        current = root
        metadata = current.lstat()
        if (not stat.S_ISDIR(metadata.st_mode)
                or (metadata.st_dev, metadata.st_ino) != (identity.st_dev, identity.st_ino)):
            raise ValueError("owned fixture root changed before cleanup")
        for part in (None,) + relative.parts:
            if part is not None:
                current = current / part
                metadata = current.lstat()
            if stat.S_ISLNK(metadata.st_mode) or getattr(metadata, "st_file_attributes", 0) & 0x0400:
                raise ValueError("indirection in owned cleanup path")
        return metadata

    def retain_readonly_unlink(function, name, error):
        if (function is not os.unlink or not isinstance(error, PermissionError)
                or error.errno not in (errno.EACCES, errno.EPERM)):
            raise error
        path = Path(name)
        if path == root:
            raise ValueError("cleanup retention requires a strict owned descendant")
        before = inspect_owned(path)
        attributes = getattr(before, "st_file_attributes", None)
        if (not stat.S_ISREG(before.st_mode) or before.st_nlink != 1
                or before.st_mode & (stat.S_IWUSR | stat.S_IWGRP | stat.S_IWOTH)
                or (attributes is not None and not attributes & 0x0001)):
            raise error
        # Stop traversal immediately. Earlier entries may already be gone;
        # neither change permissions nor retry this unlink or its parents.
        raise _RetainReadonlyFixture({
            "status": "retained_readonly", "root": str(root),
            "failed_leaf": str(path), "errno": error.errno,
            "winerror": getattr(error, "winerror", None),
        }) from error

    inspect_owned(root)
    try:
        if sys.version_info >= (3, 12):
            shutil.rmtree(root, onexc=retain_readonly_unlink)
        else:
            # Python 3.11 and earlier pass sys.exc_info(), not the exception.
            # Preserve the original object and traceback for every refusal.
            def onerror(function, name, info):
                retain_readonly_unlink(function, name, info[1].with_traceback(info[2]))
            shutil.rmtree(root, onerror=onerror)
    except _RetainReadonlyFixture as retained:
        return retained.args[0]
    return {"status": "removed", "root": str(root)}


def invoke_rmtree_error(callbacks, function, name, error):
    """Inject a cleanup failure through the selected stdlib callback API."""
    if set(callbacks) == {"onexc"}:
        return callbacks["onexc"](function, name, error)
    if set(callbacks) == {"onerror"}:
        return callbacks["onerror"](function, name, (type(error), error, error.__traceback__))
    raise AssertionError("unexpected rmtree callback contract")


class TransportTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="command-case-transport-")
        self.root = Path(self.temporary.name).resolve()

    def tearDown(self):
        self.temporary.cleanup()

    def test_exclusive_root_preserves_prior_owner_and_refuses_checkout(self):
        owned = COLLECT.owned_root(self.root / "owned")
        canary = owned / "prior-owner"
        canary.write_bytes(b"prior owner")
        with self.assertRaises(FileExistsError):
            COLLECT.owned_root(owned)
        self.assertEqual(canary.read_bytes(), b"prior owner")
        (self.root / ".git").mkdir()
        with self.assertRaisesRegex(ValueError, "outside a repository"):
            COLLECT.owned_root(self.root / "inside")
        self.assertFalse((self.root / "inside").exists())

    def test_non_directory_and_duplicate_member_preserve_bytes(self):
        target = self.root / "existing"
        target.write_bytes(b"keep")
        with self.assertRaises(FileExistsError):
            COLLECT.owned_root(target)
        store = COLLECT.Store(self.root)
        member = store.put("member.json", b"first")
        with self.assertRaises(FileExistsError):
            store.put("member.json", b"second")
        self.assertEqual((self.root / member["path"]).read_bytes(), b"first")
        self.assertEqual(target.read_bytes(), b"keep")

    def test_member_paths_duplicate_json_and_nonregular_outputs_are_refused(self):
        for path in ("../escape", "/absolute", "x//y", "x/./y", "x\\y", "C:alias"):
            with self.subTest(path=path), self.assertRaises(ValueError):
                COLLECT.safe_relative(path)
        duplicate = self.root / "duplicate.json"
        duplicate.write_bytes(b'{"a":1,"a":2}')
        with self.assertRaisesRegex(ValueError, "duplicate"):
            COLLECT.load_json(duplicate)
        directory = self.root / "directory"
        directory.mkdir()
        store = COLLECT.Store(self.root)
        with self.assertRaisesRegex(ValueError, "nonregular"):
            COLLECT.capture_optional(store, "case", "summary", directory)
        self.assertIsNone(COLLECT.capture_optional(store, "case", "detail", directory, True))
        self.assertIsNone(COLLECT.capture_optional(store, "case", "summary", self.root / "missing"))

    @unittest.skipUnless(os.name != "nt", "symlink fixture needs no Windows privilege")
    def test_symlink_inputs_outputs_and_parent_are_refused(self):
        target = self.root / "target"
        target.write_bytes(b"keep")
        link = self.root / "link"
        link.symlink_to(target)
        with self.assertRaisesRegex(ValueError, "symlink"):
            COLLECT.regular_bytes(link, 20)
        with self.assertRaisesRegex(ValueError, "symlink"):
            COLLECT.capture_optional(COLLECT.Store(self.root), "case", "summary", link)
        parent = self.root / "parent"
        parent.symlink_to(self.root, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symlink"):
            COLLECT.owned_root(parent / "escape")
        self.assertEqual(target.read_bytes(), b"keep")

    def test_child_environment_is_explicit_without_mutating_parent(self):
        parent = dict(os.environ)
        env = COLLECT.child_environment(self.root / "home", git_path())
        self.assertEqual(os.environ, parent)
        self.assertNotIn("GIT_DIR", env)
        self.assertNotIn("GIT_WORK_TREE", env)
        self.assertFalse(any(key.startswith("CARGO_ALLOW") for key in env))
        self.assertEqual(env["GIT_CONFIG_GLOBAL"], os.devnull)
        paths = env["PATH"].split(os.pathsep)
        self.assertTrue(paths)
        self.assertTrue(all(Path(path).is_absolute() for path in paths), paths)
        store = COLLECT.Store(self.root)
        process, stdout, stderr = COLLECT.run_process(
            [sys.executable, "-c", "import json,os,sys; print(json.dumps(dict(os.environ),sort_keys=True)); sys.stderr.buffer.write(sys.stdin.buffer.read() + b'\\n')"],
            self.root, env, 10, store, "process",
        )
        self.assertEqual(process["exit_code"], 0)
        observed = json.loads((self.root / stdout["path"]).read_bytes())
        for key, value in env.items():
            self.assertEqual(observed[key], value)
        self.assertNotIn("GIT_DIR", observed)
        self.assertEqual((self.root / stderr["path"]).read_bytes(), b"\n")

    def test_child_environment_excludes_relative_default_search_paths(self):
        parent = dict(os.environ)
        git = git_path()
        absolute = self.root / "absolute-default-tools"
        absolute.mkdir()
        defaults = ["", ".", "relative-tools", "C:drive-relative", r"\root-relative",
                    str(git.parent), str(absolute), str(absolute)]
        with mock.patch.object(COLLECT.os, "defpath", os.pathsep.join(defaults)):
            env = COLLECT.child_environment(self.root / "absolute-path-home", git)
        self.assertEqual(os.environ, parent)
        self.assertEqual(env["PATH"].split(os.pathsep), [str(git.parent), str(absolute)])
        self.assertTrue(all(Path(path).is_absolute() for path in env["PATH"].split(os.pathsep)))
        store = COLLECT.Store(self.root)
        process, stdout, _ = COLLECT.run_process(
            [sys.executable, "-c", "import json,os; print(json.dumps(os.get_exec_path()))"],
            self.root, env, 10, store, "absolute-path-process",
        )
        self.assertEqual(process["exit_code"], 0)
        self.assertEqual(json.loads((self.root / stdout["path"]).read_bytes()),
                         [str(git.parent), str(absolute)])
        self.assertEqual(os.environ, parent)

    def test_child_environment_refuses_relative_git_before_home_allocation(self):
        parent = dict(os.environ)
        for index, git in enumerate((Path("git"), Path("C:relative-git"), Path(r"\root-relative-git"))):
            home = self.root / ("refused-git-home-" + str(index))
            with self.subTest(git=str(git)):
                with self.assertRaisesRegex(ValueError, "absolute"):
                    COLLECT.child_environment(home, git)
                self.assertFalse(home.exists())
        self.assertEqual(os.environ, parent)

    def test_catalogue_checkout_retains_pinned_bytes_with_autocrlf(self):
        env = COLLECT.child_environment(self.root / "checkout-home", git_path())
        checkout = self.root / "checkout"
        checkout.mkdir()
        relative = "docs/release/core-command-migration-cases.v1.json"

        def git(*argv, root=checkout):
            return subprocess.run([str(git_path()), "-C", str(root), "-c", "core.autocrlf=true", *argv],
                                  env=env, capture_output=True, check=True, timeout=10).stdout

        # Read the committed bytes independently of this host's checkout EOLs.
        canonical = git("show", "HEAD:" + relative, root=REPOSITORY)
        self.assertNotIn(b"\r\n", canonical)
        self.assertIn('"' + COLLECT.digest(canonical) + '"',
                      (REPOSITORY / "crates/cargo-allow/src/core_command_migration.rs").read_text())
        copied = checkout / relative
        copied.parent.mkdir(parents=True)
        copied.write_bytes(canonical)
        attributes = REPOSITORY / "docs/release/.gitattributes"
        if attributes.exists():
            (copied.parent / ".gitattributes").write_bytes(attributes.read_bytes())
        ordinary = checkout / "ordinary.txt"
        ordinary.write_bytes(b"ordinary\n")
        git("init", "--quiet")
        git("add", "--", ".")
        copied.unlink()
        ordinary.unlink()
        git("checkout-index", "--all", "--force")
        self.assertEqual(ordinary.read_bytes(), b"ordinary\r\n")
        self.assertEqual(copied.read_bytes(), canonical)

    def test_windows_named_executable_mode_keeps_bounded_read_reachable(self):
        source = self.root / "candidate.exe"
        source.write_bytes(b"bounded executable bytes\n")
        original_lstat = type(source).lstat

        def named_executable(path, *args, **kwargs):
            observed = original_lstat(path, *args, **kwargs)
            if os.fspath(path) == str(source):
                observed = self.changed_metadata(observed, st_mode=observed.st_mode | 0o111)
            return observed

        # CPython's Windows path stat infers executable bits from the suffix;
        # its descriptor stat has no filename from which to infer them.
        with mock.patch.object(COLLECT.os, "name", "nt"), mock.patch.object(
            type(source), "lstat", named_executable
        ):
            self.assertEqual(COLLECT.regular_bytes(source, 1024), source.read_bytes())

    def test_windows_cross_api_projection_preserves_identity_and_permissions(self):
        source = self.root / "candidate.exe"
        source.write_bytes(b"unchanged candidate\n")
        actual = source.stat()
        fields = {"st_dev": actual.st_dev + 1, "st_ino": actual.st_ino + 1,
                  "st_mode": actual.st_mode ^ 0o200, "st_nlink": actual.st_nlink + 1,
                  "st_size": actual.st_size + 1, "st_mtime_ns": actual.st_mtime_ns + 1}
        for field, value in fields.items():
            changed = self.changed_metadata(actual, **{field: value})
            with self.subTest(field=field), mock.patch.object(COLLECT.os, "name", "nt"), \
                    mock.patch.object(COLLECT.os, "fstat", return_value=changed):
                with self.assertRaisesRegex(ValueError, "changed before open"):
                    COLLECT.regular_bytes(source, 1024)
        self.assertEqual(source.read_bytes(), b"unchanged candidate\n")

    def test_descriptor_changes_during_read_keep_full_mode_identity(self):
        source = self.root / "candidate.exe"
        source.write_bytes(b"unchanged candidate\n")
        actual = source.stat()
        fields = {"st_dev": actual.st_dev + 1, "st_ino": actual.st_ino + 1,
                  "st_mode": actual.st_mode ^ 0o100, "st_nlink": actual.st_nlink + 1,
                  "st_size": actual.st_size + 1, "st_mtime_ns": actual.st_mtime_ns + 1}
        for field, value in fields.items():
            changed = self.changed_metadata(actual, **{field: value})
            with self.subTest(field=field), mock.patch.object(COLLECT.os, "name", "nt"), \
                    mock.patch.object(COLLECT.os, "fstat", side_effect=[actual, changed]):
                with self.assertRaisesRegex(ValueError, "changed while reading"):
                    COLLECT.regular_bytes(source, 1024)
        self.assertEqual(source.read_bytes(), b"unchanged candidate\n")

    def test_process_transport_retains_explicit_mixed_line_endings(self):
        env = COLLECT.child_environment(self.root / "byte-home", git_path())
        process, stdout, stderr = COLLECT.run_process(
            [sys.executable, "-c", "import sys; sys.stdout.buffer.write(b'one\\r\\ntwo\\n'); sys.stderr.buffer.write(b'err\\r\\n')"],
            self.root, env, 10, COLLECT.Store(self.root), "mixed-lines",
        )
        self.assertEqual(process["exit_code"], 0)
        for member, expected in ((stdout, b"one\r\ntwo\n"), (stderr, b"err\r\n")):
            self.assertEqual((self.root / member["path"]).read_bytes(), expected)
            self.assertEqual(member["digest"], COLLECT.digest(expected))
            self.assertEqual(member["size_bytes"], len(expected))

    def test_named_changes_after_read_keep_full_mode_identity(self):
        source = self.root / "candidate.exe"
        source.write_bytes(b"unchanged candidate\n")
        actual = source.stat()
        original_lstat, original_fstat = type(source).lstat, os.fstat
        fields = {"st_dev": actual.st_dev + 1, "st_ino": actual.st_ino + 1,
                  "st_mode": actual.st_mode ^ 0o100, "st_nlink": actual.st_nlink + 1,
                  "st_size": actual.st_size + 1, "st_mtime_ns": actual.st_mtime_ns + 1}
        for field, value in fields.items():
            calls = 0

            def descriptor_stat(descriptor):
                nonlocal calls
                calls += 1
                return original_fstat(descriptor)

            def named_stat(path, *args, **kwargs):
                observed = original_lstat(path, *args, **kwargs)
                if calls == 2 and os.fspath(path) == str(source):
                    return self.changed_metadata(observed, **{field: value})
                return observed

            with self.subTest(field=field), mock.patch.object(COLLECT.os, "name", "nt"), \
                    mock.patch.object(COLLECT.os, "fstat", side_effect=descriptor_stat), \
                    mock.patch.object(type(source), "lstat", named_stat):
                with self.assertRaisesRegex(ValueError, "changed while reading"):
                    COLLECT.regular_bytes(source, 1024)
        self.assertEqual(source.read_bytes(), b"unchanged candidate\n")

    @staticmethod
    def changed_metadata(metadata, **changes):
        fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns")
        values = {name: getattr(metadata, name) for name in fields}
        values.update(changes)
        return SimpleNamespace(**values)

    def test_one_supplied_binary_invocation_and_fresh_absence_are_retained(self):
        # Python deliberately refuses cargo-allow argv. This tests process
        # transport and absence only; native semantic tests use cargo-allow.
        args = arguments(self.root, Path(sys.executable).resolve(), ["A.audit.clean_no_policy"])
        with mock.patch.object(COLLECT, "run_process", wraps=COLLECT.run_process) as calls:
            with redirect_stdout(io.StringIO()):
                COLLECT.collect(args)
        binary_calls = [call for call in calls.call_args_list if call.args[0][0] == str(args.binary)]
        self.assertEqual(len(binary_calls), 1)
        bundle = json.loads((args.output_dir / "bundle.json").read_bytes())
        case = bundle["cases"][0]
        self.assertTrue(case["process"]["started"])
        self.assertNotEqual(case["process"]["exit_code"], 0)
        self.assertIsNone(case["detail"])
        self.assertIsNone(case["summary"])
        self.assertIsNone(case["receipt"])
        self.assertEqual(bundle["binary_digest_after"], bundle["binary"]["digest"])
        self.assertEqual(case["context"]["source_snapshot_digest"], case["before"]["digest"])
        outputs = Path(case["context"]["root"]) / "target/command-case/profiles"
        self.assertTrue(outputs.is_dir())
        self.assertEqual(case["context"]["environment"].get("LLVM_PROFILE_FILE"),
                         str(outputs / "candidate-%m-%p.profraw"))

    def test_profile_export_preserves_bytes_prior_owners_and_parent_environment(self):
        # These are copy/ownership sentinels, not claimed valid LLVM profiles.
        collection = self.root / "collection"
        source = collection / "fixtures/A.audit.clean_no_policy/target/command-case/profiles/candidate-1-2.profraw"
        source.parent.mkdir(parents=True)
        source.write_bytes(b"profile copy sentinel\n")
        runner = self.root / "runner-profiles"
        runner.mkdir()
        prior = runner / "candidate-1-2.profraw"
        prior.write_bytes(b"prior owner\n")
        pattern = str(runner / "runner-%m-%p.profraw")
        parent = dict(os.environ)
        first = preserve_candidate_profiles(collection, pattern)
        second = preserve_candidate_profiles(collection, pattern)
        self.assertNotEqual(first, second)
        relative = source.relative_to(collection / "fixtures")
        self.assertEqual((first / relative).read_bytes(), source.read_bytes())
        self.assertEqual((second / relative).read_bytes(), source.read_bytes())
        self.assertEqual(prior.read_bytes(), b"prior owner\n")
        self.assertEqual(os.environ, parent)
        for refused in (None, "relative/profile.profraw", str(self.root / "absent/profile.profraw"),
                        str(self.root / "%p/profile.profraw")):
            with self.subTest(destination=refused), self.assertRaises(ValueError):
                preserve_candidate_profiles(collection, refused)
        self.assertEqual(source.read_bytes(), b"profile copy sentinel\n")
        if os.name != "nt":
            link = self.root / "linked-runner"
            link.symlink_to(runner, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "symlink"):
                preserve_candidate_profiles(collection, str(link / "profile.profraw"))
            self.assertEqual(prior.read_bytes(), b"prior owner\n")

    def test_owned_readonly_git_object_cleanup_retains_remaining_fixture(self):
        prior = self.root / "prior-owner"
        prior.write_bytes(b"outside prior owner\n")
        prior_mode = prior.lstat().st_mode
        for error_number in (errno.EACCES, errno.EPERM):
            with self.subTest(errno=error_number):
                owned = Path(tempfile.mkdtemp(prefix="owned-cleanup-", dir=self.root)).resolve()
                identity = owned.lstat()
                leaf = owned / ".git/objects/aa/object"
                leaf.parent.mkdir(parents=True)
                leaf.write_bytes(b"owned object\n")
                leaf.chmod(0o444)
                before = leaf.lstat()
                original_unlink = os.unlink
                attempts = []

                def readonly_unlink(path, *args, **kwargs):
                    observed = os.stat(path, dir_fd=kwargs.get("dir_fd"), follow_symlinks=False)
                    if (observed.st_dev, observed.st_ino) == (before.st_dev, before.st_ino):
                        attempts.append(stat.S_IMODE(observed.st_mode))
                        raise PermissionError(error_number, "controlled read-only unlink", os.fspath(path))
                    return original_unlink(path, *args, **kwargs)

                try:
                    with mock.patch.object(os, "unlink", side_effect=readonly_unlink), \
                            mock.patch.object(os, "chmod", side_effect=AssertionError("cleanup changed permissions")) as chmod, \
                            mock.patch.object(os, "fchmod", create=True, side_effect=AssertionError("cleanup changed permissions")) as fchmod, \
                            mock.patch.object(os, "rmdir", wraps=os.rmdir) as rmdir:
                        result = remove_owned_fixture(owned, identity)
                    self.assertEqual(result, {
                        "status": "retained_readonly", "root": str(owned),
                        "failed_leaf": str(leaf), "errno": error_number, "winerror": None,
                    })
                    self.assertEqual(attempts, [stat.S_IMODE(before.st_mode)])
                    chmod.assert_not_called()
                    fchmod.assert_not_called()
                    rmdir.assert_not_called()
                    self.assertTrue(owned.is_dir())
                    self.assertEqual(leaf.read_bytes(), b"owned object\n")
                    self.assertEqual(leaf.lstat().st_mode, before.st_mode)
                    self.assertEqual(prior.read_bytes(), b"outside prior owner\n")
                    self.assertEqual(prior.lstat().st_mode, prior_mode)
                finally:
                    # Release only this test's known leaf after the assertions;
                    # the production cleanup path never changes permissions.
                    leaf.chmod(0o600)

    def test_owned_cleanup_accepts_legacy_and_current_rmtree_callbacks(self):
        # The wrapper accepts Python 3.11, whose rmtree has only onerror.
        # Keep both real callback contracts observable on either test host.
        actual_rmtree = shutil.rmtree
        for version in ((3, 11), (3, 12)):
            for fail in (False, True):
                with self.subTest(version=version, fail=fail):
                    owned = Path(tempfile.mkdtemp(prefix="owned-python-api-", dir=self.root)).resolve()
                    identity = owned.lstat()
                    leaf = owned / "leaf"
                    leaf.write_bytes(b"callback identity sentinel\n")
                    error = OSError(errno.EIO, "original cleanup failure")
                    try:
                        raise error
                    except OSError:
                        original_info = sys.exc_info()
                    callbacks = []

                    def legacy(path, ignore_errors=False, onerror=None, *, dir_fd=None):
                        self.assertEqual(path, owned)
                        self.assertFalse(ignore_errors)
                        self.assertIsNone(dir_fd)
                        callbacks.append("onerror")
                        if fail:
                            onerror(os.unlink, leaf, original_info)
                        else:
                            actual_rmtree(path)

                    def current(path, *, onexc):
                        self.assertEqual(path, owned)
                        callbacks.append("onexc")
                        if fail:
                            onexc(os.unlink, leaf, error)
                        else:
                            actual_rmtree(path)

                    expected = "onerror" if version < (3, 12) else "onexc"
                    backend = legacy if version < (3, 12) else current
                    with mock.patch.object(sys, "version_info", version), \
                            mock.patch.object(shutil, "rmtree", side_effect=backend):
                        if fail:
                            try:
                                remove_owned_fixture(owned, identity)
                            except OSError as raised:
                                self.assertIs(raised, error)
                                traceback = raised.__traceback__
                                while traceback is not None and traceback is not original_info[2]:
                                    traceback = traceback.tb_next
                                self.assertIs(traceback, original_info[2])
                            else:
                                self.fail("cleanup swallowed the original failure")
                            self.assertEqual(leaf.read_bytes(), b"callback identity sentinel\n")
                        else:
                            self.assertEqual(remove_owned_fixture(owned, identity), {
                                "status": "removed", "root": str(owned),
                            })
                            self.assertFalse(owned.exists())
                    self.assertEqual(callbacks, [expected])

    def test_owned_cleanup_propagates_unrelated_writable_and_metadata_failures(self):
        for label in ("io", "other-errno", "writable", "group-write", "other-write",
                      "wrong-callback", "missing-readonly-attribute", "metadata-error"):
            with self.subTest(failure=label):
                owned = Path(tempfile.mkdtemp(prefix="owned-failure-", dir=self.root)).resolve()
                identity = owned.lstat()
                leaf = owned / "leaf"
                leaf.write_bytes(b"retain failed leaf\n")
                leaf.chmod(0o600 if label == "writable" else 0o444)
                before = leaf.lstat()
                error = (OSError(errno.EIO, "controlled I/O failure") if label == "io"
                         else PermissionError(errno.EBUSY if label == "other-errno" else errno.EACCES,
                                              "controlled unrelated failure"))
                metadata_error = OSError(errno.EIO, "controlled metadata failure")
                original_lstat = Path.lstat

                def inspect(path, *args, **kwargs):
                    observed = original_lstat(path, *args, **kwargs)
                    if path != leaf:
                        return observed
                    if label == "metadata-error":
                        raise metadata_error
                    if label in ("group-write", "other-write", "missing-readonly-attribute"):
                        mode = observed.st_mode
                        if label == "group-write":
                            mode |= stat.S_IWGRP
                        elif label == "other-write":
                            mode |= stat.S_IWOTH
                        return SimpleNamespace(st_mode=mode, st_nlink=observed.st_nlink,
                                               st_file_attributes=0 if label == "missing-readonly-attribute" else 1)
                    return observed

                def invoke_callback(root, **callbacks):
                    self.assertEqual(root, owned)
                    invoke_rmtree_error(callbacks, os.rmdir if label == "wrong-callback" else os.unlink, leaf, error)

                try:
                    with mock.patch.object(shutil, "rmtree", side_effect=invoke_callback), \
                            mock.patch.object(Path, "lstat", autospec=True, side_effect=inspect), \
                            mock.patch.object(os, "chmod", side_effect=AssertionError("cleanup changed permissions")) as chmod, \
                            mock.patch.object(os, "fchmod", create=True, side_effect=AssertionError("cleanup changed permissions")) as fchmod, \
                            mock.patch.object(os, "unlink") as unlink:
                        with self.assertRaises(OSError) as raised:
                            remove_owned_fixture(owned, identity)
                    self.assertIs(raised.exception, metadata_error if label == "metadata-error" else error)
                    chmod.assert_not_called()
                    fchmod.assert_not_called()
                    unlink.assert_not_called()
                    self.assertEqual(leaf.read_bytes(), b"retain failed leaf\n")
                    self.assertEqual(leaf.lstat().st_mode, before.st_mode)
                finally:
                    leaf.chmod(0o600)

    def test_owned_cleanup_refuses_outside_alias_nonfile_and_changed_root(self):
        owned = Path(tempfile.mkdtemp(prefix="owned-refusal-", dir=self.root)).resolve()
        identity = owned.lstat()
        prior = self.root / "readonly-prior-owner"
        prior.write_bytes(b"keep outside bytes and mode\n")
        prior.chmod(0o444)
        prior_mode = prior.lstat().st_mode
        directory = owned / "directory"
        directory.mkdir()
        alias = owned / "hard-link"
        os.link(prior, alias)
        refused = [prior, owned, Path("relative-leaf"), owned / ".." / prior.name, directory, alias]
        if os.name != "nt":
            symlink = owned / "symlink"
            symlink.symlink_to(prior)
            refused.append(symlink)
        for path in refused:
            def invoke_callback(root, **callbacks):
                self.assertEqual(root, owned)
                invoke_rmtree_error(callbacks, os.unlink, path, PermissionError(errno.EACCES, "controlled refusal"))

            with self.subTest(path=path), mock.patch.object(shutil, "rmtree", side_effect=invoke_callback), \
                    mock.patch.object(os, "chmod", side_effect=AssertionError("cleanup changed permissions")) as chmod, \
                    mock.patch.object(os, "fchmod", create=True, side_effect=AssertionError("cleanup changed permissions")) as fchmod, \
                    mock.patch.object(os, "unlink") as unlink:
                with self.assertRaises((ValueError, PermissionError)):
                    remove_owned_fixture(owned, identity)
                chmod.assert_not_called()
                fchmod.assert_not_called()
                unlink.assert_not_called()
            self.assertEqual(prior.read_bytes(), b"keep outside bytes and mode\n")
            self.assertEqual(prior.lstat().st_mode, prior_mode)
        # A synthetic metadata observation exercises the Windows reparse bit
        # without creating a privileged junction on this host.
        leaf = owned / "reparse-observation"
        leaf.write_bytes(b"keep reparse sentinel\n")
        original_lstat = Path.lstat

        def reparse_observation(path, *args, **kwargs):
            observed = original_lstat(path, *args, **kwargs)
            if path == leaf:
                return SimpleNamespace(st_mode=stat.S_IFREG | 0o444, st_nlink=1,
                                       st_file_attributes=0x0401)
            return observed

        def reparse_callback(root, **callbacks):
            invoke_rmtree_error(callbacks, os.unlink, leaf, PermissionError(errno.EACCES, "controlled reparse refusal"))

        with mock.patch.object(shutil, "rmtree", side_effect=reparse_callback), \
                mock.patch.object(Path, "lstat", autospec=True, side_effect=reparse_observation), \
                mock.patch.object(os, "chmod") as chmod, \
                mock.patch.object(os, "fchmod", create=True) as fchmod, \
                mock.patch.object(os, "unlink") as unlink:
            with self.assertRaisesRegex(ValueError, "indirection"):
                remove_owned_fixture(owned, identity)
            chmod.assert_not_called()
            fchmod.assert_not_called()
            unlink.assert_not_called()
        self.assertEqual(leaf.read_bytes(), b"keep reparse sentinel\n")
        changed = SimpleNamespace(st_dev=identity.st_dev, st_ino=identity.st_ino + 1)
        with mock.patch.object(shutil, "rmtree") as remove, mock.patch.object(os, "chmod") as chmod:
            with self.assertRaisesRegex(ValueError, "root changed"):
                remove_owned_fixture(owned, changed)
            remove.assert_not_called()
            chmod.assert_not_called()
        # Release these known test sentinels only after the unchanged-owner
        # assertions; a Windows readonly hard link cannot be unlinked yet.
        prior.chmod(0o600)
        alias.unlink()

    def test_native_cleanup_exports_before_delete_and_retains_failed_export(self):
        for fail in (True, False):
            with self.subTest(export_fails=fail):
                owned = Path(tempfile.mkdtemp(prefix="owned-export-", dir=self.root)).resolve()
                collection = owned / "collection"
                profile = collection / "fixtures/A.fake/target/command-case/profiles/candidate-unit.profraw"
                profile.parent.mkdir(parents=True)
                profile.write_bytes(b"controlled profile custody sentinel\n")
                leaf = owned / "readonly-object"
                leaf.write_bytes(b"remaining fixture sentinel\n")
                leaf.chmod(0o444)
                before = leaf.lstat()
                runner = Path(tempfile.mkdtemp(prefix="runner-", dir=self.root))
                fixture = SimpleNamespace(root=owned, root_identity=owned.lstat(), collection=collection)
                calls = []
                original_export = preserve_candidate_profiles
                original_unlink = os.unlink
                diagnostic = io.StringIO()

                def export(actual, runner_profile):
                    self.assertEqual(actual, collection)
                    self.assertEqual(profile.read_bytes(), b"controlled profile custody sentinel\n")
                    calls.append("export")
                    if fail:
                        raise OSError(errno.EIO, "controlled export failure")
                    return original_export(actual, runner_profile)

                def readonly_unlink(path, *args, **kwargs):
                    observed = os.stat(path, dir_fd=kwargs.get("dir_fd"), follow_symlinks=False)
                    if (observed.st_dev, observed.st_ino) == (before.st_dev, before.st_ino):
                        raise PermissionError(errno.EACCES, "controlled readonly fixture", os.fspath(path))
                    return original_unlink(path, *args, **kwargs)

                try:
                    with mock.patch.object(sys.modules[__name__], "preserve_candidate_profiles", side_effect=export), \
                            mock.patch.object(sys.modules[__name__], "remove_owned_fixture", wraps=remove_owned_fixture) as remove, \
                            mock.patch.dict(os.environ, {"LLVM_PROFILE_FILE": str(runner / "harness-%p.profraw")}), \
                            mock.patch.object(os, "unlink", side_effect=readonly_unlink), \
                            mock.patch.object(os, "chmod", side_effect=AssertionError("cleanup changed permissions")), \
                            mock.patch.object(os, "fchmod", create=True, side_effect=AssertionError("cleanup changed permissions")), \
                            redirect_stderr(diagnostic):
                        if fail:
                            with self.assertRaisesRegex(RuntimeError, "fixtures retained"):
                                NativeAdmissionTests.tearDownClass.__func__(fixture)
                            remove.assert_not_called()
                            self.assertEqual(profile.read_bytes(), b"controlled profile custody sentinel\n")
                            self.assertEqual(list(runner.iterdir()), [])
                        else:
                            NativeAdmissionTests.tearDownClass.__func__(fixture)
                            remove.assert_called_once_with(owned, fixture.root_identity)
                    self.assertTrue(owned.is_dir())
                    self.assertEqual(leaf.read_bytes(), b"remaining fixture sentinel\n")
                    self.assertEqual(leaf.lstat().st_mode, before.st_mode)
                    self.assertEqual(calls, ["export"])
                    if not fail:
                        copies = list(runner.glob("command-case-*/A.fake/target/command-case/profiles/candidate-unit.profraw"))
                        self.assertEqual(len(copies), 1)
                        self.assertEqual(copies[0].read_bytes(), b"controlled profile custody sentinel\n")
                        self.assertIn("remaining fixture retained at " + str(owned), diagnostic.getvalue())
                        self.assertIn(str(leaf), diagnostic.getvalue())
                        self.assertIn("errno=" + str(errno.EACCES), diagnostic.getvalue())
                finally:
                    leaf.chmod(0o600)

    def test_owned_writable_fixture_cleanup_reports_removed(self):
        owned = Path(tempfile.mkdtemp(prefix="owned-writable-", dir=self.root)).resolve()
        identity = owned.lstat()
        (owned / "leaf").write_bytes(b"ordinary cleanup\n")
        diagnostic = io.StringIO()
        with mock.patch.object(os, "chmod", side_effect=AssertionError("cleanup changed permissions")) as chmod, \
                mock.patch.object(os, "fchmod", create=True, side_effect=AssertionError("cleanup changed permissions")) as fchmod, \
                redirect_stderr(diagnostic):
            result = remove_owned_fixture(owned, identity)
        self.assertEqual(result, {"status": "removed", "root": str(owned)})
        self.assertFalse(owned.exists())
        self.assertEqual(diagnostic.getvalue(), "")
        chmod.assert_not_called()
        fchmod.assert_not_called()

    def test_native_retention_does_not_hide_an_earlier_test_failure(self):
        owned = Path(tempfile.mkdtemp(prefix="owned-prior-failure-", dir=self.root)).resolve()
        collection = owned / "collection"
        collection.mkdir()
        leaf = owned / "leaf"
        leaf.write_bytes(b"retained after original failure\n")
        leaf.chmod(0o444)
        fixture = SimpleNamespace(root=owned, root_identity=owned.lstat(), collection=collection)

        class PriorFailure(unittest.TestCase):
            @classmethod
            def tearDownClass(cls):
                NativeAdmissionTests.tearDownClass.__func__(fixture)

            def runTest(self):
                self.fail("controlled original admission failure")

        result = unittest.TestResult()
        diagnostic = io.StringIO()
        try:
            with mock.patch.object(os, "unlink", side_effect=PermissionError(errno.EACCES, "controlled readonly unlink")), \
                    mock.patch.object(os, "chmod", side_effect=AssertionError("cleanup changed permissions")), \
                    mock.patch.object(os, "fchmod", create=True, side_effect=AssertionError("cleanup changed permissions")), \
                    redirect_stderr(diagnostic):
                unittest.TestSuite([PriorFailure()]).run(result)
            self.assertEqual(result.testsRun, 1)
            self.assertEqual(len(result.failures), 1)
            self.assertEqual(result.errors, [])
            self.assertFalse(result.wasSuccessful())
            self.assertIn("controlled original admission failure", result.failures[0][1])
            self.assertIn("remaining fixture retained at " + str(owned), diagnostic.getvalue())
            self.assertEqual(leaf.read_bytes(), b"retained after original failure\n")
        finally:
            leaf.chmod(0o600)

    def test_unstarted_and_timed_out_processes_remain_distinct(self):
        store = COLLECT.Store(self.root)
        env = COLLECT.child_environment(self.root / "home", git_path())
        unstarted, _, _ = COLLECT.run_process(
            [str(self.root / "absent-binary")], self.root, env, 1, store, "unstarted",
        )
        self.assertFalse(unstarted["started"])
        self.assertIsNone(unstarted["exit_code"])
        self.assertIsNotNone(unstarted["launch_error"])
        timeout, _, _ = COLLECT.run_process(
            [sys.executable, "-c", "import time; time.sleep(2)"],
            self.root, env, 0.05, store, "timeout",
        )
        self.assertTrue(timeout["started"])
        self.assertTrue(timeout["timed_out"])
        self.assertIsNotNone(timeout["exit_code"])
        self.assertIsNone(timeout["launch_error"])

    def test_case_and_provenance_rejections_happen_before_root_allocation(self):
        for cases in (["unknown"], ["A.audit.clean_no_policy"] * 2, ["D.diff.exact_range"]):
            args = arguments(self.root, Path(sys.executable).resolve(), cases)
            with self.subTest(cases=cases), self.assertRaises(ValueError):
                COLLECT.collect(args)
            self.assertFalse(args.output_dir.exists())
        args = arguments(self.root, Path(sys.executable).resolve(), ["A.audit.clean_no_policy"])
        args.install_identity = "sha256:v1:" + "a" * 64
        with self.assertRaisesRegex(ValueError, "source build"):
            COLLECT.collect(args)
        self.assertFalse(args.output_dir.exists())
        args = arguments(self.root, Path(sys.executable).resolve(), ["A.audit.clean_no_policy"])
        args.output_dir = self.root / "expanded-%p"
        with self.assertRaisesRegex(ValueError, "profile.*pattern"):
            COLLECT.collect(args)
        self.assertFalse(args.output_dir.exists())

    def test_rewritten_catalogue_cannot_dispatch_later_commands_or_unbounded_cases(self):
        catalogue, _ = COLLECT.load_json(CATALOGUE)
        for mode in ("later_command", "too_many"):
            changed = copy.deepcopy(catalogue)
            case = next(item for item in changed["cases"] if item["id"] == "A.audit.clean_no_policy")
            if mode == "later_command":
                case["command"] = "add"
                selected = [case["id"]]
            else:
                selected = []
                for index in range(30):
                    row = copy.deepcopy(case)
                    row["id"] = f"A.audit.extra_{index}"
                    changed["cases"].append(row)
                    selected.append(row["id"])
            args = arguments(self.root, Path(sys.executable).resolve(), selected)
            with self.subTest(mode=mode), mock.patch.object(
                COLLECT, "load_json", return_value=(changed, COLLECT.json_bytes(changed))
            ), mock.patch.object(COLLECT, "run_process") as calls:
                with self.assertRaisesRegex(ValueError, "bounded read-only"):
                    COLLECT.collect(args)
                calls.assert_not_called()
                self.assertFalse(args.output_dir.exists())

    @unittest.skipUnless(hasattr(os, "mkfifo"), "named-pipe control requires Unix FIFO support")
    def test_fifo_catalogue_and_binary_are_refused_before_collection_allocation(self):
        fifo = self.root / "unwritten-fifo"
        os.mkfifo(fifo)
        for role in ("catalogue", "binary"):
            output = self.root / (role + "-collection")
            argv = [sys.executable, "-B", str(SCRIPT),
                    "--binary", str(fifo if role == "binary" else Path(sys.executable).resolve()),
                    "--git", str(git_path()),
                    "--catalogue", str(fifo if role == "catalogue" else CATALOGUE),
                    "--output-dir", str(output), "--collection-id", "fifo-rejection",
                    "--tool-version", "0.2.0", "--source-generation", "1" * 40,
                    "--provenance", "source_build", "--timeout", "0.05"]
            with self.subTest(role=role):
                observed = subprocess.run(argv, cwd=self.root, capture_output=True, timeout=2)
                self.assertNotEqual(observed.returncode, 0, observed.stdout)
                self.assertIn(b"regular file", observed.stderr)
                self.assertFalse(output.exists())
                self.assertTrue(fifo.exists())

    @unittest.skipUnless(hasattr(os, "mkfifo"), "replacement control requires Unix FIFO support")
    def test_regular_to_fifo_replacement_at_open_refuses_without_blocking(self):
        source = self.root / "source.json"
        source.write_bytes(b"prior regular owner\n")
        # Patch only the disposable child's final open operation. This places
        # the replacement after any production preflight, for both the old
        # Path.open route and a descriptor-based opener.
        code = r'''
import importlib.util, io, os, pathlib, sys
from unittest import mock
script, target = map(pathlib.Path, sys.argv[1:])
spec = importlib.util.spec_from_file_location("collector", script)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
original_io_open, original_os_open = io.open, os.open
prior = target.with_name("retained-prior-owner")
swapped = False
def swap(path):
    global swapped
    if isinstance(path, int):
        return  # Wrapping an already opened descriptor is not a new path open.
    if not swapped and pathlib.Path(path) == target:
        target.rename(prior)
        os.mkfifo(target)
        swapped = True
def io_open(path, *args, **kwargs):
    swap(path)
    return original_io_open(path, *args, **kwargs)
def os_open(path, *args, **kwargs):
    swap(path)
    return original_os_open(path, *args, **kwargs)
with mock.patch.object(io, "open", side_effect=io_open), mock.patch.object(os, "open", side_effect=os_open):
    try:
        module.regular_bytes(target, 1024)
    except ValueError as error:
        print(str(error))
    else:
        raise AssertionError("replaced nonregular input was accepted")
assert swapped and prior.read_bytes() == b"prior regular owner\n"
'''
        observed = subprocess.run([sys.executable, "-B", "-c", code, str(SCRIPT), str(source)],
                                  cwd=self.root, capture_output=True, timeout=2)
        self.assertEqual(observed.returncode, 0, observed.stderr.decode(errors="replace"))
        self.assertIn(b"regular file", observed.stdout)
        self.assertEqual((self.root / "retained-prior-owner").read_bytes(), b"prior regular owner\n")


@unittest.skipIf(NATIVE is None, "native binary supplied by the Rust integration target")
class NativeAdmissionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = Path(tempfile.mkdtemp(prefix="command-case-native-tests-")).resolve()
        cls.root_identity = cls.root.lstat()
        args = arguments(cls.root, NATIVE)
        with redirect_stdout(io.StringIO()):
            COLLECT.collect(args)
        cls.collection = args.output_dir
        cls.context = json.loads((cls.collection / "expected-context.json").read_bytes())
        cls.bundle = json.loads((cls.collection / "bundle.json").read_bytes())

    @classmethod
    def tearDownClass(cls):
        try:
            preserve_candidate_profiles(cls.collection, os.environ.get("LLVM_PROFILE_FILE"))
        except Exception as error:
            # No automatic TemporaryDirectory finalizer: failed profile
            # custody must leave the actual fixture/profile bytes available.
            raise RuntimeError(f"candidate profile export failed; fixtures retained at {cls.root}") from error
        cleanup = remove_owned_fixture(cls.root, cls.root_identity)
        if cleanup["status"] == "retained_readonly":
            print(f"remaining fixture retained at {cleanup['root']}: "
                  f"observed readonly leaf {cleanup['failed_leaf']}; unlink refused "
                  f"(errno={cleanup['errno']}, winerror={cleanup['winerror']})", file=sys.stderr)

    def setUp(self):
        self.restore = {}
        self.remove = []

    def tearDown(self):
        for path in self.remove:
            path.unlink()
        for path, data in self.restore.items():
            path.write_bytes(data)

    def selected(self, ids):
        context, bundle = copy.deepcopy(self.context), copy.deepcopy(self.bundle)
        context["cases"] = [case for case in context["cases"] if case["case_id"] in ids]
        bundle["cases"] = [case for case in bundle["cases"] if case["context"]["case_id"] in ids]
        return context, bundle

    def read_member(self, member):
        return (self.collection / member["path"]).read_bytes()

    def replace_member(self, context, case, role, data, repin=True):
        member = case[role]
        path = self.collection / member["path"]
        self.restore.setdefault(path, path.read_bytes())
        path.write_bytes(data)
        member["size_bytes"], member["digest"] = len(data), COLLECT.digest(data)
        if repin:
            case["context"]["output_digests"][role] = member["digest"]
            expected = next(item for item in context["cases"] if item["case_id"] == case["context"]["case_id"])
            expected["output_digests"][role] = member["digest"]

    def admit(self, context, bundle, catalogue=None, raw_context=None, raw_bundle=None, output=None, timeout=30):
        context_bytes = COLLECT.json_bytes(context) if raw_context is None else raw_context
        bundle["context_digest"] = COLLECT.digest(context_bytes)
        bundle_bytes = COLLECT.json_bytes(bundle) if raw_bundle is None else raw_bundle
        context_path = self.collection / "test-context.json"
        bundle_path = self.collection / "test-bundle.json"
        catalogue_path = CATALOGUE
        if catalogue is not None:
            catalogue_path = self.collection / "test-catalogue.json"
            catalogue_path.write_bytes(catalogue)
        context_path.write_bytes(context_bytes)
        bundle_path.write_bytes(bundle_bytes)
        argv = [str(NATIVE), "command-migration-evidence", "--catalogue", str(catalogue_path),
                "--expected-context", str(context_path), "--bundle", str(bundle_path)]
        if output is not None:
            argv.extend(["--output", str(output)])
        env = dict(os.environ)
        for key in list(env):
            if key.startswith("GIT_") or key.startswith("CARGO_ALLOW"):
                del env[key]
        observed = subprocess.run(argv, cwd=self.collection, env=env, capture_output=True, timeout=timeout)
        if output is not None:
            return observed, None
        self.assertTrue(observed.stdout, observed.stderr.decode(errors="replace"))
        return observed, json.loads(observed.stdout)

    def invalid(self, context, bundle, text=None, **kwargs):
        observed, result = self.admit(context, bundle, **kwargs)
        self.assertEqual(observed.returncode, 1, observed.stderr.decode(errors="replace"))
        self.assertEqual(result["semantic_validity"], "invalid", result)
        self.assertEqual(result["qualification"], "partial")
        if text:
            self.assertIn(text, json.dumps(result))
        return result

    @unittest.skipUnless(os.name == "nt", "native Windows absolute/verbatim path semantics")
    def test_windows_absolute_verbatim_and_drive_relative_reader_paths(self):
        context, bundle = self.selected(["A.audit.healthy_policy"])
        context_bytes = COLLECT.json_bytes(context)
        bundle["context_digest"] = COLLECT.digest(context_bytes)
        context_path = self.collection / "windows-context.json"
        bundle_path = self.collection / "windows-bundle.json"
        context_path.write_bytes(context_bytes)
        bundle_path.write_bytes(COLLECT.json_bytes(bundle))
        self.remove.extend([context_path, bundle_path])

        def ordinary(path):
            value = str(path)
            if value.startswith("\\\\?\\UNC\\"):
                return "\\\\" + value[8:]
            return value[4:] if value.startswith("\\\\?\\") else value

        def verbatim(path):
            value = ordinary(path)
            return "\\\\?\\UNC\\" + value[2:] if value.startswith("\\\\") else "\\\\?\\" + value

        env = {key: value for key, value in os.environ.items()
               if not key.startswith(("GIT_", "CARGO_ALLOW"))}
        for label, form in (("ordinary", ordinary), ("verbatim", verbatim)):
            argv = [str(NATIVE), "command-migration-evidence", "--catalogue", form(CATALOGUE),
                    "--expected-context", form(context_path), "--bundle", form(bundle_path)]
            output = self.collection / ("windows-" + label + "-result.json")
            with self.subTest(form=label):
                self.assertTrue(Path(form(CATALOGUE)).is_absolute())
                observed = subprocess.run(argv, cwd=self.collection, env=env, capture_output=True, timeout=30)
                self.assertEqual(observed.returncode, 0, observed.stderr.decode(errors="replace"))
                admission = json.loads(observed.stdout)
                self.assertEqual(admission["semantic_validity"], "valid", admission)
                self.assertEqual(admission["qualification"], "partial")
                self.assertTrue(admission["cases"][0]["detail_reconciled"])
                observed = subprocess.run(argv + ["--output", form(output)], cwd=self.collection,
                                          env=env, capture_output=True, timeout=30)
                self.assertEqual(observed.returncode, 0, observed.stderr.decode(errors="replace"))
                self.remove.append(output)
                self.assertEqual(json.loads(output.read_bytes()), admission)
                prior = output.read_bytes()
                observed = subprocess.run(argv + ["--output", form(output)], cwd=self.collection,
                                          env=env, capture_output=True, timeout=30)
                self.assertNotEqual(observed.returncode, 0)
                self.assertIn(b"already exists", observed.stderr)
                self.assertEqual(output.read_bytes(), prior)

        # A foreign drive prefix remains relative even after cwd.join().
        drive = "Z:" if Path(ordinary(self.collection)).drive.upper() != "Z:" else "Y:"
        relative = drive + "unanchored-reader.json"
        self.assertFalse(Path(relative).is_absolute())
        for role in ("catalogue", "expected-context", "bundle", "output"):
            inputs = {"catalogue": ordinary(CATALOGUE), "expected-context": ordinary(context_path),
                      "bundle": ordinary(bundle_path)}
            inputs[role] = relative
            argv = [str(NATIVE), "command-migration-evidence"]
            for name, path in inputs.items():
                argv.extend(["--" + name, path])
            with self.subTest(drive_relative=role):
                observed = subprocess.run(argv, cwd=self.collection, env=env, capture_output=True, timeout=30)
                self.assertNotEqual(observed.returncode, 0)
                self.assertIn(b"absolute input/member path", observed.stderr)

    @unittest.skipUnless(os.name == "nt", "native Windows junction/reparse point semantics")
    def test_windows_junction_inputs_members_and_outputs_are_refused(self):
        foreign = self.root / "foreign-junction-target"
        foreign.mkdir()
        prior = foreign / "prior-owner"
        prior.write_bytes(b"outside junction canary\n")
        catalogue = foreign / "catalogue.json"
        catalogue.write_bytes(CATALOGUE.read_bytes())
        context, bundle = self.selected(["A.audit.healthy_policy"])
        detail = foreign / "detail.json"
        detail.write_bytes(self.read_member(bundle["cases"][0]["detail"]))
        junction = self.collection / "junction"
        cmd = Path(os.environ["SystemRoot"]) / "System32/cmd.exe"
        made = subprocess.run([str(cmd), "/d", "/c", "mklink", "/J", str(junction), str(foreign)],
                              capture_output=True, timeout=10)
        self.assertEqual(made.returncode, 0, made.stderr.decode(errors="replace"))
        self.addCleanup(os.rmdir, junction)
        self.assertTrue(junction.lstat().st_file_attributes & 0x0400)
        # The member route must emit its typed Invalid result, not fail while
        # reading the three independent top-level JSON inputs.
        bundle["cases"][0]["detail"]["path"] = "junction/detail.json"
        self.invalid(context, bundle, "symlink input/member refused")
        context, bundle = self.selected(["A.audit.healthy_policy"])
        observed, _ = self.admit(context, bundle, output=junction / "new-result.json")
        self.assertNotEqual(observed.returncode, 0)
        self.assertIn(b"symlink input/member refused", observed.stderr)
        self.assertFalse((foreign / "new-result.json").exists())
        observed = subprocess.run([str(NATIVE), "command-migration-evidence", "--catalogue",
                                   str(junction / "catalogue.json"), "--expected-context",
                                   str(self.collection / "test-context.json"), "--bundle",
                                   str(self.collection / "test-bundle.json")],
                                  cwd=self.collection, capture_output=True, timeout=30)
        self.assertNotEqual(observed.returncode, 0)
        self.assertIn(b"symlink input/member refused", observed.stderr)
        self.assertEqual(prior.read_bytes(), b"outside junction canary\n")
        self.assertEqual(catalogue.read_bytes(), CATALOGUE.read_bytes())

    def test_actual_first_family_valid_observations_remain_partial_qualification(self):
        # These live routes work both before and after the separately owned
        # #4460 sidecar repair. Its three domain-error routes are not silently
        # assumed fixed here; their full producer proof is retained separately.
        ids = [case["context"]["case_id"] for case in self.bundle["cases"]
               if not any(part in case["context"]["case_id"] for part in
                          ("instrument_failure", "invalid_policy", "partial_require_clean"))
               and case["context"]["case_id"] != "A.adopt.partial_inventory"
               and not case["context"]["case_id"].startswith("A.check.")]
        ids.extend(["A.check.healthy_policy", "A.check.new_finding", "A.check.partial_inventory"])
        context, bundle = self.selected(ids)
        observed, result = self.admit(context, bundle)
        self.assertEqual(observed.returncode, 0, result)
        self.assertEqual(result["semantic_validity"], "valid", result)
        self.assertEqual(result["qualification"], "partial")
        self.assertEqual(len(result["dimensions"]), 18)
        self.assertGreater(len(result["missing_cases"]), 70)
        self.assertTrue(all(item["detail_reconciled"] for item in result["cases"]))
        by_id = {item["case_id"]: item for item in result["cases"]}
        self.assertEqual(by_id["A.check.healthy_policy"]["observed_result_class"], "findings")
        self.assertEqual(by_id["A.doctor.new_finding"]["observed_result_class"], "completed")
        self.assertEqual(by_id["A.check.partial_inventory"]["observed_completeness"], "partial")
        self.assertTrue(all(item["binding_gaps"] for item in result["cases"]))
        if EXPORT is not None:
            EXPORT.mkdir()
            for name, data in [("catalogue", CATALOGUE.read_bytes()), ("context", COLLECT.json_bytes(context)),
                               ("bundle", COLLECT.json_bytes(bundle)), ("admission", observed.stdout)]:
                (EXPORT / (name + ".json")).write_bytes(data)

    def test_actual_hard_failures_retain_typed_error_receipt_and_no_detail(self):
        ids = ["A." + command + ".instrument_failure" for command in ("adopt", "doctor", "audit", "check")]
        ids += ["A.check.clean_no_policy", "A.check.findings_no_policy", "A.check.invalid_policy", "A.audit.invalid_policy"]
        context, bundle = self.selected(ids)
        observed, result = self.admit(context, bundle)
        self.assertEqual(observed.returncode, 0, result)
        self.assertEqual(result["semantic_validity"], "incomplete", result)
        self.assertTrue(all(item["process_observed"] and not item["detail_reconciled"] for item in result["cases"]))
        self.assertTrue(all(item["semantic_validity"] == "incomplete" for item in result["cases"]))

    def test_missing_duplicate_unknown_required_cases_and_false_na_are_not_admitted(self):
        for mode in ("missing", "duplicate", "unknown", "false_na"):
            context, bundle = self.selected(["A.audit.healthy_policy"])
            if mode == "missing":
                bundle["cases"] = []
            elif mode == "duplicate":
                bundle["cases"].append(copy.deepcopy(bundle["cases"][0]))
            elif mode == "unknown":
                bundle["cases"][0]["context"]["case_id"] = "A.unknown"
            else:
                bundle["not_applicable"] = ["A.audit.healthy_policy"]
            with self.subTest(mode=mode):
                self.invalid(context, bundle)

    def test_shrunken_catalogue_and_schema_or_duplicate_json_are_rejected(self):
        context, bundle = self.selected(["A.audit.healthy_policy"])
        smaller = json.loads(CATALOGUE.read_bytes())
        smaller["cases"] = smaller["cases"][:1]
        catalogue = COLLECT.json_bytes(smaller)
        context["catalogue_digest"] = bundle["catalogue_digest"] = COLLECT.digest(catalogue)
        self.invalid(context, bundle, "accepted denominator", catalogue=catalogue)
        context, bundle = self.selected(["A.audit.healthy_policy"])
        bundle["schema_version"] = 2
        self.invalid(context, bundle, "schema generation")
        context, bundle = self.selected(["A.audit.healthy_policy"])
        self.invalid(context, bundle, "duplicate JSON", raw_bundle=b'{"schema_id":"one","schema_id":"two"}')

    def test_binary_source_install_context_and_ambient_selector_mismatches(self):
        for key, value in [("digest", "sha256:v1:" + "a" * 64), ("source_generation", "a" * 40),
                           ("path", str(self.root / "foreign")), ("tool_version", "99.0")]:
            context, bundle = self.selected(["A.audit.healthy_policy"])
            bundle["binary"][key] = value
            with self.subTest(binary_key=key):
                self.invalid(context, bundle, "identity mismatch")
        for key, value in [("cwd", str(self.root)), ("mode", "audit"), ("profile", "invented"),
                           ("resolved_config_identity", "sha256:v1:" + "a" * 64),
                           ("policy_digest", "sha256:v1:" + "a" * 64)]:
            context, bundle = self.selected(["A.audit.healthy_policy"])
            bundle["cases"][0]["context"][key] = value
            with self.subTest(case_key=key):
                self.invalid(context, bundle, "identity mismatch")
        context, bundle = self.selected(["A.audit.healthy_policy"])
        for item in [context["cases"][0], bundle["cases"][0]["context"]]:
            item["environment"]["GIT_DIR"] = "/foreign"
        self.invalid(context, bundle, "ambient")
        context, bundle = self.selected(["A.audit.healthy_policy"])
        for binary in (context["binary"], bundle["binary"]):
            binary["install_identity"] = "sha256:v1:" + "a" * 64
        self.invalid(context, bundle, "source build")

    def test_owned_profiler_environment_and_legacy_absence(self):
        context, bundle = self.selected(["A.audit.clean_no_policy"])
        expected = str(Path(context["cases"][0]["root"]) / "target/command-case/profiles/candidate-%m-%p.profraw")
        self.assertEqual(context["cases"][0]["environment"].get("LLVM_PROFILE_FILE"), expected)
        for value in (str(self.root / "foreign-%p.profraw"),
                      str(Path(context["cases"][0]["root"]) / "target/command-case/foreign-%p.profraw"),
                      expected.replace("candidate-%m-%p", "other-%m-%p")):
            changed_context, changed_bundle = self.selected(["A.audit.clean_no_policy"])
            for item in (changed_context["cases"][0], changed_bundle["cases"][0]["context"]):
                item["environment"]["LLVM_PROFILE_FILE"] = value
            with self.subTest(profile=value):
                self.invalid(changed_context, changed_bundle, "profile destination")
        # Older uninstrumented producers did not record this optional value.
        # This checks readback compatibility, not a second candidate run.
        for item in (context["cases"][0], bundle["cases"][0]["context"]):
            del item["environment"]["LLVM_PROFILE_FILE"]
        observed, result = self.admit(context, bundle)
        self.assertEqual(observed.returncode, 0, result)
        self.assertEqual(result["semantic_validity"], "valid", result)
        self.assertEqual(result["qualification"], "partial", result)

    def test_rehashed_foreign_detail_is_rejected_even_after_transport_repin(self):
        context, bundle = self.selected(["A.doctor.healthy_policy"])
        case = bundle["cases"][0]
        foreign = next(item for item in self.bundle["cases"] if item["context"]["case_id"] == "A.doctor.partial_inventory")
        self.replace_member(context, case, "detail", self.read_member(foreign["detail"]))
        self.invalid(context, bundle)

    def replace_report(self, context, case, detail, summary):
        summary["subject"]["repository_identity"] = report_identity(detail)
        self.replace_member(context, case, "detail", COLLECT.json_bytes(detail))
        self.replace_member(context, case, "summary", COLLECT.json_bytes(summary))

    def test_unrequested_receipts_are_refused_for_successful_and_incomplete_non_check_cases(self):
        foreign = next(case for case in self.bundle["cases"]
                       if case["context"]["case_id"] == "A.check.healthy_policy")
        payloads = (b"not even a JSON receipt\n", self.read_member(foreign["receipt"]))
        for command in ("adopt", "doctor", "audit"):
            for state in ("healthy", "missing_summary", "cancelled"):
                for index, payload in enumerate(payloads):
                    with self.subTest(command=command, state=state, payload=index):
                        context, bundle = self.selected(["A." + command + ".healthy_policy"])
                        case = bundle["cases"][0]
                        self.assertIsNone(case["receipt"])
                        path = self.collection / "unrequested-receipt.json"
                        path.write_bytes(payload)
                        self.remove.append(path)
                        member = {"path": path.name, "size_bytes": len(payload), "digest": COLLECT.digest(payload)}
                        case["receipt"] = member
                        case["context"]["output_digests"]["receipt"] = context["cases"][0]["output_digests"]["receipt"] = member["digest"]
                        if state == "missing_summary":
                            case["summary"] = None
                            case["context"]["output_digests"]["summary"] = context["cases"][0]["output_digests"]["summary"] = None
                        elif state == "cancelled":
                            case["process"].update(exit_code=-9, timed_out=True)
                        self.invalid(context, bundle)
                    self.tearDown()
                    self.setUp()

    def test_clean_audit_rejects_invented_roadmap_and_repair_queue_after_semantic_repin(self):
        invented = {"signal": "invented_approval", "label": "Publish without further checks",
                    "route_kind": "command", "item_kind": None, "worklist_status": None,
                    "worklist_filter": None, "count": 99, "command": "cargo-allow release-identity --help"}
        for field in ("audit_remediation_roadmap", "evidence_repair_queues"):
            with self.subTest(field=field):
                context, bundle = self.selected(["A.audit.clean_no_policy"])
                case = bundle["cases"][0]
                detail = json.loads(self.read_member(case["detail"]))
                summary = json.loads(self.read_member(case["summary"]))
                self.assertEqual(summary["subject"]["repository_identity"], report_identity(detail))
                self.assertFalse(detail.get(field))
                detail[field] = [copy.deepcopy(invented)]
                self.replace_report(context, case, detail, summary)
                self.invalid(context, bundle)
            self.tearDown()
            self.setUp()

    def test_nonempty_audit_routes_validate_every_field_and_do_not_leak_to_check(self):
        mutations = {"signal": "expired", "label": "invented label", "route_kind": "prune_stale",
                     "item_kind": "expired_allow", "worklist_status": "expired", "worklist_filter": "weak_evidence",
                     "count": 99, "command": "cargo-allow release-identity --help"}
        for field in ("audit_remediation_roadmap", "evidence_repair_queues"):
            for key, value in mutations.items():
                with self.subTest(field=field, key=key):
                    context, bundle = self.selected(["A.audit.healthy_policy"])
                    case = bundle["cases"][0]
                    detail = json.loads(self.read_member(case["detail"]))
                    summary = json.loads(self.read_member(case["summary"]))
                    self.assertEqual(len(detail[field]), 1)
                    detail[field][0][key] = value
                    self.replace_report(context, case, detail, summary)
                    self.invalid(context, bundle)
                self.tearDown()
                self.setUp()
        context, bundle = self.selected(["A.check.healthy_policy"])
        case = bundle["cases"][0]
        detail = json.loads(self.read_member(case["detail"]))
        summary = json.loads(self.read_member(case["summary"]))
        detail["audit_remediation_roadmap"] = copy.deepcopy(detail["evidence_repair_queues"])
        self.replace_report(context, case, detail, summary)
        self.invalid(context, bundle)

    def test_check_receipt_metadata_requires_the_actual_fixture_posture(self):
        mutations = [("lifecycle_posture", "invented-approved"),
                     ("lifecycle_posture", "calendar-expiry-blocking"),
                     ("lane_posture", {"source-exception": "approved"}),
                     ("lane_posture", {"non_rust_file": "advisory"}),
                     ("federation", {"federation_version": "invented", "ledger_contributors": [], "precedence_applied": "invented"}),
                     ("federation", {"federation_version": "1", "ledger_contributors": [], "precedence_applied": "federation_registry"})]
        for field in ("lifecycle_posture", "lane_posture", "federation"):
            mutations.extend([(field, None), (field, "omit")])
        for field, value in mutations:
            with self.subTest(field=field, value=value):
                context, bundle = self.selected(["A.check.healthy_policy"])
                case = bundle["cases"][0]
                receipt = json.loads(self.read_member(case["receipt"]))
                self.assertEqual(receipt["lifecycle_posture"], "candidate-mode")
                self.assertEqual(receipt["lane_posture"], {"non_rust_file": "blocking"})
                self.assertEqual(receipt["federation"], {"federation_version": "1", "ledger_contributors": [], "precedence_applied": "cli_override"})
                if value == "omit":
                    del receipt[field]
                else:
                    receipt[field] = value
                self.replace_member(context, case, "receipt", COLLECT.json_bytes(receipt))
                self.invalid(context, bundle)
            self.tearDown()
            self.setUp()

    def test_coherent_suppression_or_inflation_cannot_erase_literal_policy_advisory(self):
        for command in ("audit", "check"):
            for replacement in (0, 2):
                with self.subTest(command=command, count=replacement):
                    context, bundle = self.selected(["A." + command + ".healthy_policy"])
                    case = bundle["cases"][0]
                    before = copy.deepcopy(case["before"])
                    after = copy.deepcopy(case["after"])
                    detail = json.loads(self.read_member(case["detail"]))
                    summary = json.loads(self.read_member(case["summary"]))
                    self.assertEqual(detail["summary"]["policy_missing_evidence"], 1)
                    self.assertEqual(summary["result_class"], "findings")
                    for counts in (detail["summary"], detail["trend"]):
                        if replacement:
                            counts["policy_missing_evidence"] = replacement
                        else:
                            del counts["policy_missing_evidence"]
                    detail["trend"]["review_items"] = replacement
                    for field in ("evidence_repair_queues", "audit_remediation_roadmap"):
                        if replacement:
                            for row in detail.get(field, []):
                                row["count"] = replacement
                        elif field == "audit_remediation_roadmap":
                            detail.pop(field, None)
                        else:
                            detail[field] = []
                    if replacement:
                        summary["reason"]["message"] = "2 advisory or review outcome(s) remain"
                    else:
                        summary["result_class"], summary["posture"] = "completed", "satisfied"
                        summary.pop("primary_action", None)
                        summary["reason"] = {"code": command + ".satisfied", "message": "the selected source-exception posture is satisfied"}
                    self.replace_report(context, case, detail, summary)
                    if command == "check":
                        receipt = json.loads(self.read_member(case["receipt"]))
                        for counts in (receipt["counts"], receipt["advisory"]):
                            if replacement:
                                counts["policy_missing_evidence"] = replacement
                            else:
                                del counts["policy_missing_evidence"]
                        receipt["advisory"]["review_items"] = replacement
                        receipt["evidence_repair_queues"] = copy.deepcopy(detail["evidence_repair_queues"])
                        self.replace_member(context, case, "receipt", COLLECT.json_bytes(receipt))
                    self.assertEqual(case["before"], before)
                    self.assertEqual(case["after"], after)
                    self.invalid(context, bundle)
                self.tearDown()
                self.setUp()

    def test_other_report_context_counters_cannot_be_defined_by_submitted_detail(self):
        for summary_key, trend_key in (("policy_baseline_debt", "baseline_debt"),
                                       ("broken_evidence_links", "broken_evidence_links"),
                                       ("weak_evidence_references", "weak_evidence_references"),
                                       (None, "occurrence_headroom")):
            with self.subTest(counter=trend_key):
                context, bundle = self.selected(["A.audit.healthy_policy"])
                case = bundle["cases"][0]
                detail = json.loads(self.read_member(case["detail"]))
                summary = json.loads(self.read_member(case["summary"]))
                if summary_key is not None:
                    detail["summary"][summary_key] = 1
                detail["trend"][trend_key] = 1
                detail["trend"]["review_items"] = 2
                if trend_key != "baseline_debt":
                    summary["reason"]["message"] = "2 advisory or review outcome(s) remain"
                self.replace_report(context, case, detail, summary)
                self.invalid(context, bundle)
            self.tearDown()
            self.setUp()

    @unittest.skipUnless(hasattr(os, "mkfifo"), "native bounded-file controls require Unix FIFO support")
    def test_native_fifo_inputs_detail_and_binary_members_refuse_without_blocking(self):
        fifo = self.collection / "unwritten-fifo"
        os.mkfifo(fifo)
        self.remove.append(fifo)
        for field in ("detail", "binary_member"):
            with self.subTest(member=field):
                context, bundle = self.selected(["A.audit.healthy_policy"])
                member = bundle["cases"][0][field] if field == "detail" else bundle[field]
                member["path"] = fifo.name
                self.invalid(context, bundle, "regular file", timeout=2)
        context, bundle = self.selected(["A.audit.healthy_policy"])
        context_path = self.collection / "fifo-context.json"
        bundle_path = self.collection / "fifo-bundle.json"
        context_bytes = COLLECT.json_bytes(context)
        context_path.write_bytes(context_bytes)
        bundle["context_digest"] = COLLECT.digest(context_bytes)
        bundle_path.write_bytes(COLLECT.json_bytes(bundle))
        for field in ("catalogue", "expected-context", "bundle"):
            with self.subTest(input=field):
                inputs = {"catalogue": CATALOGUE, "expected-context": context_path, "bundle": bundle_path}
                inputs[field] = fifo
                argv = [str(NATIVE), "command-migration-evidence"]
                for name, path in inputs.items():
                    argv.extend(["--" + name, str(path)])
                observed = subprocess.run(argv, cwd=self.collection, capture_output=True, timeout=2)
                self.assertNotEqual(observed.returncode, 0, observed.stdout)
                self.assertIn(b"regular file", observed.stderr)
        self.assertTrue(fifo.exists())

    def test_rendered_inventory_and_scanner_coverage_are_not_conflated(self):
        for command in ("doctor", "audit", "check"):
            context, bundle = self.selected(["A." + command + ".partial_inventory"])
            case = bundle["cases"][0]
            detail = json.loads(self.read_member(case["detail"]))
            detail["inventory"]["completeness"] = "partial" if command == "doctor" else "scoped"
            self.replace_member(context, case, "detail", COLLECT.json_bytes(detail))
            with self.subTest(command=command):
                self.invalid(context, bundle, "inventory subject or scope")
            self.tearDown()
            self.setUp()

    def test_rehashed_class_coverage_actions_effects_and_proof_contradictions(self):
        for key, value in [("result_class", "completed"), ("completeness", "partial"),
                           ("primary_action", None), ("next_proof", None),
                           ("mode", "strict"), ("profile", "invented")]:
            context, bundle = self.selected(["A.audit.healthy_policy"])
            case = bundle["cases"][0]
            summary = json.loads(self.read_member(case["summary"]))
            summary[key] = value
            self.replace_member(context, case, "summary", COLLECT.json_bytes(summary))
            with self.subTest(key=key):
                self.invalid(context, bundle)
            self.tearDown()
            self.setUp()
        context, bundle = self.selected(["A.audit.healthy_policy"])
        case = bundle["cases"][0]
        summary = json.loads(self.read_member(case["summary"]))
        summary["operation_effects"]["writes_repository"] = True
        summary["operation_effects"]["write_paths"] = ["policy/allow.toml"]
        self.replace_member(context, case, "summary", COLLECT.json_bytes(summary))
        self.invalid(context, bundle)

    def test_hard_error_receipt_and_claim_boundary_cannot_be_rehashed_into_authority(self):
        for role in ("receipt", "summary"):
            context, bundle = self.selected(["A.check.instrument_failure"])
            case = bundle["cases"][0]
            value = json.loads(self.read_member(case[role]))
            if role == "receipt":
                value["diagnostic"] = "foreign diagnostic"
            else:
                value["claim_boundary"]["statement"] = "release qualification"
            self.replace_member(context, case, role, COLLECT.json_bytes(value))
            with self.subTest(role=role):
                self.invalid(context, bundle)
            self.tearDown()
            self.setUp()

    def test_size_digest_truncation_member_alias_and_escape_are_rejected(self):
        for mode in ("size", "digest", "missing", "alias", "escape", "truncated"):
            context, bundle = self.selected(["A.audit.healthy_policy"])
            case = bundle["cases"][0]
            if mode == "size":
                case["detail"]["size_bytes"] += 1
            elif mode == "digest":
                case["detail"]["digest"] = "sha256:v1:" + "0" * 64
            elif mode == "missing":
                case["detail"]["path"] = "absent.json"
            elif mode == "alias":
                case["detail"] = copy.deepcopy(case["summary"])
            elif mode == "escape":
                case["detail"]["path"] = "../outside.json"
            else:
                self.replace_member(context, case, "detail", b'{"schema_id":')
            with self.subTest(mode=mode):
                self.invalid(context, bundle)
            self.tearDown()
            self.setUp()

    def test_retained_detail_summary_and_binary_hardlinks_are_refused(self):
        # Hard links need no symlink privilege on Windows. Every platform
        # must reach the ownership rejection with otherwise valid evidence.
        for role in ("detail", "summary", "binary"):
            with self.subTest(role=role):
                context, bundle = self.selected(["A.audit.healthy_policy"])
                member = (bundle["binary_member"] if role == "binary"
                          else bundle["cases"][0][role])
                original = self.collection / member["path"]
                before = original.stat()
                alias = self.collection / ("hardlink-" + role)
                os.link(original, alias)
                self.remove.append(alias)
                try:
                    self.assertGreater(original.stat().st_nlink, 1)
                    self.assertEqual((alias.stat().st_dev, alias.stat().st_ino),
                                     (before.st_dev, before.st_ino))
                    member["path"] = alias.name
                    self.invalid(context, bundle, "hard-link")
                    self.assertEqual(COLLECT.digest(alias.read_bytes()), member["digest"])
                    self.assertEqual(original.stat().st_mode, before.st_mode)
                finally:
                    alias.unlink()
                    self.remove.remove(alias)
                self.assertEqual(original.stat().st_nlink, before.st_nlink)

    @unittest.skipUnless(os.name != "nt", "symlink control requires Unix link support")
    def test_symlink_hardlink_and_new_output_prior_owner_are_preserved(self):
        context, bundle = self.selected(["A.audit.healthy_policy"])
        case = bundle["cases"][0]
        original = self.collection / case["detail"]["path"]
        alias = self.collection / "alias.json"
        alias.symlink_to(original)
        self.remove.append(alias)
        case["detail"]["path"] = "alias.json"
        self.invalid(context, bundle, "symlink")
        alias.unlink()
        self.remove.remove(alias)
        os.link(original, alias)
        self.remove.append(alias)
        self.invalid(context, bundle, "hard-link")
        alias.unlink()
        self.remove.remove(alias)
        prior = self.collection / "prior-result.json"
        prior.write_bytes(b"prior owner")
        observed, _ = self.admit(context, bundle, output=prior)
        self.assertNotEqual(observed.returncode, 0)
        self.assertEqual(prior.read_bytes(), b"prior owner")
        parent = self.collection / "output-parent"
        parent.symlink_to(self.root, target_is_directory=True)
        self.remove.append(parent)
        observed, _ = self.admit(context, bundle, output=parent / "escaped-result.json")
        self.assertNotEqual(observed.returncode, 0)
        self.assertFalse((self.root / "escaped-result.json").exists())

    def test_unstarted_cancelled_and_missing_sidecar_cannot_become_green(self):
        for mode in ("unstarted", "cancelled", "missing_summary"):
            context, bundle = self.selected(["A.audit.healthy_policy"])
            case = bundle["cases"][0]
            if mode == "unstarted":
                case["process"].update(started=False, exit_code=None, launch_error="test launch refused", timed_out=False)
                for role in ("detail", "summary", "receipt"):
                    case[role] = None
                    case["context"]["output_digests"][role] = context["cases"][0]["output_digests"][role] = None
            elif mode == "cancelled":
                case["process"].update(exit_code=-9, timed_out=True)
            else:
                case["summary"] = None
                case["context"]["output_digests"]["summary"] = context["cases"][0]["output_digests"]["summary"] = None
            observed, result = self.admit(context, bundle)
            with self.subTest(mode=mode):
                self.assertEqual(observed.returncode, 0, result)
                self.assertEqual(result["semantic_validity"], "incomplete", result)
                self.assertEqual(result["qualification"], "partial")
                self.assertFalse(result["cases"][0]["detail_reconciled"])
                self.assertIn("A.audit.healthy_policy", result["missing_cases"])

    def test_unexpected_retained_source_mutation_is_not_accepted(self):
        context, bundle = self.selected(["A.audit.healthy_policy"])
        case = bundle["cases"][0]
        snapshot = json.loads(self.read_member(case["after"]))
        source = next(item for item in snapshot["entries"] if item["path"] == "src/lib.rs")
        source["mode"] ^= 0o100
        path = self.collection / case["after"]["path"]
        self.restore[path] = path.read_bytes()
        data = COLLECT.json_bytes(snapshot)
        path.write_bytes(data)
        case["after"]["size_bytes"], case["after"]["digest"] = len(data), COLLECT.digest(data)
        self.invalid(context, bundle, "mutation")
        # Instrumentation outputs get no filename exemption in source
        # snapshots, even when an attacker repins every member digest.
        context, bundle = self.selected(["A.audit.healthy_policy"])
        case = bundle["cases"][0]
        snapshot = json.loads(self.restore[path])
        relative = "cases/A.audit.healthy_policy/after/files/default.profraw"
        member_path = self.collection / relative
        data = b"unexpected source profile\n"
        member_path.write_bytes(data)
        self.remove.append(member_path)
        snapshot["entries"].append({"path": "default.profraw", "kind": "file", "mode": 0o600,
                                    "size_bytes": len(data), "link_target": None,
                                    "content": {"path": relative, "size_bytes": len(data),
                                                "digest": COLLECT.digest(data)}})
        data = COLLECT.json_bytes(snapshot)
        path.write_bytes(data)
        case["after"]["size_bytes"], case["after"]["digest"] = len(data), COLLECT.digest(data)
        self.invalid(context, bundle, "unexpected source file")


def main():
    global NATIVE, SOURCE_GENERATION, EXPORT
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-bin", type=Path)
    parser.add_argument("--source-generation")
    parser.add_argument("--export-observations", type=Path)
    args = parser.parse_args()
    NATIVE = args.native_bin.resolve() if args.native_bin is not None else None
    SOURCE_GENERATION = args.source_generation
    EXPORT = args.export_observations
    NativeAdmissionTests.__unittest_skip__ = NATIVE is None
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(TransportTests)
    if NATIVE is not None:
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(NativeAdmissionTests))
    return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())
