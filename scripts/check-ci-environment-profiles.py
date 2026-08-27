#!/usr/bin/env python3
"""Validate the selected CI environment inventory against workflow source."""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "docs/ci-environment-profiles.toml"
POSTURES = {
    "ExactToolchainQualification",
    "FixedMajorRunnerObservation",
    "MovingStableCompatibilityCanary",
    "MovingRunnerCompatibilityCanary",
    "PlatformCharacterization",
    "UnsupportedOrNotSelected",
}
REQUIRED = {"id", "workflow", "job", "posture", "runner", "toolchain", "result_posture", "claim"}


def parse_profiles(text: str) -> list[dict[str, str]]:
    profiles: list[dict[str, str]] = []
    current: dict[str, str] | None = None
    for line in text.splitlines():
        if line.strip() == "[[profile]]":
            if current is not None:
                profiles.append(current)
            current = {}
            continue
        if current is None or not line.strip() or line.lstrip().startswith("#"):
            continue
        match = re.fullmatch(r'(\w+)\s*=\s*"([^"]*)"', line.strip())
        if match:
            current[match.group(1)] = match.group(2)
    if current is not None:
        profiles.append(current)
    return profiles


def workflow_jobs(text: str) -> set[str]:
    return {match.group(1) for match in re.finditer(r"^  ([A-Za-z0-9_-]+):\s*$", text, re.MULTILINE)}


def job_block(text: str, job: str) -> str:
    match = re.search(rf"^  {re.escape(job)}:\s*$", text, re.MULTILINE)
    if not match:
        return ""
    remainder = text[match.end() :]
    next_job = re.search(r"^  [A-Za-z0-9_-]+:\s*$", remainder, re.MULTILINE)
    return remainder[: next_job.start() if next_job else len(remainder)]


def main() -> int:
    profiles = parse_profiles(MANIFEST.read_text(encoding="utf-8"))
    if not profiles:
        raise SystemExit("error: no CI environment profiles found")
    seen: set[str] = set()
    for profile in profiles:
        missing = REQUIRED - profile.keys()
        if missing:
            raise SystemExit(f"error: {profile.get('id', '<unknown>')} missing {sorted(missing)}")
        if profile["id"] in seen:
            raise SystemExit(f"error: duplicate profile id: {profile['id']}")
        seen.add(profile["id"])
        if profile["posture"] not in POSTURES:
            raise SystemExit(f"error: unsupported posture: {profile['posture']}")
        workflow = ROOT / profile["workflow"]
        if not workflow.is_file():
            raise SystemExit(f"error: missing workflow: {profile['workflow']}")
        source = workflow.read_text(encoding="utf-8")
        if profile["job"] not in workflow_jobs(source):
            raise SystemExit(f"error: {profile['id']} names missing job {profile['job']}")
        block = job_block(source, profile["job"])
        if profile["runner"] not in block:
            raise SystemExit(f"error: {profile['id']} runner {profile['runner']} is not declared")
        if profile["toolchain"] != "not-selected":
            selector = f"dtolnay/rust-toolchain@{profile['toolchain']}"
            if selector not in block and f"toolchain: {profile['toolchain']}" not in block:
                raise SystemExit(f"error: {profile['id']} toolchain {profile['toolchain']} is not declared")
    print(f"validated {len(profiles)} CI environment profiles")
    return 0


if __name__ == "__main__":
    sys.exit(main())
