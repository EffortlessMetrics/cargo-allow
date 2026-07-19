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
output="${OUTPUT:-target/cargo-allow/release-manifest-v1.json}"
generated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# Publish-order crate list (must match release.yml and docs/release/README.md)
crates=(
  allow-core
  allow-policy
  allow-inventory
  allow-files
  allow-rust
  allow-match
  allow-report
  allow-policy-legacy
  allow-diff
  cargo-allow
)

# Build crate entries with checksums from target/package/
build_crate_entries() {
  local first=true
  printf '['
  for crate in "${crates[@]}"; do
    if [ "$first" = true ]; then
      first=false
    else
      printf ','
    fi
    local crate_file="target/package/${crate}-${version}.crate"
    local checksum=""
    if [ -f "${crate_file}" ]; then
      checksum=$(sha256sum "${crate_file}" | awk '{print $1}')
    fi
    printf '{"name":"%s","version":"%s"' "${crate}" "${version}"
    if [ -n "${checksum}" ]; then
      printf ',"crate_checksum":"sha256:%s"' "${checksum}"
    fi
    printf '}'
  done
  printf ']'
}

crate_entries=$(build_crate_entries)

# Build platforms JSON array
build_platforms_json() {
  local first=true
  printf '['
  for platform in ${platforms}; do
    if [ "$first" = true ]; then
      first=false
    else
      printf ','
    fi
    printf '"%s"' "${platform}"
  done
  printf ']'
}

platforms_json=$(build_platforms_json)

# Build the manifest JSON
mkdir -p "$(dirname "${output}")"

workflow_field=""
if [ -n "${workflow_run_id}" ]; then
  workflow_field=",\"workflow_run_id\":${workflow_run_id}"
fi

cat >"${output}" <<EOF
{
  "schema_id": "cargo-allow.release-manifest.v1",
  "schema_version": 1,
  "tool_version": "${version}",
  "repository": "${repository}",
  "tag": "${tag}",
  "commit": "${commit}",
  "tree": "${tree}",
  "version": "${version}",
  "crates": ${crate_entries},
  "auth_source": "${auth_source}"${workflow_field},
  "msrv": "${msrv}",
  "platforms_proven": ${platforms_json},
  "generations": {
    "release_manifest": 1,
    "add_finding_plan": 1,
    "mutation_receipt": 1
  },
  "limitations": [
    "source-tree-only scan; cargo-allow does not execute repository code",
    "macro-expanded, type-aware, MIR-level, build-aware, control-flow, data-flow, unsafe-proof, test-adequacy, and coverage-proof behavior were not analyzed"
  ],
  "claim_boundary": "scanned source-tree/source syntax only; cargo-allow did not invoke Cargo metadata, Cargo commands, rustc, Clippy, build scripts, proc macros, external evidence tools, or repository code. Macro expansion, macro token-tree contents, type information, MIR, build output, control flow, and data flow were not analyzed.",
  "generated_at": "${generated_at}"
}
EOF

# Compute SHA-256 of the manifest
sha256sum "${output}" | awk '{print $1}' > "${output%.json}.sha256"

echo "release-manifest: ${output}"
echo "sha256: $(cat "${output%.json}.sha256")"
