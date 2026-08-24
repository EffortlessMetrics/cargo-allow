#!/usr/bin/env bash
# Generate a cargo-allow.release-manifest.v2 envelope from release
# workflow context. Runs after publish + install-smoke succeed.
#
# Inputs (environment / arguments):
#   VERSION        workspace version (e.g. 0.2.0)
#   REPOSITORY     GitHub repository full name (e.g. EffortlessMetrics/cargo-allow)
#   TAG            git tag ref (e.g. v0.2.0)
#   COMMIT         commit SHA
#   TREE           tree SHA (optional; derived from commit if absent)
#   AUTH_SOURCE    "crates_io_api_token"
#   WORKFLOW_RUN_ID  GitHub Actions run ID (optional)
#   MSRV           minimum supported Rust version
#   PLATFORMS      space-separated proven platforms (e.g. "linux")
#   BINARY_PACKAGE_RECEIPT  verified binary package receipt (optional)
#   BINARY_INSTALL_RECEIPT  clean-install receipt (optional)
#   RUST_TOOLCHAIN  toolchain used for the binary (default: stable)
#   RUNNER          runner used for the binary (default: ubuntu-latest)
#   TOPOLOGY_RECEIPT  exact topology publisher receipt
#   OUTPUT         output path (default: target/cargo-allow/release-manifest-v2.json)
#
# Outputs:
#   release-manifest-v2.json — the typed manifest envelope
#   release-manifest-v2.sha256 — SHA-256 of the manifest bytes
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
topology_receipt="${TOPOLOGY_RECEIPT:-target/cargo-allow/topology-publish.receipt.json}"
output="${OUTPUT:-target/cargo-allow/release-manifest-v2.json}"
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
  "${rust_toolchain}" "${runner}" "${topology_receipt}" <<'PY'
import hashlib
import json
import pathlib
import re
import sys

(
    output, version, repository, tag, commit, tree, auth_source, workflow_run_id,
    msrv, platforms_text, generated_at, package_path, install_path,
    rust_toolchain, runner, topology_receipt_path,
) = sys.argv[1:]
CLAIM_BOUNDARY = "topology-derived package identity and release evidence only; this manifest does not authorize publication or claim source, runtime, platform, or support completeness"
LIMITATIONS = ["candidate rows come from the exact topology publisher receipt", "binary evidence is limited to the referenced receipt and verified target"]


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
        if not isinstance(payload, dict):
            raise ValueError("JSON payload is not an object")
    except (OSError, json.JSONDecodeError, ValueError) as error:
        raise SystemExit(f"release-manifest: invalid receipt {path}: {error}")
    if payload.get("schema_id") != schema or payload.get("schema_version") != 1:
        raise SystemExit(f"release-manifest: unsupported receipt schema in {path}")
    return receipt_path, payload


topology_file, topology = receipt(topology_receipt_path, "cargo-allow.topology-publish-receipt.v1")
topology_file = topology_file.resolve()
if topology.get("commit") != commit or topology.get("tree") != tree:
    raise SystemExit("release-manifest: topology receipt identity disagrees with release")
if topology.get("mode") != "cargo-allow" or topology.get("complete") is not True:
    raise SystemExit("release-manifest: topology receipt is not a complete cargo-allow candidate")
raw_rows = topology.get("rows")
if not isinstance(raw_rows, list) or not raw_rows:
    raise SystemExit("release-manifest: topology receipt has no package rows")
publish = topology.get("publish")
if not isinstance(publish, bool):
    raise SystemExit("release-manifest: topology receipt publish field must be a Boolean")
if publish:
    if topology.get("complete") is not True:
        raise SystemExit("release-manifest: published topology receipt is not complete")
    if topology.get("incident_state") != "none":
        raise SystemExit("release-manifest: published topology receipt records an incident")
    first_irreversible_row = topology.get("first_irreversible_row")
    if not isinstance(first_irreversible_row, int) or first_irreversible_row <= 0:
        raise SystemExit(
            "release-manifest: published topology receipt lacks an irreversible-row marker"
        )


def valid_digest(value):
    return (
        isinstance(value, str)
        and value.startswith("sha256:")
        and len(value) == len("sha256:") + 64
        and all(char in "0123456789abcdef" for char in value[len("sha256:"):])
    )


def valid_unprefixed_digest(value):
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(char in "0123456789abcdef" for char in value)
    )


SEMVER_PATTERN = re.compile(
    r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$"
)


def valid_semver(value):
    return isinstance(value, str) and bool(SEMVER_PATTERN.match(value))


if not valid_unprefixed_digest(topology.get("topology_sha256")):
    raise SystemExit("release-manifest: malformed topology digest")
if not valid_unprefixed_digest(topology.get("cargo_lock_sha256")):
    raise SystemExit("release-manifest: malformed Cargo.lock digest")


def artifact_reference(path):
    candidate = pathlib.Path(path)
    if not candidate.is_absolute():
        return candidate.as_posix()
    try:
        return candidate.resolve().relative_to(pathlib.Path.cwd()).as_posix()
    except ValueError:
        # Fixture and downloaded-artifact callers may stage the receipt outside
        # the checkout. Keep the envelope repository/artifact-relative without
        # leaking an absolute runner path.
        return f"artifact/{candidate.name}"


rows = []
seen_names = set()
seen_logical_ids = set()
seen_orders = set()
for raw in raw_rows:
    required = ("logical_id", "name", "version", "release_order", "local_checksum")
    if any(not isinstance(raw.get(field), (str, int)) or str(raw.get(field)).strip() == "" for field in required):
        raise SystemExit("release-manifest: topology receipt row is incomplete")
    if not isinstance(raw["logical_id"], str) or not isinstance(raw["name"], str):
        raise SystemExit("release-manifest: topology receipt identity fields must be strings")
    if not isinstance(raw["version"], str) or not valid_semver(raw["version"]):
        raise SystemExit(f"release-manifest: malformed package version for {raw['name']}")
    name = raw["name"]
    order = int(raw["release_order"])
    if order <= 0 or name in seen_names or raw["logical_id"] in seen_logical_ids or order in seen_orders:
        raise SystemExit("release-manifest: topology receipt contains duplicate package identity")
    seen_names.add(name)
    seen_logical_ids.add(raw["logical_id"])
    seen_orders.add(order)
    if not valid_digest(raw["local_checksum"]):
        raise SystemExit(f"release-manifest: malformed crate digest for {name}")
    if publish:
        if raw.get("state") not in {"published_verified", "verified_existing"}:
            raise SystemExit(
                f"release-manifest: published topology row {name} is not registry-verified"
            )
        registry_checksum = raw.get("registry_checksum")
        if not registry_checksum or not valid_digest(registry_checksum):
            raise SystemExit(
                f"release-manifest: registry checksum missing or malformed for published row {name}"
            )
        if raw.get("state") == "published_verified" and registry_checksum != raw["local_checksum"]:
            raise SystemExit(
                f"release-manifest: registry checksum disagrees for published row {name}"
            )
    row = {
        "logical_id": raw["logical_id"],
        "package_name": name,
        "package_version": raw["version"],
        "release_order": order,
        "crate_digest": raw.get("registry_checksum") or raw["local_checksum"],
    }
    registry_checksum = raw.get("registry_checksum")
    if registry_checksum is not None:
        if not valid_digest(registry_checksum):
            raise SystemExit(f"release-manifest: malformed registry checksum for {name}")
        row["registry_checksum"] = registry_checksum
    rows.append(row)
rows.sort(key=lambda row: row["release_order"])

platforms = platforms_text.split()
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
        "executable_sha256", "tag", "commit", "tree",
    )
    if any(field not in package or field not in install for field in fields):
        raise SystemExit("release-manifest: binary receipt is missing a required field")
    for field in fields:
        if package[field] != install[field]:
            raise SystemExit(f"release-manifest: binary receipt disagreement in {field}")
    if package["version"] != version:
        raise SystemExit("release-manifest: binary receipt version disagrees with manifest")
    for field, expected in (("tag", tag), ("commit", commit), ("tree", tree)):
        if package[field] != expected:
            raise SystemExit(
                f"release-manifest: binary receipt {field} disagrees with manifest identity"
            )
    if package["target_triple"] != "x86_64-unknown-linux-gnu":
        raise SystemExit("release-manifest: unsupported binary target")
    if "linux" not in platforms:
        raise SystemExit("release-manifest: Linux binary asset requires linux in PLATFORMS")
    expected_archive = f"cargo-allow-v{version}-x86_64-unknown-linux-gnu.tar.gz"
    if package["archive_name"] != expected_archive:
        raise SystemExit("release-manifest: binary archive name is not the stable Linux name")
    if package.get("archive_format") != "tar.gz":
        raise SystemExit("release-manifest: Linux binary archive format must be tar.gz")
    if package.get("executable_name") != "cargo-allow":
        raise SystemExit("release-manifest: binary executable name must be cargo-allow")
    if not rust_toolchain or not runner:
        raise SystemExit("release-manifest: binary toolchain and runner are required")
    for field in ("archive_sha256", "executable_sha256"):
        value = package[field]
        if not valid_digest(value):
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

if auth_source != "crates_io_api_token":
    raise SystemExit("release-manifest: only crates_io_api_token is supported")
manifest = {
    "payload": {
        "schema_id": "cargo-allow.release-manifest.v2",
        "schema_version": 2,
        "operation": "cargo_allow_release",
        "repository": repository,
        "tag_or_authorization": tag,
        "commit": commit,
        "tree": tree,
        "cargo_lock_digest": "sha256:" + topology["cargo_lock_sha256"],
        "architecture_digest": "sha256:" + topology["topology_sha256"],
        "candidate_digest": file_digest(topology_file),
        "package_rows": rows,
        "authentication": "crates_io_api_token",
        "publication_posture": "published" if publish else "unpublished",
        "support_posture": "experimental",
        "limitations": LIMITATIONS,
        "claim_boundary": CLAIM_BOUNDARY,
    },
    "generated_at": generated_at,
    "workflow_path": ".github/workflows/release.yml",
    "workflow_run_id": int(workflow_run_id) if workflow_run_id else None,
    "event": "release",
    "github_ref": tag,
    "artifact_references": [
        artifact_reference(topology_file),
        *(
            [artifact_reference(package_path), artifact_reference(install_path)]
            if package_path and install_path
            else []
        ),
    ],
    "authorization_reference": f"workflow/{workflow_run_id}/crates-io" if workflow_run_id else "workflow/crates-io",
    "instrument_diagnostics": [] if binary_attestation_verified else ["binary attestation is not verified"],
}
manifest = {key: value for key, value in manifest.items() if value is not None}
if workflow_run_id:
    try:
        manifest["workflow_run_id"] = int(workflow_run_id)
    except ValueError:
        raise SystemExit(f"release-manifest: invalid workflow_run_id: {workflow_run_id}")
pathlib.Path(output).write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
PY

# Compute a sha256sum-compatible checksum record for the manifest.  The
# release workflow verifies this file from the artifact directory, so record
# the manifest basename rather than the generator's possibly absolute path.
sha256sum "${output}" | awk -v file="$(basename "${output}")" '{print $1 "  " file}' > "${output%.json}.sha256"

echo "release-manifest: ${output}"
echo "sha256: $(cat "${output%.json}.sha256")"
