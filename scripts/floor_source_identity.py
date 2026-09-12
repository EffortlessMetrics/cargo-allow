"""Commit only a derived floor lock in an owned detached proof checkout."""

import json
import subprocess
import sys


def git(*arguments):
    result = subprocess.run(
        ["git", *arguments], capture_output=True, text=True, check=False, timeout=30,
    )
    if result.returncode:
        raise ValueError(f"git {arguments[0]} failed: {result.stderr.strip()}")
    return result.stdout


def derive(source):
    if git("rev-parse", "HEAD").strip() != source:
        raise ValueError("floor source HEAD changed before derivation")
    if git("branch", "--show-current").strip():
        raise ValueError("floor derivation requires a detached checkout")
    flags = git("ls-files", "-v", "-z").split("\0")
    if any(entry and (entry[0].islower() or entry[0] == "S") for entry in flags):
        raise ValueError("floor derivation rejects hidden index entries")
    ignored = git("ls-files", "--others", "--ignored", "--exclude-standard", "-z")
    if any(path and not path.startswith("target/") for path in ignored.split("\0")):
        raise ValueError("floor derivation rejects ignored files outside root target")
    status = git("status", "--porcelain=v1", "--untracked-files=all", "-z")
    if status not in ("", " M Cargo.lock\0"):
        raise ValueError("floor derivation permits only an unstaged Cargo.lock change")
    if status:
        git("add", "--", "Cargo.lock")
        git("-c", "core.hooksPath=", "-c", "commit.gpgSign=false",
            "-c", "user.name=cargo-allow floor proof",
            "-c", "user.email=floor-proof@example.invalid", "commit",
            "-m", "chore(proof): derive local direct-floor lock")
    derived = git("rev-parse", "HEAD").strip()
    if derived != source:
        if git("rev-list", "--parents", "-n", "1", "HEAD").split() != [derived, source]:
            raise ValueError("derived floor commit must have exactly the source parent")
        if git("diff", "--name-only", "-z", source, derived) != "Cargo.lock\0":
            raise ValueError("derived floor commit changed source beyond Cargo.lock")
    if git("status", "--porcelain=v1", "--untracked-files=all", "-z"):
        raise ValueError("derived floor checkout is not clean")
    return {
        "source_commit": source,
        "derived_commit": derived,
        "derived_tree": git("rev-parse", "HEAD^{tree}").strip(),
    }


if __name__ == "__main__":
    try:
        print(json.dumps(derive(sys.argv[1]), indent=1))
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"proof-direct-floors: {error}", file=sys.stderr)
        raise SystemExit(1)
