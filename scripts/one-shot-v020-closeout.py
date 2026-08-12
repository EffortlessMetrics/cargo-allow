#!/usr/bin/env python3
"""Converge the v0.2.0 release candidate onto the token-backed mixed-version path.

This is a temporary branch-only migration helper. The one-shot workflow removes
it before committing the reviewed release candidate.
"""

from __future__ import annotations

import re
import textwrap
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

NAMESPACE_PACKAGES = (
    "effortless-rust-source-index",
    "intent-model",
    "intent-protocol",
    "intent-compiler",
    "intent-edit",
    "cargo-intent",
    "proof-protocol",
    "proof-orchestrator",
    "cargo-proof",
)

MANIFEST_PATHS = (
    "crates/effortless-rust-source-index/Cargo.toml",
    "crates/intent-model/Cargo.toml",
    "crates/intent-protocol/Cargo.toml",
    "crates/intent-engine/Cargo.toml",
    "crates/intent-edit/Cargo.toml",
    "crates/cargo-intent/Cargo.toml",
    "crates/proof-protocol/Cargo.toml",
    "crates/proof-engine/Cargo.toml",
    "crates/cargo-proof/Cargo.toml",
)

PUBLISH_ROWS = (
    ("allow-core", "0.2.0"),
    ("allow-policy", "0.2.0"),
    ("allow-inventory", "0.2.0"),
    ("allow-files", "0.2.0"),
    ("allow-rust", "0.2.0"),
    ("allow-match", "0.2.0"),
    ("allow-report", "0.2.0"),
    ("allow-policy-legacy", "0.2.0"),
    ("effortless-repo-protocol", "0.1.0"),
    ("effortless-repo-snapshot", "0.1.0"),
    ("effortless-repo-edit", "0.1.0"),
    ("allow-diff", "0.2.0"),
    ("cargo-allow", "0.2.0"),
)

ALL_TOPOLOGY_ORDER = (
    "allow-core",
    "allow-policy",
    "allow-inventory",
    "allow-files",
    "allow-rust",
    "allow-match",
    "allow-report",
    "allow-policy-legacy",
    "effortless-repo-protocol",
    "effortless-repo-snapshot",
    "effortless-repo-edit",
    "allow-diff",
    "cargo-allow",
    "effortless-rust-source-index",
    "intent-model",
    "intent-protocol",
    "intent-compiler",
    "intent-edit",
    "cargo-intent",
    "proof-protocol",
    "proof-orchestrator",
    "cargo-proof",
)


def read(relative: str) -> str:
    return (ROOT / relative).read_text(encoding="utf-8")


def write(relative: str, content: str) -> None:
    (ROOT / relative).write_text(content, encoding="utf-8")


def replace_once(relative: str, old: str, new: str) -> None:
    content = read(relative)
    count = content.count(old)
    if count != 1:
        raise SystemExit(f"{relative}: expected one occurrence, found {count}: {old!r}")
    write(relative, content.replace(old, new, 1))


def enable_namespace_manifests() -> None:
    for relative in MANIFEST_PATHS:
        content = read(relative)
        count = content.count("publish = false")
        if count != 1:
            raise SystemExit(f"{relative}: expected one publish=false, found {count}")
        write(relative, content.replace("publish = false", "publish = true", 1))


def enable_namespace_topology() -> None:
    relative = "policy/product-package-topology-v2.toml"
    content = read(relative)
    prefix, *blocks = content.split("[[package]]")
    changed: set[str] = set()
    rendered = [prefix]
    for raw in blocks:
        block = "[[package]]" + raw
        match = re.search(r'^cargo_package_name = "([^"]+)"$', block, re.MULTILINE)
        if match and match.group(1) in NAMESPACE_PACKAGES:
            if "publish = false" not in block:
                raise SystemExit(f"{relative}: {match.group(1)} is not publish=false")
            block = block.replace("publish = false", "publish = true", 1)
            changed.add(match.group(1))
        rendered.append(block)
    missing = sorted(set(NAMESPACE_PACKAGES) - changed)
    if missing:
        raise SystemExit(f"{relative}: namespace rows not changed: {missing}")
    write(relative, "".join(rendered))


def update_topology_publisher() -> None:
    relative = "scripts/release-topology-publisher.py"
    content = read(relative)
    old = '''    families = FAMILY_MODES[mode]\n    rows: list[dict[str, Any]] = []\n    for raw in topology.get("package", []):\n        if raw.get("product_family") not in families or raw.get("publish") is not True:\n            continue\n'''
    new = '''    families = FAMILY_MODES[mode]\n    rows: list[dict[str, Any]] = []\n    for raw in topology.get("package", []):\n        if raw.get("publish") is not True:\n            continue\n        if mode == "cargo-allow":\n            # The supported product candidate is mixed-version: ten cargo-allow\n            # rows plus the three shared rows selected by the V2 authority.\n            if raw.get("candidate_inclusion") is not True:\n                continue\n        elif raw.get("product_family") not in families:\n            continue\n'''
    if old not in content:
        raise SystemExit(f"{relative}: selection seam changed")
    content = content.replace(old, new, 1)
    content = content.replace(
        '''            "publication_state",\n            "release_order",\n''',
        '''            "publication_state",\n            "candidate_inclusion",\n            "release_order",\n''',
        1,
    )
    content = content.replace(
        '''        "publish": args.publish,\n        "topology_id": topology["topology_id"],\n''',
        '''        "publish": args.publish,\n        "authentication": "crates_io_api_token" if args.publish else "none",\n        "topology_id": topology["topology_id"],\n''',
        1,
    )
    write(relative, content)


def rewrite_release_workflow() -> None:
    workflow = r'''name: Release

on:
  push:
    tags:
      - v*
  workflow_dispatch:
    # Ordinary dispatches are zero-upload rehearsals. The separately checked
    # authorization workflow is the only caller of publish_recovery.
    inputs:
      publish_recovery:
        description: "Publish and close out one exact authorized tagged candidate"
        required: false
        default: false
        type: boolean
      recovery_version:
        description: "Exact tagged version, without the v prefix"
        required: false
        type: string
      recovery_commit:
        description: "Exact commit referenced by the tag"
        required: false
        type: string
      recovery_tree:
        description: "Exact tree referenced by the tag"
        required: false
        type: string
      recovery_authorization:
        description: "Bounded authorization or release-incident reference"
        required: false
        type: string

permissions:
  contents: read

concurrency:
  group: release-${{ github.ref }}-${{ inputs.recovery_version }}
  cancel-in-progress: false

env:
  CARGO_TERM_COLOR: always

jobs:
  preflight:
    runs-on: ubuntu-24.04
    timeout-minutes: 120
    permissions:
      contents: read
    outputs:
      version: ${{ steps.release_context.outputs.version }}
      tag: ${{ steps.release_context.outputs.tag }}
      commit: ${{ steps.release_context.outputs.commit }}
      tree: ${{ steps.release_context.outputs.tree }}
      publish: ${{ steps.release_context.outputs.publish }}
      authorization: ${{ steps.release_context.outputs.authorization }}
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          fetch-depth: 0
          persist-credentials: false
          ref: ${{ github.event_name == 'workflow_dispatch' && inputs.publish_recovery && inputs.recovery_version != '' && format('refs/tags/v{0}', inputs.recovery_version) || github.sha }}
      - uses: dtolnay/rust-toolchain@6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772
        with:
          toolchain: stable
      - name: Resolve exact release context
        id: release_context
        shell: bash
        env:
          REQUESTED_PUBLISH: ${{ inputs.publish_recovery }}
          RECOVERY_VERSION: ${{ inputs.recovery_version }}
          RECOVERY_COMMIT: ${{ inputs.recovery_commit }}
          RECOVERY_TREE: ${{ inputs.recovery_tree }}
          RECOVERY_AUTHORIZATION: ${{ inputs.recovery_authorization }}
        run: |
          set -euo pipefail
          publish=false
          authorization=""
          if [ "${GITHUB_EVENT_NAME}" = "workflow_dispatch" ] && [ "${REQUESTED_PUBLISH}" = "true" ]; then
            publish=true
            version="${RECOVERY_VERSION}"
            tag="v${version}"
            authorization="${RECOVERY_AUTHORIZATION}"
            if [ -z "${version}" ] || [ -z "${RECOVERY_COMMIT}" ] || [ -z "${RECOVERY_TREE}" ] || [ -z "${authorization}" ]; then
              echo "::error::publish_recovery requires version, commit, tree, and authorization"
              exit 1
            fi
            if [[ ! "${RECOVERY_COMMIT}" =~ ^[0-9a-fA-F]{40,64}$ ]] || [[ ! "${RECOVERY_TREE}" =~ ^[0-9a-fA-F]{40,64}$ ]]; then
              echo "::error::recovery_commit and recovery_tree must be hexadecimal identities"
              exit 1
            fi
            if [[ ! "${authorization}" =~ ^[A-Za-z0-9._:/#-]+$ ]]; then
              echo "::error::recovery_authorization must be a bounded reference"
              exit 1
            fi
            git fetch --tags origin
            tagged_commit="$(git rev-parse "${tag}^{commit}")"
            tagged_tree="$(git rev-parse "${tag}^{tree}")"
            if [ "${tagged_commit}" != "${RECOVERY_COMMIT}" ] || [ "${tagged_tree}" != "${RECOVERY_TREE}" ]; then
              echo "::error::recovery tag does not match the supplied commit/tree"
              exit 1
            fi
          elif [ "${GITHUB_EVENT_NAME}" = "push" ] && [[ "${GITHUB_REF_NAME}" == v* ]]; then
            # A bare tag push never authorizes an upload. It exercises the exact
            # candidate as a rehearsal; the authorization workflow dispatches
            # the real publication after it verifies namespace publication.
            version="${GITHUB_REF_NAME#v}"
            tag="${GITHUB_REF_NAME}"
          else
            version="$(awk '/^\[workspace\.package\]/{f=1;next} /^\[/{if(f) exit} f && /^version = /{gsub(/version = "/,""); gsub(/".*/,""); print; exit}' Cargo.toml)"
            tag=""
          fi
          commit="$(git rev-parse HEAD^{commit})"
          tree="$(git rev-parse HEAD^{tree})"
          if [ "${publish}" = "true" ]; then
            if [ "${commit}" != "${RECOVERY_COMMIT}" ] || [ "${tree}" != "${RECOVERY_TREE}" ]; then
              echo "::error::recovery checkout is not the exact tagged commit/tree"
              exit 1
            fi
          fi
          bash scripts/release-version-preflight.sh "${version}"
          {
            echo "version=${version}"
            echo "tag=${tag}"
            echo "commit=${commit}"
            echo "tree=${tree}"
            echo "publish=${publish}"
            echo "authorization=${authorization}"
          } >> "${GITHUB_OUTPUT}"
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-targets --locked -- -D warnings
      - run: cargo test --workspace --locked
      - name: Prove exact topology row counts
        shell: bash
        run: |
          set -euo pipefail
          test "$(python3 scripts/release-topology-publisher.py --mode namespace --list | wc -l)" -eq 12
          test "$(python3 scripts/release-topology-publisher.py --mode cargo-allow --list | wc -l)" -eq 13
      - run: cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md
      - uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a
        if: always()
        with:
          name: release-preflight-receipts
          path: target/cargo-allow/

  publish:
    needs: preflight
    runs-on: ubuntu-24.04
    timeout-minutes: 120
    permissions:
      contents: read
    outputs:
      auth_source: ${{ steps.registry_token.outputs.source }}
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          fetch-depth: 0
          persist-credentials: false
          ref: ${{ needs.preflight.outputs.publish == 'true' && format('refs/tags/{0}', needs.preflight.outputs.tag) || github.sha }}
      - uses: dtolnay/rust-toolchain@6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772
        with:
          toolchain: stable
      - name: Resolve bounded crates.io API token
        id: registry_token
        shell: bash
        env:
          REQUESTED_PUBLISH: ${{ needs.preflight.outputs.publish }}
          CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
        run: |
          set -euo pipefail
          if [ "${REQUESTED_PUBLISH}" = "true" ]; then
            [ -n "${CARGO_REGISTRY_TOKEN}" ] || {
              echo "::error::CARGO_REGISTRY_TOKEN is absent; no upload was attempted"
              exit 1
            }
            echo "source=crates_io_api_token" >> "${GITHUB_OUTPUT}"
          else
            echo "source=none" >> "${GITHUB_OUTPUT}"
          fi
      - name: Recheck exact tagged identity before publication
        if: needs.preflight.outputs.publish == 'true'
        shell: bash
        run: |
          set -euo pipefail
          test "$(git rev-parse HEAD^{commit})" = "${{ needs.preflight.outputs.commit }}"
          test "$(git rev-parse HEAD^{tree})" = "${{ needs.preflight.outputs.tree }}"
      - name: Publish or rehearse the topology-derived cargo-allow candidate
        shell: bash
        env:
          REQUESTED_PUBLISH: ${{ needs.preflight.outputs.publish }}
          CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
        run: |
          set -euo pipefail
          args=(--mode cargo-allow --receipt target/cargo-allow/topology-publish.receipt.json)
          if [ "${REQUESTED_PUBLISH}" = "true" ]; then
            args+=(--publish)
          fi
          python3 scripts/release-topology-publisher.py "${args[@]}"
      - name: Record publication handoff
        if: always()
        shell: bash
        run: |
          mkdir -p target/cargo-allow
          cat > target/cargo-allow/release-publish.receipt.json <<EOF
          {
            "event": "${{ github.event_name }}",
            "ref": "${{ needs.preflight.outputs.tag }}",
            "version": "${{ needs.preflight.outputs.version }}",
            "candidate_commit": "${{ needs.preflight.outputs.commit }}",
            "candidate_tree": "${{ needs.preflight.outputs.tree }}",
            "publish": ${{ needs.preflight.outputs.publish }},
            "authorization": "${{ needs.preflight.outputs.authorization }}",
            "workflow_run_id": ${{ github.run_id }},
            "auth": "${{ steps.registry_token.outputs.source }}"
          }
          EOF
      - uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a
        if: always()
        with:
          name: release-publish-receipt
          path: |
            target/cargo-allow/release-publish.receipt.json
            target/cargo-allow/topology-publish.receipt.json

  install-smoke:
    needs: [preflight, publish]
    if: needs.preflight.outputs.publish == 'true' && needs.publish.result == 'success'
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-24.04, windows-2025]
    runs-on: ${{ matrix.os }}
    timeout-minutes: 60
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          persist-credentials: false
          ref: ${{ needs.preflight.outputs.tag }}
      - uses: dtolnay/rust-toolchain@6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772
        with:
          toolchain: stable
      - name: Verify exact cargo-allow install from crates.io
        shell: bash
        run: bash scripts/release-install-smoke.sh "${{ needs.preflight.outputs.version }}"
      - name: Record install-smoke receipt
        if: always()
        shell: bash
        run: |
          mkdir -p target/cargo-allow
          cat > target/cargo-allow/release-install-smoke.receipt.json <<EOF
          {
            "tag": "${{ needs.preflight.outputs.tag }}",
            "commit": "${{ needs.preflight.outputs.commit }}",
            "tree": "${{ needs.preflight.outputs.tree }}",
            "version": "${{ needs.preflight.outputs.version }}",
            "os": "${{ matrix.os }}",
            "workflow_run_id": ${{ github.run_id }}
          }
          EOF
      - uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a
        if: always()
        with:
          name: release-install-smoke-receipt-${{ matrix.os }}
          path: target/cargo-allow/release-install-smoke.receipt.json

  github-release:
    needs: [preflight, install-smoke, publish]
    if: needs.preflight.outputs.publish == 'true' && needs.publish.result == 'success' && needs.install-smoke.result == 'success'
    runs-on: ubuntu-24.04
    timeout-minutes: 90
    permissions:
      contents: write
      id-token: write
      attestations: write
      actions: read
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          fetch-depth: 0
          persist-credentials: false
          ref: ${{ needs.preflight.outputs.tag }}
      - uses: dtolnay/rust-toolchain@6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772
        with:
          toolchain: stable
          targets: x86_64-unknown-linux-gnu
      - name: Download the exact publication receipt from this run
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          set -euo pipefail
          gh run download "${GITHUB_RUN_ID}" --repo "${GITHUB_REPOSITORY}" \
            --name release-publish-receipt \
            --dir target/cargo-allow/publish-artifact
          cp target/cargo-allow/publish-artifact/topology-publish.receipt.json \
            target/cargo-allow/topology-publish.receipt.json
      - name: Repackage exact mixed-version candidate bytes
        shell: bash
        run: |
          set -euo pipefail
          while IFS=$'\t' read -r _order _family package _version; do
            cargo package -p "${package}" --locked --no-verify
          done < <(python3 scripts/release-topology-publisher.py --mode cargo-allow --list)
      - name: Build tagged Linux release binary
        run: cargo build -p cargo-allow --bin cargo-allow --release --locked --target x86_64-unknown-linux-gnu
      - name: Package tagged Linux executable archive
        env:
          RELEASE_TAG: ${{ needs.preflight.outputs.tag }}
          RELEASE_COMMIT: ${{ needs.preflight.outputs.commit }}
          RELEASE_TREE: ${{ needs.preflight.outputs.tree }}
          CARGO_ALLOW_BIN: target/x86_64-unknown-linux-gnu/release/cargo-allow
          OUTPUT_DIR: target/cargo-allow/release-assets
        run: |
          set -euo pipefail
          bash scripts/package-release-binary.sh \
            --version "${{ needs.preflight.outputs.version }}" \
            --target x86_64-unknown-linux-gnu \
            --output-dir "${OUTPUT_DIR}"
      - name: Verify tagged Linux executable archive
        env:
          RELEASE_TAG: ${{ needs.preflight.outputs.tag }}
          RELEASE_COMMIT: ${{ needs.preflight.outputs.commit }}
          RELEASE_TREE: ${{ needs.preflight.outputs.tree }}
          OUTPUT_DIR: target/cargo-allow/release-assets
        run: |
          set -euo pipefail
          archive="${OUTPUT_DIR}/cargo-allow-${RELEASE_TAG}-x86_64-unknown-linux-gnu.tar.gz"
          bash scripts/verify-release-binary.sh \
            --version "${{ needs.preflight.outputs.version }}" \
            --receipt "${OUTPUT_DIR}/release-binary-install.receipt.json" \
            "${archive}"
      - name: Attest tagged Linux executable archive
        uses: actions/attest-build-provenance@0f67c3f4856b2e3261c31976d6725780e5e4c373
        with:
          subject-path: target/cargo-allow/release-assets/cargo-allow-${{ needs.preflight.outputs.tag }}-x86_64-unknown-linux-gnu.tar.gz
      - name: Verify tagged Linux executable attestation
        env:
          GH_TOKEN: ${{ github.token }}
          RELEASE_TAG: ${{ needs.preflight.outputs.tag }}
          RELEASE_COMMIT: ${{ needs.preflight.outputs.commit }}
          RELEASE_TREE: ${{ needs.preflight.outputs.tree }}
          OUTPUT_DIR: target/cargo-allow/release-assets
        run: |
          set -euo pipefail
          archive="${OUTPUT_DIR}/cargo-allow-${RELEASE_TAG}-x86_64-unknown-linux-gnu.tar.gz"
          gh attestation verify "${archive}" --repo "${GITHUB_REPOSITORY}"
          ATTESTATION_VERIFIED=true bash scripts/verify-release-binary.sh \
            --version "${{ needs.preflight.outputs.version }}" \
            --receipt "${OUTPUT_DIR}/release-binary-install.receipt.json" \
            "${archive}"
      - name: Generate exact mixed-version release manifest
        env:
          VERSION: ${{ needs.preflight.outputs.version }}
          REPOSITORY: ${{ github.repository }}
          TAG: ${{ needs.preflight.outputs.tag }}
          COMMIT: ${{ needs.preflight.outputs.commit }}
          TREE: ${{ needs.preflight.outputs.tree }}
          AUTH_SOURCE: ${{ needs.publish.outputs.auth_source }}
          WORKFLOW_RUN_ID: ${{ github.run_id }}
          MSRV: "1.95"
          PLATFORMS: "linux windows"
          PUBLISH_RECEIPT: target/cargo-allow/topology-publish.receipt.json
          BINARY_PACKAGE_RECEIPT: target/cargo-allow/release-assets/release-binary.receipt.json
          BINARY_INSTALL_RECEIPT: target/cargo-allow/release-assets/release-binary-install.receipt.json
          RUST_TOOLCHAIN: stable
          RUNNER: ubuntu-24.04
        run: bash scripts/generate-release-manifest.sh
      - name: Require a Complete manifest before attestation or release creation
        run: |
          python3 - <<'PY'
          import json
          from pathlib import Path
          payload = json.loads(Path('target/cargo-allow/release-manifest-v1.json').read_text())
          if payload.get('result') != 'Complete':
              raise SystemExit(f"release manifest is not Complete: {payload.get('result')}")
          PY
          (cd target/cargo-allow && sha256sum -c release-manifest-v1.sha256)
      - name: Attest release manifest
        uses: actions/attest-build-provenance@0f67c3f4856b2e3261c31976d6725780e5e4c373
        with:
          subject-path: target/cargo-allow/release-manifest-v1.json
      - name: Create draft GitHub Release
        uses: softprops/action-gh-release@42dc69e1aa15d09112580998cf2ef0119e2e91ae
        with:
          tag_name: ${{ needs.preflight.outputs.tag }}
          name: cargo-allow ${{ needs.preflight.outputs.tag }}
          body_path: docs/release/github/v${{ needs.preflight.outputs.version }}.md
          draft: true
          prerelease: false
      - name: Attach verified release evidence and Linux archive
        uses: softprops/action-gh-release@42dc69e1aa15d09112580998cf2ef0119e2e91ae
        with:
          tag_name: ${{ needs.preflight.outputs.tag }}
          files: |
            target/cargo-allow/release-manifest-v1.json
            target/cargo-allow/release-manifest-v1.sha256
            target/cargo-allow/topology-publish.receipt.json
            target/cargo-allow/release-assets/cargo-allow-${{ needs.preflight.outputs.tag }}-x86_64-unknown-linux-gnu.tar.gz
            target/cargo-allow/release-assets/cargo-allow-${{ needs.preflight.outputs.tag }}-x86_64-unknown-linux-gnu.tar.gz.sha256
            target/cargo-allow/release-assets/cargo-allow-${{ needs.preflight.outputs.tag }}-x86_64-unknown-linux-gnu.tar.gz.executable.sha256
            target/cargo-allow/release-assets/release-binary.receipt.json
            target/cargo-allow/release-assets/release-binary-install.receipt.json
      - name: Publish GitHub Release only after complete attachment
        uses: softprops/action-gh-release@42dc69e1aa15d09112580998cf2ef0119e2e91ae
        with:
          tag_name: ${{ needs.preflight.outputs.tag }}
          draft: false
          prerelease: false
'''
    write(".github/workflows/release.yml", workflow)


def rewrite_manifest_generator() -> None:
    generator = r'''#!/usr/bin/env bash
# Generate a topology-derived cargo-allow.release-manifest.v1 artifact from the
# exact publication receipt and tagged binary evidence.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

version="${VERSION:?VERSION is required}"
repository="${REPOSITORY:?REPOSITORY is required}"
tag="${TAG:?TAG is required}"
commit="${COMMIT:?COMMIT is required}"
tree="${TREE:?TREE is required}"
auth_source="${AUTH_SOURCE:?AUTH_SOURCE is required}"
msrv="${MSRV:?MSRV is required}"
platforms="${PLATFORMS:?PLATFORMS is required}"
publish_receipt="${PUBLISH_RECEIPT:?PUBLISH_RECEIPT is required}"
workflow_run_id="${WORKFLOW_RUN_ID:-}"
binary_package_receipt="${BINARY_PACKAGE_RECEIPT:-}"
binary_install_receipt="${BINARY_INSTALL_RECEIPT:-}"
rust_toolchain="${RUST_TOOLCHAIN:-stable}"
runner="${RUNNER:-ubuntu-24.04}"
output="${OUTPUT:-target/cargo-allow/release-manifest-v1.json}"
generated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

if [[ -n "${binary_package_receipt}" && -z "${binary_install_receipt}" ]] || \
   [[ -z "${binary_package_receipt}" && -n "${binary_install_receipt}" ]]; then
  echo 'release-manifest: binary package/install receipts must be supplied together' >&2
  exit 1
fi
mkdir -p "$(dirname "${output}")"

python3 - "${output}" "${version}" "${repository}" "${tag}" "${commit}" \
  "${tree}" "${auth_source}" "${workflow_run_id}" "${msrv}" "${platforms}" \
  "${generated_at}" "${publish_receipt}" "${binary_package_receipt}" \
  "${binary_install_receipt}" "${rust_toolchain}" "${runner}" <<'PY'
import hashlib
import json
import pathlib
import sys
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

(
    output, version, repository, tag, commit, tree, auth_source,
    workflow_run_id, msrv, platforms_text, generated_at, publish_path,
    package_path, install_path, rust_toolchain, runner,
) = sys.argv[1:]

ROOT = pathlib.Path('.')
TOPOLOGY = ROOT / 'policy/product-package-topology-v2.toml'
ALLOWED_AUTH = {'crates_io_api_token', 'oidc'}
CLAIM_BOUNDARY = (
    'cargo-allow scanned source-tree/source-syntax surfaces and maintained a '
    'durable exception ledger. It did not claim compilation, type analysis, '
    'macro expansion, MIR, runtime behavior, test adequacy, or universal '
    'repository suitability.'
)
LIMITATIONS = [
    'source-tree/source-syntax analysis only; repository code was not executed',
    'Linux and Windows crates.io installs were proven; the attached prebuilt archive is x86_64-unknown-linux-gnu only',
    'cargo-intent and cargo-proof packages are experimental and are not part of the supported cargo-allow product surface',
]


def sha256(path):
    digest = hashlib.sha256()
    with pathlib.Path(path).open('rb') as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def digest(value):
    return 'sha256:' + value


def read_json(path):
    try:
        value = json.loads(pathlib.Path(path).read_text(encoding='utf-8'))
    except (OSError, json.JSONDecodeError) as error:
        raise SystemExit(f'release-manifest: invalid JSON receipt {path}: {error}')
    if not isinstance(value, dict):
        raise SystemExit(f'release-manifest: receipt {path} is not an object')
    return value


with TOPOLOGY.open('rb') as handle:
    topology = tomllib.load(handle)
rows = [
    dict(row)
    for row in topology.get('package', [])
    if row.get('publish') is True and row.get('candidate_inclusion') is True
]
rows.sort(key=lambda row: (int(row['release_order']), row['cargo_package_name']))
if len(rows) != 13:
    raise SystemExit(f'release-manifest: expected 13 candidate rows, got {len(rows)}')
if version != '0.2.0' or tag != 'v0.2.0':
    raise SystemExit('release-manifest: this generator is bound to the v0.2.0 candidate')
if auth_source not in ALLOWED_AUTH:
    raise SystemExit(f'release-manifest: unsupported auth source {auth_source}')

publication = read_json(publish_path)
for field, expected in (
    ('schema_id', 'cargo-allow.topology-publish-receipt.v1'),
    ('mode', 'cargo-allow'),
    ('publish', True),
    ('complete', True),
    ('commit', commit),
    ('tree', tree),
):
    if publication.get(field) != expected:
        raise SystemExit(
            f'release-manifest: publication receipt {field} differs: '
            f'{publication.get(field)!r} != {expected!r}'
        )
if publication.get('authentication') != auth_source:
    raise SystemExit('release-manifest: publication authentication differs from workflow handoff')

published_rows = publication.get('rows')
if not isinstance(published_rows, list) or len(published_rows) != len(rows):
    raise SystemExit('release-manifest: publication receipt row count differs from topology')

crates = []
for topology_row, published in zip(rows, published_rows):
    name = topology_row['cargo_package_name']
    package_version = topology_row['package_version']
    expected = {
        'name': name,
        'version': package_version,
        'release_order': int(topology_row['release_order']),
    }
    for field, value in expected.items():
        if published.get(field) != value:
            raise SystemExit(f'release-manifest: publication row {field} differs for {name}')
    if published.get('state') not in {'verified_existing', 'published_verified'}:
        raise SystemExit(f"release-manifest: {name} is not registry verified: {published.get('state')}")
    local = published.get('local_checksum')
    registry = published.get('registry_checksum')
    if not isinstance(local, str) or not isinstance(registry, str) or local != registry:
        raise SystemExit(f'release-manifest: local/registry checksum disagreement for {name}')
    crate_path = ROOT / 'target/package' / f'{name}-{package_version}.crate'
    if not crate_path.is_file():
        raise SystemExit(f'release-manifest: missing repackaged candidate {crate_path}')
    observed = sha256(crate_path)
    if observed != local:
        raise SystemExit(f'release-manifest: repackaged bytes differ from publication for {name}')
    crates.append({
        'name': name,
        'version': package_version,
        'crate_checksum': digest(local),
        'registry_checksum': digest(registry),
    })

platforms = platforms_text.split()
binary_assets = []
binary_attestation_verified = True
if package_path and install_path:
    package = read_json(package_path)
    install = read_json(install_path)
    if package.get('schema_id') != 'cargo-allow.release-binary-package.v1':
        raise SystemExit('release-manifest: unexpected binary package receipt schema')
    if install.get('schema_id') != 'cargo-allow.release-binary-install.v1':
        raise SystemExit('release-manifest: unexpected binary install receipt schema')
    fields = (
        'version', 'target_triple', 'archive_name', 'archive_sha256',
        'executable_sha256', 'tag', 'commit', 'tree',
    )
    for field in fields:
        if package.get(field) != install.get(field):
            raise SystemExit(f'release-manifest: binary receipt disagreement in {field}')
    for field, expected in (
        ('version', version), ('tag', tag), ('commit', commit), ('tree', tree),
    ):
        if package.get(field) != expected:
            raise SystemExit(f'release-manifest: binary {field} differs from release identity')
    if package.get('target_triple') != 'x86_64-unknown-linux-gnu':
        raise SystemExit('release-manifest: unsupported binary target')
    if package.get('archive_format') != 'tar.gz' or package.get('executable_name') != 'cargo-allow':
        raise SystemExit('release-manifest: malformed Linux archive contract')
    binary_attestation_verified = install.get('attestation_verified') is True
    binary_assets.append({
        'platform': 'linux',
        'target_triple': package['target_triple'],
        'version': version,
        'tag': tag,
        'commit': commit,
        'tree': tree,
        'executable_version': version,
        'archive_name': package['archive_name'],
        'archive_format': package['archive_format'],
        'archive_sha256': package['archive_sha256'],
        'executable_name': package['executable_name'],
        'executable_sha256': package['executable_sha256'],
        'rust_toolchain': rust_toolchain,
        'runner': runner,
        'candidate_receipt_digest': digest(sha256(package_path)),
        'installed_smoke_receipt_digest': digest(sha256(install_path)),
        'attestation_subject_sha256': package['archive_sha256'],
        'limitations': [
            'x86_64-unknown-linux-gnu archive only; no universal Linux or CPU compatibility claim',
            'the archive is release-complete only after GitHub attestation verification',
        ],
    })

complete = (
    len(crates) == 13
    and all(item.get('crate_checksum') == item.get('registry_checksum') for item in crates)
    and auth_source in ALLOWED_AUTH
    and binary_attestation_verified
    and {'linux', 'windows'}.issubset(set(platforms))
)
manifest = {
    'schema_id': 'cargo-allow.release-manifest.v1',
    'schema_version': 1,
    'tool_version': version,
    'repository': repository,
    'tag': tag,
    'commit': commit,
    'tree': tree,
    'version': version,
    'source_candidate_digest': digest(hashlib.sha256(TOPOLOGY.read_bytes()).hexdigest()),
    'crates': crates,
    'auth_source': auth_source,
    'msrv': msrv,
    'platforms_proven': platforms,
    'binary_assets': binary_assets,
    'generations': {'release_manifest': 1, 'add_finding_plan': 1, 'mutation_receipt': 1},
    'limitations': LIMITATIONS,
    'claim_boundary': CLAIM_BOUNDARY,
    'result': 'Complete' if complete else 'Incomplete',
    'generated_at': generated_at,
}
if workflow_run_id:
    manifest['workflow_run_id'] = int(workflow_run_id)
pathlib.Path(output).write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
PY

sha256sum "${output}" | awk '{print $1 "  " FILENAME}' FILENAME="$(basename "${output}")" \
  > "${output%.json}.sha256"
# awk's FILENAME is not populated for stdin on every implementation; normalize.
digest_value="$(sha256sum "${output}" | awk '{print $1}')"
printf '%s  %s\n' "${digest_value}" "$(basename "${output}")" > "${output%.json}.sha256"
echo "release-manifest: ${output}"
echo "sha256: ${digest_value}"
'''
    write("scripts/generate-release-manifest.sh", generator)


def update_typed_manifest() -> None:
    relative = "crates/allow-report/src/artifacts/release_manifest.rs"
    content = read(relative)
    order_pattern = re.compile(
        r'pub const PUBLISH_ORDER: &\[&str\] = &\[.*?\n\];', re.DOTALL
    )
    replacement = '''pub const PUBLISH_ROWS: &[(&str, &str)] = &[\n'''
    replacement += "".join(f'    ("{name}", "{version}"),\n' for name, version in PUBLISH_ROWS)
    replacement += '''];\n\npub const PUBLISH_ORDER: &[&str] = &[\n'''
    replacement += "".join(f'    "{name}",\n' for name, _ in PUBLISH_ROWS)
    replacement += '''];'''
    content, count = order_pattern.subn(replacement, content, count=1)
    if count != 1:
        raise SystemExit(f"{relative}: publish order block not found")

    start = content.index('    if manifest.auth_source != "oidc" {')
    end = content.index('\n\n    validate_binary_assets(manifest, &mut gaps);', start)
    auth_and_rows = '''    if !matches!(manifest.auth_source.as_str(), "crates_io_api_token" | "oidc") {\n        gaps.push(ManifestGap {\n            field: "auth_source",\n            detail: format!(\n                "expected crates_io_api_token (or historical oidc), got {}",\n                manifest.auth_source\n            ),\n        });\n    }\n    if manifest.crates.len() != PUBLISH_ROWS.len() {\n        gaps.push(ManifestGap {\n            field: "crates",\n            detail: format!(\n                "expected {} crates, got {}",\n                PUBLISH_ROWS.len(),\n                manifest.crates.len()\n            ),\n        });\n    }\n    for (index, (expected_name, expected_version)) in PUBLISH_ROWS.iter().enumerate() {\n        if let Some(crate_entry) = manifest.crates.get(index) {\n            if crate_entry.name != *expected_name {\n                gaps.push(ManifestGap {\n                    field: "crates",\n                    detail: format!(\n                        "position {index}: expected {expected_name}, got {}",\n                        crate_entry.name\n                    ),\n                });\n            }\n            if crate_entry.version != *expected_version {\n                gaps.push(ManifestGap {\n                    field: "crates",\n                    detail: format!(\n                        "{} expected version {}, got {}",\n                        crate_entry.name, expected_version, crate_entry.version\n                    ),\n                });\n            }\n            if crate_entry.crate_checksum.is_none() {\n                gaps.push(ManifestGap {\n                    field: "crates",\n                    detail: format!("{} is missing crate_checksum", crate_entry.name),\n                });\n            }\n        }\n    }'''
    content = content[:start] + auth_and_rows + content[end:]

    old_generator = '''    let crates = PUBLISH_ORDER\n        .iter()\n        .enumerate()\n        .map(|(i, name)| ManifestCrate {\n            name: (*name).to_string(),\n            version: input.version.to_string(),\n            crate_checksum: input.crate_checksums.get(i).cloned().flatten(),\n            registry_checksum: None,\n        })\n        .collect();'''
    new_generator = '''    let crates = PUBLISH_ROWS\n        .iter()\n        .enumerate()\n        .map(|(index, (name, version))| ManifestCrate {\n            name: (*name).to_string(),\n            version: (*version).to_string(),\n            crate_checksum: input.crate_checksums.get(index).cloned().flatten(),\n            registry_checksum: None,\n        })\n        .collect();'''
    if old_generator not in content:
        raise SystemExit(f"{relative}: typed generator seam not found")
    content = content.replace(old_generator, new_generator, 1)
    content = content.replace(
        '/// auth_source is oidc, and the manifest is safe to attest.',
        '/// auth_source is an approved bounded crates.io authentication class.',
    )
    content = content.replace(
        '/// How the release authenticated with crates.io: `oidc` (only accepted\n    /// value for attestation; `secret` produces `Incomplete`).',
        '/// How the release authenticated with crates.io. The selected v0.2.0\n    /// path records `crates_io_api_token`; `oidc` remains readable historically.',
    )
    write(relative, content)


def update_schema_and_contract_tests() -> None:
    replace_once(
        "docs/schemas/release-manifest.schema.json",
        '"auth_source": { "const": "oidc" }',
        '"auth_source": { "enum": ["crates_io_api_token", "oidc"] }',
    )

    relative = "crates/cargo-allow/src/release_prep_tests.rs"
    content = read(relative)
    old_auth = '''    assert!(\n        workflow.contains("rust-lang/crates-io-auth-action@"),\n        "{RELEASE_WORKFLOW} should authenticate with crates.io Trusted Publishing"\n    );'''
    new_auth = '''    assert!(\n        workflow.contains("CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}")\n            && workflow.contains("source=crates_io_api_token"),\n        "{RELEASE_WORKFLOW} should use the bounded crates.io API token"\n    );\n    assert!(\n        !workflow.contains("rust-lang/crates-io-auth-action@"),\n        "{RELEASE_WORKFLOW} should not require crates.io OIDC"\n    );'''
    if old_auth not in content:
        raise SystemExit(f"{relative}: old auth assertion not found")
    content = content.replace(old_auth, new_auth, 1)
    content = content.replace(
        'workflow.contains("needs: [install-smoke, publish]")',
        'workflow.contains("needs: [preflight, install-smoke, publish]")',
        1,
    )
    content = content.replace(
        '"cargo-allow-${{ github.ref_name }}-x86_64-unknown-linux-gnu.tar.gz",',
        '"cargo-allow-${{ needs.preflight.outputs.tag }}-x86_64-unknown-linux-gnu.tar.gz",',
        1,
    )
    content = content.replace('auth_source: "oidc",', 'auth_source: "crates_io_api_token",', 1)
    write(relative, content)


def rewrite_release_docs() -> None:
    publish_order = "\n".join(
        f"{index}. {name}" for index, name in enumerate(ALL_TOPOLOGY_ORDER, start=1)
    )
    record = f'''# 0.2.0 Release Record

> **Candidate status:** the exact mixed-version release implementation is on
> `main`; crates.io publication, `v0.2.0`, and the public GitHub Release occur
> only after the one-file authorization commit passes its own exact checks.

0.2.0 is the first cargo-allow release on Rust 1.95 and the first release built
from the separated 22-package architecture. The supported product remains the
cargo-allow source-syntax policy linter and durable exception ledger.
`cargo-intent`, `cargo-proof`, and the shared `effortless-*` packages are
published at 0.1.0 as experimental or registry-transitive components; their
publication does not make them part of cargo-allow's supported CLI surface.

## What changed

- `cargo-allow`, `cargo-intent`, and `cargo-proof` now have distinct package and
  version lines inside one monorepo.
- The intent implementation package is `intent-compiler`; its Rust library name
  remains `intent_engine`.
- The proof implementation package is `proof-orchestrator`; its Rust library
  name remains `proof_engine`.
- Shared repository protocol, snapshot, edit, and Rust subject-index contracts
  live under the `effortless-*` package names.
- The final publisher derives package membership, exact versions, and dependency
  order from `policy/product-package-topology-v2.toml`.
- GitHub Actions publishes with the bounded `CARGO_REGISTRY_TOKEN` secret. No
  crates.io Trusted Publishing/OIDC configuration is required.
- Every upload is followed by crates.io visibility and checksum verification
  before a dependent package can publish.
- Post-publication proof installs exact `cargo-allow 0.2.0` from crates.io on
  hosted Linux and Windows runners.
- The GitHub Release remains draft until the publication receipt, Complete
  release manifest, manifest attestation, Linux archive, checksums, and clean
  archive-install receipt are attached.

## Operator workflow

```text
doctor / audit
→ why --kind ... --path ... --line ...
→ why --plan
→ add --from-plan --update
→ check --mode no-new
→ list / explain / worklist
→ diff --base <base>
```

Notable operator changes include the single-file `why` fast path, stale-safe
plan/application handoff, atomic in-place ledger mutation, consistent next-action
summaries, scanner-completeness propagation, and stronger target-identity checks.

## Scanner capability contract

```bash
cargo-allow capabilities --format json
```

The output schema is `cargo-allow.sensor-capabilities.v1`, generation 1. It
states which source-tree sensors ran and which claims remain excluded. It is not
a compilation, type-analysis, macro-expansion, MIR, runtime, test-adequacy, or
coverage-proof contract.

## Install

After the release workflow reports Complete:

```bash
cargo install cargo-allow --version 0.2.0 --locked
cargo-allow --version
```

The expected version is `cargo-allow 0.2.0`. A verified
`x86_64-unknown-linux-gnu` archive is also attached to the GitHub Release. No
Windows or macOS prebuilt archive is claimed by this cut.

## Publication phases

### Phase 1 — experimental namespace graph

The authorization workflow publishes twelve real 0.1.0 packages: four shared,
five cargo-intent, and three cargo-proof packages. It rejects missing package
closure, stale names, wrong versions, and checksum conflicts.

### Phase 2 — supported cargo-allow graph

The cargo-allow candidate contains thirteen registry rows: ten cargo-allow
packages at 0.2.0 plus `effortless-repo-protocol`,
`effortless-repo-snapshot`, and `effortless-repo-edit` at 0.1.0. The shared
rows are verified as already-published exact bytes; only missing cargo-allow
rows are uploaded.

## Topology authority order

```text
{publish_order}
```

This is the unique 22-package topology order. The two-phase publisher may verify
an already-visible row rather than uploading it again.

## Release verification

The clean closeout requires all of the following on one exact candidate:

- ordinary CI, Windows, coverage, MSRV, package-smoke, deny, shallow-diff,
  operator-latency, Changie, and review contracts green;
- twelve namespace rows and thirteen cargo-allow candidate rows derived from V2;
- exact crates.io checksum equality for every registry row;
- exact Linux and Windows `cargo install` smoke;
- Complete release manifest with `auth_source = crates_io_api_token`;
- verified GitHub provenance for the Linux archive and release manifest;
- a public GitHub Release containing the complete checked asset set.

## Claim boundary and limitations

cargo-allow scans selected source-tree/source-syntax surfaces and maintains a
durable exception ledger. It does not execute target repository code and does
not claim compiler, type, macro-expanded, MIR, control-flow, data-flow,
unsafe-proof, test-adequacy, or coverage-proof behavior. Experimental sibling
products may evolve independently after their initial 0.1.0 namespace release.

## Upgrade and rollback

Upgrade from the published 0.1.11 line with the exact install command above.
Before rollback, preserve the current policy and receipts. Reinstall 0.1.11 with:

```bash
cargo install cargo-allow --version 0.1.11 --locked --force
```

Published crate bytes are immutable. A defective 0.2.0 package must be yanked or
superseded by a new version; the tag or package bytes must never be replaced.
'''
    write("docs/release/0.2.0.md", record)

    github_notes = '''# cargo-allow v0.2.0

cargo-allow 0.2.0 is the first Rust 1.95 release and the first release from the
separated three-product package architecture.

## Highlights

- **Stale-safe finding-to-receipt loop** — `why --plan` and
  `add --from-plan --update` bind human judgment to a fresh source, policy, and
  finding identity before atomically updating the ledger.
- **Faster one-finding diagnosis** — `why` scans the selected file rather than
  the full repository for advisory explanation.
- **Actionable command summaries** — `doctor`, `audit`, `check`, `list`,
  `explain`, `why`, `worklist`, and mutation commands share a compact operator
  grammar without replacing their detailed artifacts.
- **Stronger trust boundaries** — scanner completeness, mutation-target
  identity, repository containment, and release package identities fail closed.
- **Updated crate layout** — the monorepo now contains independent cargo-allow,
  cargo-intent, cargo-proof, and `effortless-*` package families. The supported
  cargo-allow install remains independent of the experimental sibling products.
- **Exact crates.io publication** — package rows and mixed versions are derived
  from V2 topology, published with the repository's bounded crates.io API token,
  and checksum-verified before dependants advance.
- **Published-install proof** — exact cargo-allow 0.2.0 installs are exercised on
  hosted Linux and Windows. The release also includes one attested
  `x86_64-unknown-linux-gnu` archive.

## Install

```bash
cargo install cargo-allow --version 0.2.0 --locked
cargo-allow --version
```

## Package layout

The release registers the shared, cargo-intent, and cargo-proof 0.1.0 package
names, including `intent-compiler` and `proof-orchestrator`. Those packages are
experimental. The supported cargo-allow product resolves a 13-row mixed-version
graph: ten cargo-allow packages at 0.2.0 plus three exact shared 0.1.0 packages.

## Claim boundary

cargo-allow is a source-syntax policy linter and durable exception ledger. It is
not a compiler, Clippy replacement, semantic proof system, or universal
repository correctness claim.

See the attached release manifest and publication receipt for exact package,
checksum, source, platform, and limitation evidence. The detailed release record
is in `docs/release/0.2.0.md`.
'''
    write("docs/release/github/v0.2.0.md", github_notes)

    relative = "docs/release/README.md"
    content = read(relative)
    opening = '''# Release on Tag\n\nFuture cargo-allow releases publish from GitHub Actions when a version tag is\npushed. Manual `cargo publish` remains a documented fallback during Trusted\nPublishing setup or when automation is blocked.\n'''
    replacement = '''# Authorized release publication\n\nThe selected release path is GitHub Actions plus the repository's bounded\n`CARGO_REGISTRY_TOKEN` secret. A normal tag push or manual workflow dispatch is\na zero-upload rehearsal. Real publication begins only when the one-file\nauthorization commit triggers `release-authorized.yml`, which publishes the\nnamespace graph, creates the exact tag, and dispatches the guarded final\nrelease workflow.\n'''
    if opening not in content:
        raise SystemExit(f"{relative}: opening release text changed")
    content = content.replace(opening, replacement, 1)
    block = f'''## Prerequisites\n\n| Prerequisite | Verification |\n| --- | --- |\n| Bounded crates.io token | Repository secret `CARGO_REGISTRY_TOKEN` exists; its value is never logged, hashed, or retained |\n| Exact package topology | V2 derives 12 namespace rows and a 13-row cargo-allow candidate |\n| Green candidate | Linux, Windows, coverage, MSRV, package, deny, shallow-diff, operator-latency, Changie, and review contracts pass on the exact candidate |\n| Release documentation | `CHANGELOG.md`, `docs/release/0.2.0.md`, and `docs/release/github/v0.2.0.md` describe the same package and support boundary |\n| Explicit authorization | One commit changes only `release/authorize-v0.2.0.json` and binds the candidate commit/tree plus Cargo.lock and topology digests |\n\n## Canonical path\n\n1. Merge release-closeout changes and obtain green current-main evidence.\n2. Merge the authorization-only PR.\n3. `release-authorized.yml` re-proves the exact parent candidate, publishes and verifies the twelve namespace rows, then creates `v0.2.0`.\n4. The controller dispatches `release.yml` with the exact tag, commit, tree, and authorization reference.\n5. `release.yml` verifies the three shared rows, publishes missing cargo-allow 0.2.0 rows, runs exact Linux and Windows crates.io installs, builds and attests the Linux archive, emits a Complete manifest, and publishes the GitHub Release.\n6. A reconciliation PR updates registry state, public install pins, support projections, and issue closeout from the retained receipts.\n\nA bare tag push does not authorize publication. An ordinary workflow dispatch performs no upload.\n\n## Publication phases\n\nThe namespace phase publishes four shared, five cargo-intent, and three cargo-proof packages at 0.1.0. The supported cargo-allow phase uses ten 0.2.0 packages plus three exact shared 0.1.0 rows. The sibling products remain experimental and are not pulled into the cargo-allow supported product merely because their names are registered.\n\n## Topology authority order\n\n```text\n{publish_order}\n```\n\nThe publisher derives this order and each package version from `policy/product-package-topology-v2.toml`; workflow arrays are not release authority. It runs `cargo publish --dry-run` immediately before each missing upload and waits for exact crates.io checksum visibility before advancing to a dependent row.\n\n## Manual rehearsal\n\nRun **Actions → Release → Run workflow** on `main` with `publish_recovery` left false. The run must prove preflight and package-row derivation, emit bounded receipts, and perform zero upload. Real publication is reserved for the exact dispatch from `release-authorized.yml`.\n\n'''
    content, count = re.subn(
        r'## Prerequisites\n.*?(?=## Recovery)',
        block,
        content,
        count=1,
        flags=re.DOTALL,
    )
    if count != 1:
        raise SystemExit(f"{relative}: prerequisite/release path block not found")
    write(relative, content)


def prepare_changie() -> None:
    relative = "CHANGELOG.md"
    content = read(relative)
    content, count = re.subn(
        r'\n## \[0\.2\.0\] - [^\n]+\n.*?(?=\n## \[0\.1\.11\])',
        "\n",
        content,
        count=1,
        flags=re.DOTALL,
    )
    if count != 1:
        raise SystemExit(f"{relative}: expected one premature 0.2.0 section")
    write(relative, content)
    version_file = ROOT / "0.2.0.md"
    if version_file.exists():
        version_file.unlink()
    fragment = ROOT / ".changes/Fixed-20260811-release-topology-token.yaml"
    fragment.write_text(
        "kind: Fixed\n"
        "body: >-\n"
        "  Route the 0.2.0 release through exact topology-derived mixed-version package rows, the bounded crates.io API token, per-row registry checksum verification, and Linux/Windows published-install closeout.\n",
        encoding="utf-8",
    )


def main() -> None:
    enable_namespace_manifests()
    enable_namespace_topology()
    update_topology_publisher()
    rewrite_release_workflow()
    rewrite_manifest_generator()
    update_typed_manifest()
    update_schema_and_contract_tests()
    rewrite_release_docs()
    prepare_changie()


if __name__ == "__main__":
    main()
