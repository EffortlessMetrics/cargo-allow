#!/usr/bin/env python3
"""One-shot exact cargo-allow 0.2.0-rc.1 identity cut (#3694).

This helper is branch-only and removes itself before the durable commit. It
changes only cargo-allow-family versions; shared, cargo-intent, and cargo-proof
packages remain on their independent 0.1.0 lines.
"""

from __future__ import annotations

import re
import sys
from collections import OrderedDict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OLD = "0.2.0"
RC = "0.2.0-rc.1"
TODAY = "2026-08-23"


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, value: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(value, encoding="utf-8")


def replace_once(path: str, old: str, new: str, label: str) -> None:
    text = read(path)
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence in {path}, found {count}")
    write(path, text.replace(old, new, 1))


def prepare_cargo() -> None:
    path = ROOT / "Cargo.toml"
    text = path.read_text(encoding="utf-8")
    pattern = re.compile(r'(\[workspace\.package\][\s\S]*?^version = ")0\.2\.0("$)', re.M)
    text, count = pattern.subn(rf"\g<1>{RC}\2", text, count=1)
    if count != 1:
        raise SystemExit("workspace package version seam changed")

    dep_pattern = re.compile(
        r'^(allow-[a-z0-9-]+ = \{ path = "[^"]+", version = ")0\.2\.0(" \})$',
        re.M,
    )
    text, count = dep_pattern.subn(rf"\g<1>{RC}\2", text)
    if count != 9:
        raise SystemExit(f"expected nine cargo-allow internal dependency rows, changed {count}")
    if 'rust-version = "1.95"' not in text:
        raise SystemExit("RC cut must retain Rust 1.95")
    path.write_text(text, encoding="utf-8")

    replace_once(
        "crates/cargo-allow/Cargo.toml",
        'candidate_version = "0.2.0"',
        f'candidate_version = "{RC}"',
        "packaged candidate version",
    )


def prepare_topology() -> None:
    path = ROOT / "policy/product-package-topology-v2.toml"
    text = path.read_text(encoding="utf-8")
    parts = text.split("[[package]]")
    changed = 0
    rebuilt = [parts[0]]
    for block in parts[1:]:
        if 'product_family = "cargo-allow"' in block:
            count = block.count('package_version = "0.2.0"')
            if count != 1:
                raise SystemExit(
                    "cargo-allow topology row must carry one 0.2.0 package_version"
                )
            block = block.replace(
                'package_version = "0.2.0"',
                f'package_version = "{RC}"',
                1,
            )
            changed += 1
        rebuilt.append("[[package]]" + block)
    if changed != 10:
        raise SystemExit(f"expected ten cargo-allow topology rows, changed {changed}")
    output = "".join(rebuilt)
    for family in ("shared", "cargo-intent", "cargo-proof"):
        for block in output.split("[[package]]")[1:]:
            if f'product_family = "{family}"' in block and 'package_version = "0.1.0"' not in block:
                raise SystemExit(f"{family} package left its 0.1.0 version line")
    path.write_text(output, encoding="utf-8")


def prepare_support() -> None:
    replace_once(
        "docs/support-matrix.toml",
        'candidate_version = "0.2.0"',
        f'candidate_version = "{RC}"',
        "support candidate version",
    )
    replace_once(
        "docs/support-matrix.toml",
        'candidate_evidence = "docs/release/0.2.0.md"',
        f'candidate_evidence = "docs/release/{RC}.md"',
        "support candidate evidence",
    )
    support = read("docs/support-matrix.toml")
    support = support.replace(
        "# Workspace version on main. Deliberately unpublished pending the 0.2.0\n# blocker set; see docs/release/0.2.0.md and issue #2501.",
        "# Exact opt-in RC source candidate. Final stable 0.2.0 remains a later\n# fresh refreeze; see docs/release/0.2.0-rc.1.md and issue #3695.",
        1,
    )
    write("docs/support-matrix.toml", support)


def prepare_release_docs() -> None:
    stable_path = ROOT / "docs/release/0.2.0.md"
    stable = stable_path.read_text(encoding="utf-8")
    banner = (
        "> **Planned final release.** The current public-candidate train is "
        "[`0.2.0-rc.1`](0.2.0-rc.1.md). Final `0.2.0` requires a fresh "
        "post-RC refreeze and authorization.\n\n"
    )
    if banner not in stable:
        stable_path.write_text(banner + stable, encoding="utf-8")

    rc = stable.replace("0.2.0", RC)
    rc_banner = (
        f"> **Prerelease.** `{RC}` is an opt-in public release candidate on "
        "Rust 1.95. `0.1.11` remains the stable rollback baseline. Success of "
        "this RC does not authorize final `0.2.0`.\n\n"
    )
    write(f"docs/release/{RC}.md", rc_banner + rc)

    github_source = read("docs/release/github/v0.2.0.md")
    github_rc = github_source.replace("0.2.0", RC)
    github_banner = (
        f"> **Prerelease:** install explicitly with `cargo install cargo-allow "
        f"--version {RC} --locked`. The ordinary stable channel remains "
        "`0.1.11`.\n\n"
    )
    write(f"docs/release/github/v{RC}.md", github_banner + github_rc)

    release_readme = read("docs/release/README.md")
    section = f"""## Current release candidate\n\nThe selected prerelease is `{RC}` on Rust 1.95. Install it only by exact\nversion after the crates.io publication receipt is complete:\n\n```bash\ncargo install cargo-allow --version {RC} --locked\n```\n\n`0.1.11` remains the stable rollback baseline. Final `0.2.0` requires a new\nrefreeze and explicit authorization after RC evidence and repairs are merged.\n\n"""
    if "## Current release candidate" not in release_readme:
        anchor = release_readme.find("\n## ")
        if anchor == -1:
            release_readme = release_readme.rstrip() + "\n\n" + section
        else:
            release_readme = release_readme[: anchor + 1] + section + release_readme[anchor + 1 :]
        write("docs/release/README.md", release_readme)


def prepare_workflow() -> None:
    path = ROOT / ".github/workflows/release.yml"
    text = path.read_text(encoding="utf-8")
    output_anchor = "      recovery_receipt_run_id: ${{ steps.release_context.outputs.recovery_receipt_run_id }}\n"
    if output_anchor not in text:
        raise SystemExit("release preflight output seam changed")
    text = text.replace(
        output_anchor,
        output_anchor + "      prerelease: ${{ steps.release_context.outputs.prerelease }}\n",
        1,
    )

    context_anchor = '          commit="$(git rev-parse HEAD^{commit})"\n'
    if context_anchor not in text:
        raise SystemExit("release context identity seam changed")
    text = text.replace(
        context_anchor,
        '          if [[ "${version}" == *-* ]]; then prerelease=true; else prerelease=false; fi\n'
        + context_anchor,
        1,
    )

    output_line = '            echo "recovery_receipt_run_id=${RECOVERY_RECEIPT_RUN_ID}"\n'
    if output_line not in text:
        raise SystemExit("release context output block changed")
    text = text.replace(
        output_line,
        output_line + '            echo "prerelease=${prerelease}"\n',
        1,
    )

    count = text.count("          prerelease: false")
    if count != 2:
        raise SystemExit(f"expected two GitHub release prerelease literals, found {count}")
    text = text.replace(
        "          prerelease: false",
        "          prerelease: ${{ needs.preflight.outputs.prerelease == 'true' }}",
    )
    path.write_text(text, encoding="utf-8")


def prepare_preflight() -> None:
    path = ROOT / "scripts/release-version-preflight.sh"
    text = path.read_text(encoding="utf-8")
    anchor = 'workspace_version="$(read_workspace_version)"\n[[ -n "${workspace_version}" ]] || fail "could not read [workspace.package].version from Cargo.toml"\n'
    if anchor not in text:
        raise SystemExit("release version preflight workspace seam changed")
    validation = anchor + '''\nif [[ ! "${version}" =~ ^[0-9]+\\.[0-9]+\\.[0-9]+(-[0-9A-Za-z][0-9A-Za-z.-]*)?$ ]]; then
  fail "release version ${version} is not a supported SemVer release identity"
fi
if [[ "${version}" == *-* ]]; then
  release_channel="prerelease"
else
  release_channel="stable"
fi
log "release channel is ${release_channel}"
'''
    path.write_text(text.replace(anchor, validation, 1), encoding="utf-8")

    test_path = ROOT / "scripts/test-release-version-preflight.sh"
    tests = test_path.read_text(encoding="utf-8")
    append = '''\nrun_expect_success "matching RC tag" \\
  env DRY_RUN=true GITHUB_EVENT_NAME=push GITHUB_REF_NAME="v${workspace_version}" \\
  bash scripts/release-version-preflight.sh "${workspace_version}"

run_expect_failure "stable tag over RC package bytes" \\
  env DRY_RUN=true GITHUB_EVENT_NAME=push GITHUB_REF_NAME=v0.2.0 \\
  bash scripts/release-version-preflight.sh "${workspace_version}"

run_expect_failure "malformed release version" \\
  env DRY_RUN=true bash scripts/release-version-preflight.sh "0.2.0_rc1"
'''
    if "matching RC tag" not in tests:
        tests = tests.rstrip() + "\n" + append
        test_path.write_text(tests, encoding="utf-8")


def prepare_changie() -> None:
    old = ROOT / ".changes/0.2.0.md"
    if not old.exists():
        raise SystemExit("expected retained .changes/0.2.0.md corpus")
    target = ROOT / "target/cargo-allow/rc1-legacy-0.2.0.md"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(old.read_bytes())
    old.unlink()
    rc_file = ROOT / f".changes/{RC}.md"
    if rc_file.exists():
        raise SystemExit(f"{rc_file} already exists before Changie batch")
    fragment = ROOT / ".changes/Changed-20260823-rc1-release-identity.yaml"
    fragment.write_text(
        "kind: Changed\n"
        "body: >-\n"
        "  Cut the Rust 1.95 cargo-allow package family as the explicit\n"
        "  0.2.0-rc.1 prerelease while preserving the shared, cargo-intent,\n"
        "  and cargo-proof packages on their independent 0.1.0 lines.\n",
        encoding="utf-8",
    )


def parse_sections(text: str) -> OrderedDict[str, list[str]]:
    sections: OrderedDict[str, list[str]] = OrderedDict()
    current: str | None = None
    for line in text.splitlines()[1:]:
        if line.startswith("### "):
            current = line[4:].strip()
            sections.setdefault(current, [])
        elif current is not None:
            sections[current].append(line)
    return sections


def merge_changelog() -> None:
    legacy_path = ROOT / "target/cargo-allow/rc1-legacy-0.2.0.md"
    generated_path = ROOT / f".changes/{RC}.md"
    if not legacy_path.is_file() or not generated_path.is_file():
        raise SystemExit("Changie RC merge inputs are missing")
    generated = generated_path.read_text(encoding="utf-8")
    heading = generated.splitlines()[0]
    if RC not in heading:
        raise SystemExit("Changie generated the wrong prerelease heading")
    legacy_sections = parse_sections(legacy_path.read_text(encoding="utf-8"))
    generated_sections = parse_sections(generated)
    order = ["Added", "Changed", "Deprecated", "Removed", "Fixed", "Security", "Documentation"]
    output = [heading, ""]
    for kind in order:
        lines: list[str] = []
        lines.extend(legacy_sections.get(kind, []))
        if lines and lines[-1] != "":
            lines.append("")
        lines.extend(generated_sections.get(kind, []))
        while lines and not lines[0].strip():
            lines.pop(0)
        while lines and not lines[-1].strip():
            lines.pop()
        if not lines:
            continue
        output.extend([f"### {kind}", "", *lines, ""])
    generated_path.write_text("\n".join(output).rstrip() + "\n", encoding="utf-8")
    legacy_path.unlink()


def verify() -> None:
    cargo = read("Cargo.toml")
    if f'version = "{RC}"' not in cargo or 'rust-version = "1.95"' not in cargo:
        raise SystemExit("workspace RC/MSRV identity is incomplete")
    topology = read("policy/product-package-topology-v2.toml")
    if topology.count(f'package_version = "{RC}"') != 10:
        raise SystemExit("topology does not contain exactly ten RC rows")
    if topology.count('package_version = "0.1.0"') != 12:
        raise SystemExit("topology does not preserve exactly twelve 0.1.0 namespace rows")
    changelog = read("CHANGELOG.md")
    if changelog.count(f"## [{RC}] - ") != 1:
        raise SystemExit("CHANGELOG does not contain exactly one RC section")
    if "## [0.2.0] - " in changelog:
        raise SystemExit("future stable 0.2.0 is still presented as released")
    if not (ROOT / f"docs/release/{RC}.md").is_file():
        raise SystemExit("RC release record missing")
    if not (ROOT / f"docs/release/github/v{RC}.md").is_file():
        raise SystemExit("RC GitHub notes missing")


def main() -> None:
    if len(sys.argv) != 2 or sys.argv[1] not in {"prepare", "merge", "verify"}:
        raise SystemExit("usage: one-shot-cut-rc1.py prepare|merge|verify")
    phase = sys.argv[1]
    if phase == "prepare":
        prepare_cargo()
        prepare_topology()
        prepare_support()
        prepare_release_docs()
        prepare_workflow()
        prepare_preflight()
        prepare_changie()
    elif phase == "merge":
        merge_changelog()
    else:
        verify()


if __name__ == "__main__":
    main()
