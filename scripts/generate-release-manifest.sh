#!/usr/bin/env bash
# Generate a cargo-allow.release-manifest.v1 JSON manifest from release
# workflow context. Runs after publish + install-smoke succeed.
#
# Inputs (environment / arguments):
#   VERSION        workspace version (e.g. 0.2.0)
#   REPOSITORY     GitHub repository full name (e.g. EffortlessMetrics/cargo-allow)
#   TAG            git tag ref (e.g. v0.2.0)
#   COMMIT         commit SHA
#   TREE           tree SHA (optional; derived from commit if absent)
#   AUTH_SOURCE    "oidc" or "secret"
#   WORKFLOW_RUN_ID  GitHub Actions run ID (optional)
#   MSRV           minimum supported Rust version
#   PLATFORMS      space-separated proven platforms (e.g. "linux")
#   BINARY_PACKAGE_RECEIPT  verified binary package receipt (optional)
#   BINARY_INSTALL_RECEIPT  clean-install receipt (optional)
#   RUST_TOOLCHAIN  toolchain used for the binary (default: stable)
#   RUNNER          runner used for the binary (default: ubuntu-latest)
#   OUTPUT         output path (default: target/cargo-allow/release-manifest-v1.json)
#
# Outputs:
#   release-manifest-v1.json — the typed manifest
#   release-manifest-v1.sha256 — SHA-256 of the manifest bytes
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

version="${VERSION:?VERSION is required}"
repository="${REPOSITORY:?REPOSITORY is required}"
tag="${TAG:?TAG is required}"
commit="${COMMIT:?COMMIT is required}"
auth_source="${AUTH_SOURCE:?AUTH_SOURCE is required}"
msrv="${MSRV:?MSRV is required}"
tree="${TREE:-$(git rev-parse "${commit}^{tree}" 2>/dev/null || echo "")}"
workflow_run_id="${WORKFLOW_RUN_ID:-}"
platforms="${PLATFORMS:-linux}"
binary_package_receipt="${BINARY_PACKAGE_RECEIPT:-}"
binary_install_receipt="${BINARY_INSTALL_RECEIPT:-}"
rust_toolchain="${RUST_TOOLCHAIN:-stable}"
runner="${RUNNER:-ubuntu-latest}"
output="${OUTPUT:-target/cargo-allow/release-manifest-v1.json}"
generated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

if [[ -n "${binary_package_receipt}" && -z "${binary_install_receipt}" ]] || \
   [[ -z "${binary_package_receipt}" && -n "${binary_install_receipt}" ]]; then
  printf 'release-manifest: error: BINARY_PACKAGE_RECEIPT and BINARY_INSTALL_RECEIPT must be supplied together\n' >&2
  exit 1
fi
command -v python3 >/dev/null 2>&1 || {
  printf 'release-manifest: error: python3 is required\n' >&2
  exit 1
}

# JSON construction and receipt reconciliation stay in one bounded encoder.
# Workflow strings and receipt values are never interpolated as JSON. A binary
# asset remains Incomplete until its attestation verification receipt is green.
mkdir -p "$(dirname "${output}")"

python3 - "${output}" "${version}" "${repository}" "${tag}" "${commit}" \
  "${tree}" "${auth_source}" "${workflow_run_id}" "${msrv}" "${platforms}" \
  "${generated_at}" "${binary_package_receipt}" "${binary_install_receipt}" \
  "${rust_toolchain}" "${runner}" <<'PY'
import hashlib
import json
import pathlib
import sys

(
    output, version, repository, tag, commit, tree, auth_source, workflow_run_id,
    msrv, platforms_text, generated_at, package_path, install_path,
    rust_toolchain, runner,
) = sys.argv[1:]

PUBLISH_ORDER = [
    "allow-core", "allow-policy", "allow-inventory", "allow-files",
    "allow-rust", "allow-match", "allow-policy-legacy", "allow-report",
    "allow-diff", "cargo-allow",
]
CLAIM_BOUNDARY = (
    "scanned source-tree/source syntax only; cargo-allow did not invoke Cargo "
    "metadata, Cargo commands, rustc, Clippy, build scripts, proc macros, "
    "external evidence tools, or repository code. Macro expansion, macro "
    "token-tree contents, type information, MIR, build output, control flow, "
    "and data flow were not analyzed."
)
LIMITATIONS = [
    "source-tree-only scan; cargo-allow does not execute repository code",
    "macro-expanded, type-aware, MIR-level, build-aware, control-flow, data-flow, unsafe-proof, test-adequacy, and coverage-proof behavior were not analyzed",
]


def file_digest(path):
    digest = hashlib.sha256()
    with pathlib.Path(path).open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def receipt(path, schema):
    receipt_path = pathlib.Path(path)
    try:
        payload = json.loads(receipt_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SystemExit(f"release-manifest: invalid receipt {path}: {error}")
    if payload.get("schema_id") != schema or payload.get("schema_version") != 1:
        raise SystemExit(f"release-manifest: unsupported receipt schema in {path}")
    return receipt_path, payload


crates = []
all_crate_checksums = True
for name in PUBLISH_ORDER:
    path = pathlib.Path("target/package") / f"{name}-{version}.crate"
    item = {"name": name, "version": version}
    if path.is_file():
        item["crate_checksum"] = file_digest(path)
    else:
        all_crate_checksums = False
    crates.append(item)

binary_assets = []
binary_attestation_verified = True
if package_path and install_path:
    package_file, package = receipt(
        package_path, "cargo-allow.release-binary-package.v1"
    )
    install_file, install = receipt(
        install_path, "cargo-allow.release-binary-install.v1"
    )
    fields = (
        "version", "target_triple", "archive_name", "archive_sha256",
        "executable_sha256",
    )
    if any(field not in package or field not in install for field in fields):
        raise SystemExit("release-manifest: binary receipt is missing a required field")
    for field in fields:
        if package[field] != install[field]:
            raise SystemExit(f"release-manifest: binary receipt disagreement in {field}")
    if package["version"] != version:
        raise SystemExit("release-manifest: binary receipt version disagrees with manifest")
    if package["target_triple"] != "x86_64-unknown-linux-gnu":
        raise SystemExit("release-manifest: unsupported binary target")
    for field in ("archive_sha256", "executable_sha256"):
        value = package[field]
        if not value.startswith("sha256:") or len(value) != len("sha256:") + 64:
            raise SystemExit(f"release-manifest: malformed binary digest {field}")

    archive_sha = package["archive_sha256"]
    binary_attestation_verified = install.get("attestation_verified") is True
    binary_assets.append({
        "platform": "linux",
        "target_triple": package["target_triple"],
        "version": version,
        "tag": tag,
        "commit": commit,
        "tree": tree,
        "executable_version": version,
        "archive_name": package["archive_name"],
        "archive_format": package.get("archive_format", "tar.gz"),
        "archive_sha256": archive_sha,
        "executable_name": package.get("executable_name", "cargo-allow"),
        "executable_sha256": package["executable_sha256"],
        "rust_toolchain": rust_toolchain,
        "runner": runner,
        "candidate_receipt_digest": file_digest(package_file),
        "installed_smoke_receipt_digest": file_digest(install_file),
        "attestation_subject_sha256": archive_sha,
        "limitations": [
            "x86_64-unknown-linux-gnu proof only; no universal Linux or CPU compatibility claim",
            "the archive is only release-complete after the workflow verifies its attestation",
        ],
    })

manifest = {
    "schema_id": "cargo-allow.release-manifest.v1",
    "schema_version": 1,
    "tool_version": version,
    "repository": repository,
    "tag": tag,
    "commit": commit,
    "tree": tree,
    "version": version,
    "crates": crates,
    "auth_source": auth_source,
    "msrv": msrv,
    "platforms_proven": platforms_text.split(),
    "binary_assets": binary_assets,
    "generations": {"release_manifest": 1, "add_finding_plan": 1, "mutation_receipt": 1},
    "limitations": LIMITATIONS,
    "claim_boundary": CLAIM_BOUNDARY,
    "result": (
        "Complete"
        if all_crate_checksums and auth_source == "oidc" and binary_attestation_verified
        else "Incomplete"
    ),
    "generated_at": generated_at,
}
if workflow_run_id:
    manifest["workflow_run_id"] = int(workflow_run_id)
pathlib.Path(output).write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
PY

# Compute SHA-256 of the manifest
sha256sum "${output}" | awk '{print $1}' > "${output%.json}.sha256"

echo "release-manifest: ${output}"
echo "sha256: $(cat "${output%.json}.sha256")"
