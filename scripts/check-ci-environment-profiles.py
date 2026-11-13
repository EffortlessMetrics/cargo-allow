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


def job_env(block: str) -> dict[str, str]:
    """Return the job-level ``env:`` string mapping."""
    env: dict[str, str] = {}
    match = re.search(r"^    env:\s*$", block, re.MULTILINE)
    if not match:
        return env
    remainder = block[match.end() :]
    stop = re.search(r"^    [A-Za-z0-9_-]+:", remainder, re.MULTILINE)
    section = remainder[: stop.start() if stop else len(remainder)]
    for key, value in re.findall(r'^      (\w+):\s*"([^"]*)"\s*$', section, re.MULTILINE):
        env[key] = value
    return env


def resolve_toolchain(value: str, env: dict[str, str]) -> str | None:
    """Resolve a literal or ``${{ env.NAME }}`` toolchain value."""
    value = value.strip().strip('"')
    match = re.fullmatch(r"\$\{\{\s*env\.(\w+)\s*\}\}", value)
    if match:
        return env.get(match.group(1))
    return value


def declared_toolchains(block: str, env: dict[str, str]) -> list[str | None]:
    """Return the effective RUSTUP_TOOLCHAIN and toolchain: input values.

    RUSTUP_TOOLCHAIN counts only in the job-level env mapping: a step-local
    assignment is a per-command override, not the lane's selected toolchain.
    """
    values: list[str | None] = []
    if "RUSTUP_TOOLCHAIN" in env:
        values.append(env["RUSTUP_TOOLCHAIN"])
    for match in re.finditer(r"^\s*toolchain:\s*(.+?)\s*$", block, re.MULTILINE):
        values.append(resolve_toolchain(match.group(1), env))
    return values


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
        env = job_env(block)
        declared = declared_toolchains(block, env)
        if profile["toolchain"] == "not-selected":
            if any(value is not None for value in declared):
                raise SystemExit(
                    f"error: {profile['id']} declares a toolchain {declared} "
                    "but the manifest says not-selected"
                )
            continue

        resolved = sorted({value for value in declared if value is not None})
        if resolved:
            mismatched = [value for value in resolved if value != profile["toolchain"]]
            if mismatched:
                raise SystemExit(
                    f"error: {profile['id']} declares toolchain {mismatched} "
                    f"but the manifest says {profile['toolchain']}"
                )
            continue

        selector = f"dtolnay/rust-toolchain@{profile['toolchain']}"
        if selector not in block and f"toolchain: {profile['toolchain']}" not in block:
            raise SystemExit(f"error: {profile['id']} toolchain {profile['toolchain']} is not declared")
    print(f"validated {len(profiles)} CI environment profiles")
    return 0


if __name__ == "__main__":
    sys.exit(main())
