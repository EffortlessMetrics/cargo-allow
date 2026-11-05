#!/usr/bin/env python3
"""Collect bounded #3149 A-family observations from an explicitly supplied binary.

This is a transport harness, not a semantic evaluator. It runs each selected
case once, retains the command's own outputs (including absence), and snapshots
the controlled fixture. Native command-migration-evidence performs admission.
The emitted expected-context file must be pinned by its caller; this collector
is not a trusted-worker, installation, or release-qualification authority.
"""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys


JSON_LIMIT = 8 * 1024 * 1024
BINARY_LIMIT = 512 * 1024 * 1024
FIRST_FAMILY_COMMANDS = {"adopt", "doctor", "audit", "check"}
MAX_SELECTED_CASES = 29
SCHEMA = "cargo-allow.command-case-bundle.v1"
# Read-only #4724 presentation routes. Each entry is (route, requires_tty).
# human_receipt owns the receipt-only human check route on a real terminal
# boundary plus its JSON-route receipt/detail correspondence authority.
PRESENTATION_ROUTES = {
    "human_receipt": {
        "check": [
            ("receipt_only", False),
            ("receipt_only_tty", True),
            ("json", False),
        ],
    },
    "presentation": {
        "adopt": [("human", False), ("json", False), ("quiet", False), ("file", False), ("tty", True)],
        "doctor": [("human", False), ("json", False), ("quiet", False), ("file", False), ("tty", True)],
        "audit": [("human", False), ("json", False), ("quiet", False), ("file", False), ("tty", True)],
        "check": [("human", False), ("json", False), ("quiet", False), ("file", False), ("tty", True)],
    },
}


def presentation_routes_for(spec):
    """The accepted route set for a case, or None outside the reader."""
    if spec.get("family") != "A" or spec.get("first_family_collector"):
        return None
    return PRESENTATION_ROUTES.get(spec.get("scenario"), {}).get(spec.get("command"))


def digest(data):
    return "sha256:v1:" + hashlib.sha256(data).hexdigest()


def json_bytes(value):
    return (json.dumps(value, indent=2, ensure_ascii=False, sort_keys=True) + "\n").encode()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON key: " + key)
        result[key] = value
    return result


def load_json(path):
    raw = regular_bytes(path, JSON_LIMIT)
    return json.loads(raw, object_pairs_hook=unique_object), raw


def utc_now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat().replace("+00:00", "Z")


def reject_symlinks(path):
    if not path.is_absolute() or ".." in path.parts:
        raise ValueError("an absolute, unambiguous path is required: " + str(path))
    for item in reversed((path,) + tuple(path.parents)):
        if item.is_symlink():
            raise ValueError("symlink path refused: " + str(item))


def regular_bytes(path, limit):
    reject_symlinks(path)
    named_before = path.lstat()
    if not stat.S_ISREG(named_before.st_mode) or named_before.st_size > limit:
        raise ValueError("not a bounded regular file: " + str(path))
    flags = os.O_RDONLY | getattr(os, "O_BINARY", 0)
    if os.name == "posix":
        if not hasattr(os, "O_NONBLOCK") or not hasattr(os, "O_NOFOLLOW"):
            raise ValueError("bounded regular file opening is unsupported on this host")
        flags |= os.O_NONBLOCK | os.O_NOFOLLOW
    # Preflight rejects existing special files. Nonblocking/no-follow open on
    # Unix also refuses a FIFO or symlink substituted after that inspection.
    descriptor = os.open(path, flags)
    with os.fdopen(descriptor, "rb") as handle:
        before = os.fstat(handle.fileno())
        if not stat.S_ISREG(before.st_mode) or before.st_size > limit:
            raise ValueError("not a bounded regular file: " + str(path))
        if file_identity(named_before, cross_view=True) != file_identity(before, cross_view=True):
            raise ValueError("file changed before open: " + str(path))
        data = handle.read(limit + 1)
        after = os.fstat(handle.fileno())
    reject_symlinks(path)
    named = path.lstat()
    if (file_identity(before) != file_identity(after)
            or file_identity(named_before) != file_identity(named)
            or file_identity(after, cross_view=True) != file_identity(named, cross_view=True)
            or len(data) != before.st_size):
        raise ValueError("file changed while reading: " + str(path))
    return data


def file_identity(metadata, *, cross_view=False):
    mode = metadata.st_mode
    if cross_view and os.name == "nt":
        # CPython infers these bits from .exe/.bat/.cmd/.com names, while
        # descriptor stat has no filename. Keep full modes within each view.
        mode &= ~0o111
    return (metadata.st_dev, metadata.st_ino, mode, metadata.st_nlink,
            metadata.st_size, metadata.st_mtime_ns)


def safe_relative(value):
    if not value or "\\" in value or ":" in value or any(
        part in ("", ".", "..") for part in value.split("/")
    ):
        raise ValueError("unsafe member path: " + repr(value))
    return value


class Store:
    def __init__(self, root):
        self.root = root

    def put(self, relative, data):
        safe_relative(relative)
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        reject_symlinks(path.parent)
        with path.open("xb") as handle:
            handle.write(data)
        observed = regular_bytes(path, max(JSON_LIMIT, len(data)))
        if observed != data:
            raise ValueError("retained member readback mismatch: " + relative)
        return {"path": relative, "size_bytes": len(data), "digest": digest(data)}

    def capture(self, relative, source):
        return self.put(relative, regular_bytes(source, JSON_LIMIT))


def owned_root(path):
    path = path.absolute()
    reject_symlinks(path.parent)
    if "%" in str(path):
        raise ValueError("collection root cannot contain an LLVM profile filename pattern")
    if any((parent / ".git").exists() for parent in (path.parent,) + tuple(path.parents)):
        raise ValueError("collection fixtures must be outside a repository checkout")
    path.mkdir()  # Exclusive ownership: existing files, directories and links all fail.
    return path.resolve()


def child_environment(home, git):
    if not git.is_absolute():
        raise ValueError("child Git executable must have an absolute path")
    # Windows' default search path includes the current directory. Retained
    # evidence requires absolute components, with the selected Git first.
    defaults = [entry for entry in os.defpath.split(os.pathsep) if Path(entry).is_absolute()]
    path = os.pathsep.join(dict.fromkeys([str(git.parent)] + defaults))
    home.mkdir()
    cargo_home = home / "cargo"
    cargo_home.mkdir()
    env = {
        "PATH": path,
        "HOME": str(home),
        "USERPROFILE": str(home),
        "CARGO_HOME": str(cargo_home),
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_TERMINAL_PROMPT": "0",
        "GIT_OPTIONAL_LOCKS": "0",
        "LANG": "C",
        "LC_ALL": "C",
        "NO_COLOR": "1",
    }
    # Windows process creation needs its actual system directory. This is a
    # recorded child-local value, never an inherited policy or Git selector.
    if os.name == "nt":
        env["SYSTEMROOT"] = os.environ.get("SYSTEMROOT", "C:\\Windows")
        env["TEMP"] = str(home)
        env["TMP"] = str(home)
    return env


def run_process(argv, cwd, environment, timeout, store, prefix):
    out_path = store.root / (prefix + ".stdout")
    err_path = store.root / (prefix + ".stderr")
    out_path.parent.mkdir(parents=True, exist_ok=True)
    started_at = utc_now()
    started, exit_code, launch_error, timed_out = False, None, None, False
    with out_path.open("xb") as stdout, err_path.open("xb") as stderr:
        try:
            process = subprocess.Popen(
                argv, cwd=cwd, env=environment, stdin=subprocess.DEVNULL,
                stdout=stdout, stderr=stderr,
            )
            started = True
            try:
                exit_code = process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                timed_out = True
                process.kill()
                exit_code = process.wait()
        except OSError as error:
            launch_error = str(error)
    result = {
        "started_at_utc": started_at,
        "finished_at_utc": utc_now(),
        "started": started,
        "exit_code": exit_code,
        "launch_error": launch_error,
        "timed_out": timed_out,
    }
    members = []
    for path in (out_path, err_path):
        data = regular_bytes(path, JSON_LIMIT)
        members.append({
            "path": path.relative_to(store.root).as_posix(),
            "size_bytes": len(data), "digest": digest(data),
        })
    return result, members[0], members[1]


def run_process_tty(argv, cwd, environment, timeout, store, prefix):
    """Run argv attached to a real pseudo-terminal and retain the console relay.

    A real terminal merges both command streams into one console stream, so
    the retained stdout member holds the relayed console bytes and the stderr
    member is empty. When the host provides no usable pseudo-terminal the
    invocation is recorded as never started instead of silently falling back
    to pipes: presentation evidence never fabricates a terminal observation.
    """
    out_path = store.root / (prefix + ".stdout")
    err_path = store.root / (prefix + ".stderr")
    out_path.parent.mkdir(parents=True, exist_ok=True)
    started_at = utc_now()
    if os.name == "nt":
        data, started, exit_code, launch_error, timed_out = _conpty_run(
            argv, cwd, environment, timeout
        )
    else:
        data, started, exit_code, launch_error, timed_out = _posix_pty_run(
            argv, cwd, environment, timeout
        )
    with out_path.open("xb") as stdout, err_path.open("xb") as stderr:
        stdout.write(data)
    result = {
        "started_at_utc": started_at,
        "finished_at_utc": utc_now(),
        "started": started,
        "exit_code": exit_code,
        "launch_error": launch_error,
        "timed_out": timed_out,
    }
    members = []
    for path in (out_path, err_path):
        retained = regular_bytes(path, JSON_LIMIT)
        members.append({
            "path": path.relative_to(store.root).as_posix(),
            "size_bytes": len(retained), "digest": digest(retained),
        })
    return result, members[0], members[1]


def _posix_pty_run(argv, cwd, environment, timeout):
    import pty
    import threading
    try:
        master_fd, slave_fd = pty.openpty()
    except OSError as error:
        return b"", False, None, "pseudo-terminal unavailable: " + str(error), False
    try:
        process = subprocess.Popen(
            argv, cwd=cwd, env=environment, stdin=slave_fd,
            stdout=slave_fd, stderr=slave_fd, close_fds=True,
        )
    except OSError as error:
        os.close(slave_fd)
        os.close(master_fd)
        return b"", False, None, str(error), False
    started = True
    os.close(slave_fd)
    chunks = []

    def pump():
        while True:
            try:
                piece = os.read(master_fd, 65536)
            except OSError:
                break
            if not piece:
                break
            chunks.append(piece)

    reader = threading.Thread(target=pump, daemon=True)
    reader.start()
    timed_out = False
    try:
        exit_code = process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        timed_out = True
        process.kill()
        exit_code = process.wait()
    reader.join(timeout=10)
    try:
        os.close(master_fd)
    except OSError:
        pass
    return b"".join(chunks), started, exit_code, None, timed_out


def _conpty_run(argv, cwd, environment, timeout):
    """Run argv on a real Windows pseudo-terminal (ConPTY) via stdlib ctypes.

    The merged console relay bytes are captured from the ConPTY output pipe.
    A failure to create the terminal or process is returned as a launch error;
    the invocation is then recorded as never started.
    """
    import ctypes
    import threading
    from ctypes import wintypes

    class COORD(ctypes.Structure):
        _fields_ = [("X", ctypes.c_short), ("Y", ctypes.c_short)]

    class STARTUPINFOW(ctypes.Structure):
        _fields_ = [
            ("cb", wintypes.DWORD), ("lpReserved", wintypes.LPWSTR),
            ("lpDesktop", wintypes.LPWSTR), ("lpTitle", wintypes.LPWSTR),
            ("dwX", wintypes.DWORD), ("dwY", wintypes.DWORD),
            ("dwXSize", wintypes.DWORD), ("dwYSize", wintypes.DWORD),
            ("dwXCountChars", wintypes.DWORD), ("dwYCountChars", wintypes.DWORD),
            ("dwFillAttribute", wintypes.DWORD), ("dwFlags", wintypes.DWORD),
            ("wShowWindow", wintypes.WORD), ("cbReserved2", wintypes.WORD),
            ("lpReserved2", ctypes.POINTER(ctypes.c_byte)),
            ("hStdInput", wintypes.HANDLE), ("hStdOutput", wintypes.HANDLE),
            ("hStdError", wintypes.HANDLE),
        ]

    class STARTUPINFOEXW(ctypes.Structure):
        _fields_ = [
            ("StartupInfo", STARTUPINFOW), ("lpAttributeList", ctypes.c_void_p),
        ]

    class SECURITY_ATTRIBUTES(ctypes.Structure):
        _fields_ = [
            ("nLength", wintypes.DWORD),
            ("lpSecurityDescriptor", ctypes.c_void_p),
            ("bInheritHandle", wintypes.BOOL),
        ]

    class PROCESS_INFORMATION(ctypes.Structure):
        _fields_ = [
            ("hProcess", wintypes.HANDLE), ("hThread", wintypes.HANDLE),
            ("dwProcessId", wintypes.DWORD), ("dwThreadId", wintypes.DWORD),
        ]

    kernel32 = ctypes.WinDLL("kernel32.dll", use_last_error=True)
    handle = wintypes.HANDLE
    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE = 0x00020016
    CREATE_UNICODE_ENVIRONMENT = 0x00000400
    CREATE_EXTENDED_STARTUPINFO_PRESENT = 0x00080000
    WAIT_TIMEOUT = 0x00000102
    ERROR_INSUFFICIENT_BUFFER = 122

    kernel32.CreatePipe.argtypes = [
        ctypes.POINTER(handle), ctypes.POINTER(handle),
        ctypes.POINTER(SECURITY_ATTRIBUTES), wintypes.DWORD,
    ]
    kernel32.CreatePseudoConsole.argtypes = [
        COORD, handle, handle, wintypes.DWORD, ctypes.POINTER(ctypes.c_void_p),
    ]
    kernel32.CloseHandle.argtypes = [handle]
    kernel32.InitializeProcThreadAttributeList.argtypes = [
        ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD,
        ctypes.POINTER(ctypes.c_size_t),
    ]
    kernel32.UpdateProcThreadAttribute.argtypes = [
        ctypes.c_void_p, wintypes.DWORD, ctypes.c_size_t,
        ctypes.POINTER(ctypes.c_void_p),
        ctypes.c_size_t, ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p),
    ]
    kernel32.CreateProcessW.argtypes = [
        wintypes.LPCWSTR, wintypes.LPWSTR, ctypes.c_void_p, ctypes.c_void_p,
        wintypes.BOOL, wintypes.DWORD, ctypes.c_void_p, wintypes.LPCWSTR,
        ctypes.POINTER(STARTUPINFOEXW), ctypes.POINTER(PROCESS_INFORMATION),
    ]
    kernel32.WaitForSingleObject.argtypes = [handle, wintypes.DWORD]
    kernel32.GetExitCodeProcess.argtypes = [handle, ctypes.POINTER(wintypes.DWORD)]
    kernel32.TerminateProcess.argtypes = [handle, wintypes.UINT]
    kernel32.ReadFile.argtypes = [
        handle, ctypes.c_char_p, wintypes.DWORD,
        ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p,
    ]

    def fail(launch_error, owned=()):
        for descriptor in owned:
            kernel32.CloseHandle(descriptor)
        return b"", False, None, launch_error, False

    security = SECURITY_ATTRIBUTES()
    security.nLength = ctypes.sizeof(SECURITY_ATTRIBUTES)
    security.bInheritHandle = True
    stdin_read, stdin_write, stdout_read, stdout_write = (
        handle(), handle(), handle(), handle(),
    )
    if not kernel32.CreatePipe(
        ctypes.byref(stdin_read), ctypes.byref(stdin_write),
        ctypes.byref(security), 0,
    ):
        return fail("CreatePipe failed: " + str(ctypes.get_last_error()))
    if not kernel32.CreatePipe(
        ctypes.byref(stdout_read), ctypes.byref(stdout_write),
        ctypes.byref(security), 0,
    ):
        return fail(
            "CreatePipe failed: " + str(ctypes.get_last_error()),
            owned=(stdin_read, stdin_write),
        )
    terminal = ctypes.c_void_p()
    # CreatePseudoConsole returns an HRESULT, not a BOOL: zero is success.
    hresult = kernel32.CreatePseudoConsole(
        COORD(120, 30), stdin_read, stdout_write, 0, ctypes.byref(terminal),
    )
    if hresult != 0:
        return fail(
            f"CreatePseudoConsole failed: HRESULT {hresult:#010x}",
            owned=(stdin_read, stdin_write, stdout_read, stdout_write),
        )
    attribute_size = ctypes.c_size_t(0)
    kernel32.InitializeProcThreadAttributeList(None, 1, 0, ctypes.byref(attribute_size))
    if ctypes.get_last_error() != ERROR_INSUFFICIENT_BUFFER or not attribute_size.value:
        return fail(
            "InitializeProcThreadAttributeList sizing failed",
            owned=(stdin_read, stdin_write, stdout_read, stdout_write),
        )
    attribute_buffer = ctypes.create_string_buffer(attribute_size.value)
    attribute_list = ctypes.cast(attribute_buffer, ctypes.c_void_p)
    if not kernel32.InitializeProcThreadAttributeList(
        attribute_list, 1, 0, ctypes.byref(attribute_size)
    ):
        return fail(
            "InitializeProcThreadAttributeList failed: " + str(ctypes.get_last_error()),
            owned=(stdin_read, stdin_write, stdout_read, stdout_write),
        )
    if not kernel32.UpdateProcThreadAttribute(
        attribute_list, 0, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
        ctypes.byref(terminal), ctypes.sizeof(terminal), None, None,
    ):
        kernel32.DeleteProcThreadAttributeList(attribute_list)
        return fail(
            "UpdateProcThreadAttribute failed: " + str(ctypes.get_last_error()),
            owned=(stdin_read, stdin_write, stdout_read, stdout_write),
        )
    startup = STARTUPINFOEXW()
    startup.StartupInfo.cb = ctypes.sizeof(STARTUPINFOEXW)
    startup.lpAttributeList = attribute_list
    command_line = subprocess.list2cmdline([str(argument) for argument in argv])
    environment_block = "\0".join(
        f"{key}={value}"
        for key, value in sorted(environment.items(), key=lambda item: item[0].upper())
    ) + "\0"
    environment_bytes = environment_block.encode("utf-16-le") + b"\0"
    information = PROCESS_INFORMATION()
    if not kernel32.CreateProcessW(
        None, command_line, None, None, False,
        CREATE_UNICODE_ENVIRONMENT | CREATE_EXTENDED_STARTUPINFO_PRESENT,
        environment_bytes, str(cwd), ctypes.byref(startup),
        ctypes.byref(information),
    ):
        kernel32.DeleteProcThreadAttributeList(attribute_list)
        return fail(
            "CreateProcessW failed: " + str(ctypes.get_last_error()),
            owned=(stdin_read, stdin_write, stdout_read, stdout_write),
        )
    # The pseudo-terminal owns these ends now; the parent must release them
    # before the child sees a coherent console.
    kernel32.CloseHandle(stdin_read)
    kernel32.CloseHandle(stdout_write)
    kernel32.CloseHandle(stdin_write)
    started = True
    chunks = []

    def pump():
        buffer = ctypes.create_string_buffer(65536)
        read = wintypes.DWORD()
        while True:
            if not kernel32.ReadFile(stdout_read, buffer, 65536, ctypes.byref(read), None):
                break
            if read.value == 0:
                break
            chunks.append(buffer.raw[: read.value])

    reader = threading.Thread(target=pump, daemon=True)
    reader.start()
    waited = kernel32.WaitForSingleObject(information.hProcess, int(timeout * 1000))
    timed_out = waited == WAIT_TIMEOUT
    if timed_out:
        kernel32.TerminateProcess(information.hProcess, 1)
        kernel32.WaitForSingleObject(information.hProcess, 10000)
    code = wintypes.DWORD()
    kernel32.GetExitCodeProcess(information.hProcess, ctypes.byref(code))
    exit_code = code.value if code.value < 2**31 else code.value - 2**31 * 2
    # Closing the pseudo-terminal flushes and ends the relay; the pump then
    # observes end-of-file.
    kernel32.ClosePseudoConsole(terminal)
    reader.join(timeout=10)
    kernel32.CloseHandle(stdout_read)
    kernel32.DeleteProcThreadAttributeList(attribute_list)
    kernel32.CloseHandle(information.hProcess)
    kernel32.CloseHandle(information.hThread)
    # A working ConPTY host always relays at least its own terminal framing.
    # Zero relay bytes are recorded faithfully: the native reader treats an
    # empty retained TTY stream as an unproven terminal observation instead
    # of terminal evidence.
    return b"".join(chunks), started, exit_code, None, timed_out


def setup_git(root, git, env, timeout, store, prefix):
    commands = [
        [str(git), "init", "--quiet"],
        [str(git), "add", "--", "src/lib.rs"],
    ]
    if (root / "policy/allow.toml").exists():
        commands.append([str(git), "add", "--", "policy/allow.toml"])
    commands.append([
        str(git), "-c", "user.name=command-case-fixture",
        "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false",
        "commit", "--quiet", "--message", "controlled command fixture",
    ])
    setup_env = dict(env)
    setup_env["GIT_AUTHOR_DATE"] = "2000-01-01T00:00:00Z"
    setup_env["GIT_COMMITTER_DATE"] = "2000-01-01T00:00:00Z"
    for number, argv in enumerate(commands):
        result, stdout, stderr = run_process(
            argv, root, setup_env, timeout, store, f"{prefix}/git-{number}"
        )
        store.put(f"{prefix}/git-{number}.json", json_bytes({
            "argv": argv, "cwd": str(root), "environment": setup_env,
            "process": result, "stdout": stdout, "stderr": stderr,
        }))
        if result["exit_code"] != 0:
            raise ValueError("Git fixture setup failed; raw setup members retained")
    result, stdout, stderr = run_process(
        [str(git), "rev-parse", "--verify", "HEAD"], root, env, timeout,
        store, prefix + "/git-head",
    )
    if result["exit_code"] != 0:
        raise ValueError("Git fixture HEAD observation failed")
    head = regular_bytes(store.root / stdout["path"], JSON_LIMIT).decode("ascii").strip()
    if len(head) not in (40, 64) or any(c not in "0123456789abcdef" for c in head):
        raise ValueError("Git returned an invalid object identity")
    store.put(prefix + "/git-head.json", json_bytes({
        "argv": [str(git), "rev-parse", "--verify", "HEAD"],
        "cwd": str(root), "environment": env, "process": result,
        "stdout": stdout, "stderr": stderr,
    }))
    return head


def snapshot(root, store, prefix):
    entries = []
    for parent, directories, files in os.walk(root, followlinks=False):
        parent = Path(parent)
        if parent == root:
            directories[:] = [name for name in directories if name != "target"]
        directories.sort()
        for name in sorted(directories + files):
            path = parent / name
            relative = path.relative_to(root).as_posix()
            safe_relative(relative)
            metadata = path.lstat()
            entry = {
                "path": relative, "kind": "other",
                "mode": stat.S_IMODE(metadata.st_mode), "size_bytes": 0,
                "content": None, "link_target": None,
            }
            if stat.S_ISLNK(metadata.st_mode):
                entry["kind"] = "symlink"
                entry["link_target"] = os.readlink(path)
            elif stat.S_ISDIR(metadata.st_mode):
                entry["kind"] = "directory"
            elif stat.S_ISREG(metadata.st_mode):
                entry["kind"] = "file"
                entry["content"] = store.capture(prefix + "/files/" + relative, path)
                entry["size_bytes"] = entry["content"]["size_bytes"]
            entries.append(entry)
            if len(entries) > 512:
                raise ValueError("controlled fixture exceeds the bounded snapshot")
    entries.sort(key=lambda item: item["path"])
    return store.put(prefix + "/snapshot.json", json_bytes({"entries": entries}))


def capture_optional(store, prefix, role, path, blocked_directory=False):
    reject_symlinks(path)
    if not path.exists() and not path.is_symlink():
        return None
    if blocked_directory and path.is_dir():
        # Only this deliberately owned directory represents expected absence.
        # Other nonregular paths fail collection; they are not silent absence.
        return None
    if not path.is_file():
        raise ValueError("unexpected nonregular output: " + str(path))
    return store.capture(prefix + "/" + role + ".json", path)


def collect_case(spec, fixture, root, store, binary, git, timeout):
    case_id = spec["id"]
    prefix = "cases/" + safe_relative(case_id)
    fixture_root = root / "fixtures" / case_id
    fixture_root.mkdir(parents=True)
    (fixture_root / "src").mkdir()
    (fixture_root / "src/lib.rs").write_bytes(fixture["source"].encode("utf-8"))
    if fixture["policy"] is not None:
        (fixture_root / "policy").mkdir()
        (fixture_root / "policy/allow.toml").write_bytes(fixture["policy"].encode("utf-8"))
    env = child_environment(root / "homes" / case_id, git)
    fixture_commit = setup_git(fixture_root, git, env, timeout, store, prefix + "/setup")
    outputs = fixture_root / "target/command-case"
    outputs.mkdir(parents=True)
    profiles = outputs / "profiles"
    profiles.mkdir()
    # An instrumented supplied binary must not write a default profile among
    # the fixture's source files. Bind its output to this invocation's owned
    # directory, never to an inherited runner path. Uninstrumented binaries
    # ignore the value; no second candidate execution is needed.
    env["LLVM_PROFILE_FILE"] = str(profiles / "candidate-%m-%p.profraw")
    detail_path, summary_path = outputs / "detail.json", outputs / "summary.json"
    receipt_path = outputs / "receipt.json"
    output_guard = None
    if fixture["output_failure"]:
        detail_path.mkdir()
        (detail_path / "prior-owner.txt").write_bytes(b"prior owner\n")
        output_guard = {
            "before": store.capture(prefix + "/blocked-output-before.txt", detail_path / "prior-owner.txt"),
            "after": None,
        }
    argv = [
        str(binary), "--command-summary-output", str(summary_path), spec["command"],
        "--root", str(fixture_root), "--format", "json", "--output", str(detail_path),
    ]
    if fixture["policy"] is not None:
        argv.extend(["--config", "policy/allow.toml"])
    if spec["command"] == "check":
        argv.extend([
            "--mode", "no-new", "--persistent-cache", "off", "--receipt", str(receipt_path)
        ])
    if spec["command"] == "doctor" and fixture["require_clean"]:
        argv.append("--require-clean")
    before = snapshot(fixture_root, store, prefix + "/before")
    process, stdout, stderr = run_process(
        argv, fixture_root, env, timeout, store, prefix + "/command"
    )
    after = snapshot(fixture_root, store, prefix + "/after")
    detail = capture_optional(store, prefix, "detail", detail_path, fixture["output_failure"])
    summary = capture_optional(store, prefix, "summary", summary_path)
    receipt = capture_optional(store, prefix, "receipt", receipt_path)
    if output_guard is not None:
        output_guard["after"] = store.capture(
            prefix + "/blocked-output-after.txt", detail_path / "prior-owner.txt"
        )
    output_digests = {role: None if member is None else member["digest"] for role, member in [
        ("stdout", stdout), ("stderr", stderr), ("detail", detail), ("summary", summary), ("receipt", receipt)
    ]}
    context = {
        "case_id": case_id, "root": str(fixture_root), "cwd": str(fixture_root),
        "argv": argv, "environment": env, "source_snapshot_digest": before["digest"],
        "fixture_commit": fixture_commit, "output_digests": output_digests,
        "policy_digest": None if fixture["policy"] is None else digest(fixture["policy"].encode()),
        "config_path": None if fixture["policy"] is None else "policy/allow.toml",
        "mode": "no-new" if spec["command"] == "check" else None,
        "profile": None, "resolved_config_identity": None,
    }
    return {
        "context": context, "process": process, "before": before, "after": after,
        "stdout": stdout, "stderr": stderr, "detail": detail, "summary": summary, "receipt": receipt,
        "output_guard": output_guard,
    }


def collect_presentation_case(spec, fixture, root, store, binary, git, timeout):
    """Execute the accepted read-only presentation routes for one #4724 case.

    Every route runs as a separate actual invocation of the supplied binary
    against the same committed fixture, with its own before/after snapshots,
    retained streams and artifacts. The tty routes require a real
    pseudo-terminal; a host without one records those invocations as never
    started rather than substituting a pipe observation.
    """
    case_id = spec["id"]
    routes = PRESENTATION_ROUTES[spec["scenario"]][spec["command"]]
    fixture_root = root / "fixtures" / case_id
    fixture_root.mkdir(parents=True)
    (fixture_root / "src").mkdir()
    (fixture_root / "src/lib.rs").write_bytes(fixture["source"].encode("utf-8"))
    if fixture["policy"] is not None:
        (fixture_root / "policy").mkdir()
        (fixture_root / "policy/allow.toml").write_bytes(fixture["policy"].encode("utf-8"))
    env = child_environment(root / "homes" / case_id, git)
    fixture_commit = setup_git(
        fixture_root, git, env, timeout, store, "cases/" + safe_relative(case_id) + "/setup"
    )
    executions = []
    contexts = []
    for route, tty in routes:
        outputs = fixture_root / "target/command-case" / route
        outputs.mkdir(parents=True)
        (outputs / "profiles").mkdir()
        invocation_env = dict(env)
        invocation_env["LLVM_PROFILE_FILE"] = str(outputs / "profiles" / "candidate-%m-%p.profraw")
        summary_path = outputs / "summary.json"
        detail_path = outputs / "detail.json" if route == "json" else None
        report_path = outputs / "report.txt" if route == "file" else None
        receipt_path = outputs / "receipt.json" if spec["command"] == "check" else None
        argv = [
            str(binary), "--command-summary-output", str(summary_path), spec["command"],
            "--root", str(fixture_root),
        ]
        if fixture["policy"] is not None:
            argv.extend(["--config", "policy/allow.toml"])
        if route in ("human", "tty", "receipt_only", "receipt_only_tty"):
            argv.extend(["--format", "human"])
        elif route == "quiet":
            argv.extend(["--quiet", "--format", "human"])
        elif route == "json":
            argv.extend(["--format", "json", "--output", str(detail_path)])
        elif route == "file":
            argv.extend(["--format", "human", "--output", str(report_path)])
        if spec["command"] == "check":
            argv.extend([
                "--mode", "no-new", "--persistent-cache", "off", "--receipt", str(receipt_path),
            ])
        output_path = detail_path if route == "json" else report_path
        prefix = "presentation/" + safe_relative(case_id) + "/" + route
        before = snapshot(fixture_root, store, prefix + "/before")
        if tty:
            process, stdout, stderr = run_process_tty(
                argv, fixture_root, invocation_env, timeout, store, prefix + "/command"
            )
        else:
            process, stdout, stderr = run_process(
                argv, fixture_root, invocation_env, timeout, store, prefix + "/command"
            )
        after = snapshot(fixture_root, store, prefix + "/after")
        detail = capture_optional(store, prefix, "detail", output_path) if output_path else None
        summary = capture_optional(store, prefix, "summary", summary_path)
        receipt = capture_optional(store, prefix, "receipt", receipt_path) if receipt_path else None
        output_digests = {role: None if member is None else member["digest"] for role, member in [
            ("stdout", stdout), ("stderr", stderr), ("detail", detail),
            ("summary", summary), ("receipt", receipt),
        ]}
        context = {
            "route": route, "tty": tty, "root": str(fixture_root), "cwd": str(fixture_root),
            "argv": argv, "environment": invocation_env,
            "source_snapshot_digest": before["digest"], "fixture_commit": fixture_commit,
            "output_digests": output_digests,
            "policy_digest": None if fixture["policy"] is None else digest(fixture["policy"].encode()),
            "config_path": None if fixture["policy"] is None else "policy/allow.toml",
            "mode": "no-new" if spec["command"] == "check" else None,
            "profile": None, "resolved_config_identity": None,
        }
        contexts.append(context)
        executions.append({
            "context": context, "process": process, "before": before, "after": after,
            "stdout": stdout, "stderr": stderr, "detail": detail, "summary": summary,
            "receipt": receipt, "output_guard": None,
        })
    return {"case_id": case_id, "invocations": executions}, {
        "case_id": case_id, "invocations": contexts,
    }


def collect(args):
    catalogue, catalogue_bytes = load_json(args.catalogue.absolute())
    if catalogue.get("schema_id") != "cargo-allow.command-migration-catalogue.v1" or catalogue.get("schema_version") != 1:
        raise ValueError("unsupported catalogue generation")
    specs = {item["id"]: item for item in catalogue["cases"]}
    if len(specs) != len(catalogue["cases"]):
        raise ValueError("duplicate catalogue case")
    selected = args.case or [item["id"] for item in catalogue["cases"] if item["first_family_collector"]]
    if len(set(selected)) != len(selected) or not selected:
        raise ValueError("case selection is empty or duplicated")
    if any(
        case not in specs
        or not (
            specs[case]["first_family_collector"]
            or presentation_routes_for(specs[case]) is not None
        )
        for case in selected
    ):
        raise ValueError("unknown or unimplemented case requested")
    # This is an execution boundary, not semantic admission of the catalogue.
    # A rewritten collector flag must not dispatch a later mutation/release
    # command before the native reader can reject its denominator. The
    # read-only #4724 presentation route set is bounded to the same A-family
    # diagnostic commands.
    if len(selected) > MAX_SELECTED_CASES or any(
        specs[case]["family"] != "A" or specs[case]["command"] not in FIRST_FAMILY_COMMANDS
        for case in selected
    ):
        raise ValueError("selection exceeds the bounded read-only A-family transport")
    binary, git = args.binary.absolute(), args.git.absolute()
    if not args.binary.is_absolute() or not args.git.is_absolute():
        raise ValueError("--binary and --git must be explicit absolute paths")
    binary_bytes = regular_bytes(binary, BINARY_LIMIT)
    if not binary_bytes:
        raise ValueError("supplied binary is empty")
    regular_bytes(git, BINARY_LIMIT)
    if args.provenance == "supplied_installed_candidate" and not (args.candidate_identity and args.install_identity):
        raise ValueError("a supplied installed candidate requires explicit candidate and install identities")
    if args.provenance == "source_build" and (args.candidate_identity or args.install_identity):
        raise ValueError("a source build cannot claim installed-candidate identities")
    root = owned_root(args.output_dir)
    store = Store(root)
    (root / "homes").mkdir()
    binary_member = store.put("binary/cargo-allow", binary_bytes)
    binary_context = {
        "path": str(binary), "size_bytes": len(binary_bytes), "digest": digest(binary_bytes),
        "tool_version": args.tool_version, "provenance": args.provenance,
        "source_generation": args.source_generation, "candidate_identity": args.candidate_identity,
        "install_identity": args.install_identity,
    }
    store.put("catalogue.json", catalogue_bytes)
    cases = []
    presentation_executions = []
    presentation_contexts = []
    for case_id in selected:
        spec = specs[case_id]
        fixture = catalogue["first_family_fixtures"][spec["scenario"]]
        if spec["first_family_collector"]:
            cases.append(collect_case(
                spec, fixture, root, store, binary, git, args.timeout,
            ))
        else:
            execution, context = collect_presentation_case(
                spec, fixture, root, store, binary, git, args.timeout,
            )
            presentation_executions.append(execution)
            presentation_contexts.append(context)
    context = {
        "schema_id": "cargo-allow.command-case-context.v1", "schema_version": 1,
        "collection_id": args.collection_id, "catalogue_digest": digest(catalogue_bytes),
        "binary": binary_context, "cases": [case["context"] for case in cases],
    }
    if presentation_contexts:
        context["presentation_cases"] = presentation_contexts
    context_member = store.put("expected-context.json", json_bytes(context))
    bundle = {
        "schema_id": SCHEMA, "schema_version": 1, "collection_id": args.collection_id,
        "catalogue_digest": digest(catalogue_bytes), "context_digest": context_member["digest"],
        "binary": binary_context, "binary_member": binary_member,
        "binary_digest_after": digest(regular_bytes(binary, BINARY_LIMIT)), "cases": cases,
    }
    if presentation_executions:
        bundle["presentation_cases"] = presentation_executions
    store.put("bundle.json", json_bytes(bundle))
    print(json.dumps({
        "bundle": str(root / "bundle.json"), "expected_context": str(root / "expected-context.json"),
        "selected_cases": len(cases) + len(presentation_executions),
        "catalogue_cases": len(catalogue["cases"]),
        "semantic_admission": "not_performed", "qualification": "not_claimed",
    }, sort_keys=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--git", type=Path, required=True)
    parser.add_argument("--catalogue", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--collection-id", required=True)
    parser.add_argument("--tool-version", required=True)
    parser.add_argument("--source-generation", required=True)
    parser.add_argument("--provenance", choices=["source_build", "supplied_installed_candidate"], required=True)
    parser.add_argument("--candidate-identity")
    parser.add_argument("--install-identity")
    parser.add_argument("--case", action="append")
    parser.add_argument("--timeout", type=float, default=30.0)
    args = parser.parse_args()
    if args.timeout <= 0 or args.timeout > 300:
        parser.error("--timeout must be greater than zero and at most 300 seconds")
    try:
        collect(args)
    except (OSError, ValueError, KeyError, TypeError) as error:
        print("command-case collection failed: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
