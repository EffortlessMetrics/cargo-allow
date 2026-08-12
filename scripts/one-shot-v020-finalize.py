#!/usr/bin/env python3
"""Apply final contract/document repairs after the v0.2.0 closeout migration.

Temporary branch-only helper. It is removed by the one-shot workflow before the
durable release-candidate commit.
"""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


def patch_release_tests() -> None:
    path = ROOT / "crates/cargo-allow/src/release_prep_tests.rs"
    text = path.read_text(encoding="utf-8")
    replacements = [
        (
            '''    assert!(
        publish.contains(r#"version="${RELEASE_VERSION}""#),
        "publish should consume the preflight-resolved version"
    );''',
            '''    assert!(
        publish.contains("needs.preflight.outputs.version"),
        "publish should consume the preflight-resolved version"
    );''',
        ),
        (
            '"Generate release manifest"',
            '"Generate exact mixed-version release manifest"',
        ),
        (
            '"Attach release manifest to GitHub Release"',
            '"Attach verified release evidence and Linux archive"',
        ),
        (
            '"cargo-allow-${{ github.ref_name }}-x86_64-unknown-linux-gnu.tar.gz",',
            '"cargo-allow-${{ needs.preflight.outputs.tag }}-x86_64-unknown-linux-gnu.tar.gz",',
        ),
        (
            'workflow.contains("needs: [install-smoke, publish]")',
            'workflow.contains("needs: [preflight, install-smoke, publish]")',
        ),
    ]
    for old, new in replacements:
        if old in text:
            text = text.replace(old, new, 1)
    if 'publish.contains("needs.preflight.outputs.version")' not in text:
        raise SystemExit("release recovery version assertion seam changed")
    path.write_text(text, encoding="utf-8")


def patch_manifest_test() -> None:
    replace_once(
        ROOT / "crates/allow-report/src/artifacts/release_manifest.rs",
        '        assert_eq!(manifest.crates[9].name, "cargo-allow");',
        '''        assert_eq!(
            manifest.crates.last().map(|entry| entry.name.as_str()),
            Some("cargo-allow")
        );''',
        "release manifest mixed-row test",
    )


def patch_release_record() -> None:
    path = ROOT / "docs/release/0.2.0.md"
    text = path.read_text(encoding="utf-8")
    if "## Topology authority order" not in text:
        raise SystemExit("release publish-order heading seam changed")
    text = text.replace("## Topology authority order", "## Publish Order", 1)
    old = '''The output schema is `cargo-allow.sensor-capabilities.v1`, generation 1. It
states which source-tree sensors ran and which claims remain excluded. It is not
a compilation, type-analysis, macro-expansion, MIR, runtime, test-adequacy, or
coverage-proof contract.'''
    new = '''The output schema is `cargo-allow.sensor-capabilities.v1`, generation 1. The
catalog describes source-tree sensors and their bounded limitations. The catalog
does not claim compilation, type analysis, macro expansion, MIR, runtime
behavior, or test adequacy. Its checked source of truth remains #2570 and
`docs/support-matrix.toml`; it is not a coverage-proof contract.'''
    if old not in text:
        raise SystemExit("release capability claim-boundary seam changed")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


def patch_release_workflow() -> None:
    path = ROOT / ".github/workflows/release.yml"
    old = '''          test "$(git rev-parse HEAD^{commit})" = "${{ needs.preflight.outputs.commit }}"
          test "$(git rev-parse HEAD^{tree})" = "${{ needs.preflight.outputs.tree }}"'''
    new = '''          if [ "$(git rev-parse HEAD^{commit})" != "${{ needs.preflight.outputs.commit }}" ]; then
            echo "::error::publish checkout commit differs from preflight"
            exit 1
          fi
          if [ "$(git rev-parse HEAD^{tree})" != "${{ needs.preflight.outputs.tree }}" ]; then
            echo "::error::publish checkout tree differs from preflight"
            exit 1
          fi'''
    replace_once(path, old, new, "publish identity recheck")


def main() -> None:
    patch_release_tests()
    patch_manifest_test()
    patch_release_record()
    patch_release_workflow()


if __name__ == "__main__":
    main()
