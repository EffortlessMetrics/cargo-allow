#!/usr/bin/env python3
"""Settle all selected direct dependency floors into one Cargo.lock."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tomllib
from typing import Callable, Iterable, Mapping

MAX_DIAGNOSTIC_BYTES = 300
CRATES_IO_SOURCES = (
    "registry+https://github.com/rust-lang/crates.io-index",
    "sparse+https://index.crates.io/",
)


@dataclass(frozen=True)
class LockedPackage:
    """One exact Cargo package identity retained by Cargo.lock."""

    version: str
    source: str | None


@dataclass(frozen=True)
class LockSnapshot:
    """All same-name lock identities plus the complete lock digest."""

    resolved: Mapping[str, tuple[LockedPackage, ...]]
    identity: str


Attempt = Callable[[Mapping[str, str]], tuple[int, str]]
Snapshot = Callable[[], LockSnapshot]
ObservedState = tuple[tuple[str, tuple[LockedPackage, ...]], ...]


def base_version(version: str | None) -> str:
    """Return the SemVer precedence version without build metadata."""

    return (version or "").split("+", 1)[0]


def locked_occurrences(
    snapshot: LockSnapshot, package: str
) -> tuple[LockedPackage, ...]:
    """Return every exact lock identity for a package name."""

    return snapshot.resolved.get(package, ())


def is_registry_source(source: str | None) -> bool:
    """Return whether a lock source is registry-backed."""

    return bool(source) and source.startswith(("registry+", "sparse+"))


def source_identity(package: LockedPackage | None) -> str:
    """Project a locked source into the public receipt vocabulary."""

    if package is None:
        return "unresolved"
    if package.source in CRATES_IO_SOURCES or (
        package.source
        and (
            "crates.io-index" in package.source
            or "index.crates.io" in package.source
        )
    ):
        return "registry:crates.io"
    return package.source or "path:workspace-or-local"


def _format_identity(package: LockedPackage) -> str:
    source = package.source or "path:workspace-or-local"
    return f"{package.version} @ {source}"


def selected_registry_identity(
    snapshot: LockSnapshot, row: Mapping[str, str]
) -> tuple[LockedPackage | None, str | None]:
    """Select one registry identity or return a fail-closed reason."""

    name = row["package"]
    occurrences = locked_occurrences(snapshot, name)
    if not occurrences:
        return None, f"{name} is missing from Cargo.lock"
    if len(occurrences) != 1:
        rendered = ", ".join(_format_identity(item) for item in occurrences)
        return None, f"{name} has multiple locked identities: {rendered}"
    selected = occurrences[0]
    if not is_registry_source(selected.source):
        return (
            selected,
            f"{name} selected a non-registry lock identity: "
            f"{_format_identity(selected)}",
        )
    return selected, None


def floor_identity(
    snapshot: LockSnapshot, row: Mapping[str, str]
) -> tuple[LockedPackage | None, str | None]:
    """Return the unique registry identity only when it is at the floor."""

    selected, issue = selected_registry_identity(snapshot, row)
    if issue:
        return selected, issue
    assert selected is not None
    observed = base_version(selected.version)
    if observed != row["floor"]:
        return (
            selected,
            "pin did not remain at declared floor: "
            f"locked at {observed}, floor requires {row['floor']}",
        )
    return selected, None


def cargo_update_spec(
    snapshot: LockSnapshot, row: Mapping[str, str]
) -> tuple[str | None, str | None]:
    """Choose a non-ambiguous Cargo package spec for one update attempt."""

    occurrences = locked_occurrences(snapshot, row["package"])
    if not occurrences:
        # The first update bootstraps a freshly removed lock. Cargo still
        # fails closed if the generated graph makes this name ambiguous.
        return row["package"], None
    selected, issue = selected_registry_identity(snapshot, row)
    if issue:
        return None, issue
    assert selected is not None
    return f"{row['package']}@{selected.version}", None


def _is_exact(snapshot: LockSnapshot, row: Mapping[str, str]) -> bool:
    _, issue = floor_identity(snapshot, row)
    return issue is None


def _attempt_failure_diagnostic(
    row: Mapping[str, str], returncode: int, stderr: str
) -> str:
    detail = stderr.strip()
    if detail:
        return detail[:MAX_DIAGNOSTIC_BYTES]
    return (
        f"cargo update failed with exit code {returncode} while pinning "
        f"{row['package']} to {row['floor']}; stderr was empty"
    )[:MAX_DIAGNOSTIC_BYTES]


def settle_floors(
    floors: Iterable[Mapping[str, str]],
    attempt: Attempt,
    snapshot: Snapshot,
    *,
    max_passes: int | None = None,
) -> dict[str, str]:
    """Return final bounded diagnostics for floors that cannot settle together.

    The caller's ``attempt`` mutates the candidate lock. Rows are sorted by
    package for deterministic execution, but every unresolved row is retried
    after sibling pins have had a chance to settle. Repeated lock states and a
    bounded pass count make an incompatible or oscillating set terminate.
    A selected name must end at exactly one registry-backed name/version/source
    identity at its floor; ambiguous or substituted identities remain non-clean.
    """

    ordered = sorted((dict(row) for row in floors), key=lambda row: row["package"])
    if not ordered:
        return {}

    pass_limit = max_passes if max_passes is not None else max(2, len(ordered) * 2 + 1)
    if pass_limit < 1:
        raise ValueError("max_passes must be positive")

    diagnostics: dict[str, str] = {}
    seen_states: set[tuple[ObservedState, str]] = set()

    for _ in range(pass_limit):
        for row in ordered:
            if _is_exact(snapshot(), row):
                continue
            returncode, stderr = attempt(row)
            name = row["package"]
            if returncode == 0:
                diagnostics.pop(name, None)
            else:
                diagnostics[name] = _attempt_failure_diagnostic(
                    row, returncode, stderr
                )

        current = snapshot()
        unresolved = [row for row in ordered if not _is_exact(current, row)]
        if not unresolved:
            return {}

        observed: ObservedState = tuple(
            (row["package"], locked_occurrences(current, row["package"]))
            for row in ordered
        )
        state = (observed, current.identity)
        if state in seen_states:
            break
        seen_states.add(state)

    current = snapshot()
    failures: dict[str, str] = {}
    for row in ordered:
        selected, issue = selected_registry_identity(current, row)
        if issue:
            failures[row["package"]] = diagnostics.get(
                row["package"], issue
            )[:MAX_DIAGNOSTIC_BYTES]
            continue
        assert selected is not None
        if base_version(selected.version) == row["floor"]:
            continue
        floor_issue = (
            "pin did not remain at declared floor: "
            f"locked at {base_version(selected.version)}, floor requires {row['floor']}"
        )
        failures[row["package"]] = diagnostics.get(
            row["package"], floor_issue
        )[:MAX_DIAGNOSTIC_BYTES]
    return failures


def read_lock_snapshot(lock_path: Path) -> LockSnapshot:
    """Read every name/version/source occurrence from a Cargo lockfile."""

    if not lock_path.exists():
        return LockSnapshot({}, "missing")

    raw = lock_path.read_bytes().replace(b"\r", b"")
    lock = tomllib.loads(raw.decode("utf-8"))
    observed: dict[str, list[LockedPackage]] = {}
    for package in lock.get("package", []):
        source = package.get("source")
        observed.setdefault(package["name"], []).append(
            LockedPackage(
                version=package["version"],
                source=source if isinstance(source, str) else None,
            )
        )
    resolved = {
        name: tuple(
            sorted(packages, key=lambda item: (item.version, item.source or ""))
        )
        for name, packages in observed.items()
    }
    return LockSnapshot(resolved, hashlib.sha256(raw).hexdigest())


def main(argv: list[str]) -> int:
    """Run bounded settlement and write final per-package failures."""

    if len(argv) != 4:
        print(
            "usage: floor_pin_settlement.py "
            "<floors.json> <pin-failures.json> <Cargo.lock>",
            file=sys.stderr,
        )
        return 2

    floors_path = Path(argv[1])
    failures_path = Path(argv[2])
    lock_path = Path(argv[3])
    floors = json.loads(floors_path.read_text(encoding="utf-8"))

    def attempt(row: Mapping[str, str]) -> tuple[int, str]:
        package_spec, issue = cargo_update_spec(
            read_lock_snapshot(lock_path), row
        )
        if issue:
            return 2, issue
        assert package_spec is not None
        proc = subprocess.run(
            [
                "cargo",
                "update",
                "-p",
                package_spec,
                "--precise",
                row["floor"],
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        return proc.returncode, proc.stderr

    failures = settle_floors(
        floors,
        attempt,
        lambda: read_lock_snapshot(lock_path),
    )
    failures_path.write_text(
        json.dumps(failures, indent=1) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))