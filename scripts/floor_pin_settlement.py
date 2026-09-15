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


@dataclass(frozen=True)
class LockSnapshot:
    resolved: Mapping[str, str]
    identity: str


Attempt = Callable[[Mapping[str, str]], tuple[int, str]]
Snapshot = Callable[[], LockSnapshot]


def _base_version(version: str | None) -> str:
    return (version or "").split("+", 1)[0]


def _is_exact(snapshot: LockSnapshot, row: Mapping[str, str]) -> bool:
    return _base_version(snapshot.resolved.get(row["package"])) == row["floor"]


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
    """

    ordered = sorted((dict(row) for row in floors), key=lambda row: row["package"])
    if not ordered:
        return {}

    pass_limit = max_passes if max_passes is not None else max(2, len(ordered) * 2 + 1)
    if pass_limit < 1:
        raise ValueError("max_passes must be positive")

    diagnostics: dict[str, str] = {}
    seen_states: set[tuple[tuple[tuple[str, str], ...], str]] = set()

    for _ in range(pass_limit):
        for row in ordered:
            if _is_exact(snapshot(), row):
                continue
            returncode, stderr = attempt(row)
            name = row["package"]
            if returncode == 0:
                diagnostics.pop(name, None)
            else:
                diagnostics[name] = stderr.strip()[:MAX_DIAGNOSTIC_BYTES]

        current = snapshot()
        unresolved = [row for row in ordered if not _is_exact(current, row)]
        if not unresolved:
            return {}

        state = (
            tuple(
                (
                    row["package"],
                    _base_version(current.resolved.get(row["package"])),
                )
                for row in ordered
            ),
            current.identity,
        )
        if state in seen_states:
            break
        seen_states.add(state)

    current = snapshot()
    failures: dict[str, str] = {}
    for row in ordered:
        if _is_exact(current, row):
            continue
        name = row["package"]
        observed = _base_version(current.resolved.get(name)) or "missing"
        failures[name] = diagnostics.get(
            name,
            "pin did not remain at declared floor: "
            f"locked at {observed}, floor requires {row['floor']}",
        )[:MAX_DIAGNOSTIC_BYTES]
    return failures


def read_lock_snapshot(lock_path: Path) -> LockSnapshot:
    if not lock_path.exists():
        return LockSnapshot({}, "missing")

    raw = lock_path.read_bytes().replace(b"\r", b"")
    lock = tomllib.loads(raw.decode("utf-8"))
    resolved: dict[str, str] = {}
    for package in lock.get("package", []):
        resolved.setdefault(package["name"], package["version"])
    return LockSnapshot(resolved, hashlib.sha256(raw).hexdigest())


def main(argv: list[str]) -> int:
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
        proc = subprocess.run(
            ["cargo", "update", "-p", row["package"], "--precise", row["floor"]],
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
