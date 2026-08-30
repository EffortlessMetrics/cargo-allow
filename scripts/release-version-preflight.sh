#!/usr/bin/env bash
# Release version and artifact preflight for tag-triggered publishes.
#
# Usage:
#   scripts/release-version-preflight.sh [VERSION]
#
# Environment:
#   DRY_RUN=true          Skip release-record file requirements (workflow_dispatch).
#   RELEASE_VERSION       Version override when no positional arg is supplied.
#   RELEASE_IDENTITY_PROJECTION_FILE
#                         When set, the validated typed release-identity JSON
#                         projection is written to this path for callers that
#                         consume its channel/github_prerelease fields.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

log() {
  printf 'release-version-preflight: %s\n' "$*"
}

fail() {
  printf 'release-version-preflight: error: %s\n' "$*" >&2
  exit 1
}

read_workspace_version() {
  awk '
    /^\[workspace\.package\]/ { in_ws = 1; next }
    /^\[/ { if (in_ws) exit }
    in_ws && /^version = / {
      gsub(/^version = "/, "", $0)
      gsub(/".*$/, "", $0)
      print $0
      exit
    }
  ' Cargo.toml
}

read_workspace_dependency_versions() {
  awk '
    /^\[workspace\.dependencies\]/ { in_ws = 1; next }
    /^\[/ { if (in_ws) exit }
    in_ws && /^allow-/ {
      line = $0
      sub(/^[^=]+= \{ path = "[^"]+", version = "/, "", line)
      sub(/".*$/, "", line)
      print line
    }
  ' Cargo.toml
}

validate_typed_release_identity() {
  local release_version="$1"
  local -a command=(
    cargo run --quiet -p cargo-allow --locked --
    release-identity --version "${release_version}"
  )

  if [[ "${GITHUB_EVENT_NAME:-}" == "push" && "${GITHUB_REF_NAME:-}" == v* ]]; then
    command+=(--tag "${GITHUB_REF_NAME}")
  fi

  local projection
  if ! projection="$("${command[@]}")"; then
    fail "typed release identity rejected version/tag inputs"
  fi
  printf '%s\n' "${projection}" | grep -q '"result": "validated"' \
    || fail "typed release identity did not return a validated projection"
  if [[ -n "${RELEASE_IDENTITY_PROJECTION_FILE:-}" ]]; then
    mkdir -p "$(dirname "${RELEASE_IDENTITY_PROJECTION_FILE}")"
    printf '%s\n' "${projection}" > "${RELEASE_IDENTITY_PROJECTION_FILE}" \
      || fail "could not write typed release identity projection ${RELEASE_IDENTITY_PROJECTION_FILE}"
    log "typed release identity projection written to ${RELEASE_IDENTITY_PROJECTION_FILE}"
  fi
  log "typed release identity accepted ${release_version}"
}

version="${1:-${RELEASE_VERSION:-}}"
if [[ -z "${version}" ]]; then
  version="$(read_workspace_version)"
  log "no release version argument; using workspace version ${version}"
fi

workspace_version="$(read_workspace_version)"
[[ -n "${workspace_version}" ]] || fail "could not read [workspace.package].version from Cargo.toml"

# Version grammar, canonical tag identity, and stable-versus-RC channel semantics
# belong to the Rust release identity contract. Shell retains only source-file
# equality checks needed to ensure the selected workspace is the candidate being
# released.
validate_typed_release_identity "${version}"

if [[ "${GITHUB_EVENT_NAME:-}" == "push" && "${GITHUB_REF_NAME:-}" == v* ]]; then
  tag_version="${GITHUB_REF_NAME#v}"
  [[ "${tag_version}" == "${version}" ]] || fail "tag version v${tag_version} does not match release version ${version}"
  [[ "${tag_version}" == "${workspace_version}" ]] || fail "tag version v${tag_version} does not match workspace version ${workspace_version}"
  log "tag v${tag_version} matches workspace version ${workspace_version}"
else
  [[ "${version}" == "${workspace_version}" ]] || fail "release version ${version} does not match workspace version ${workspace_version}"
  log "release version ${version} matches workspace version ${workspace_version}"
fi

while IFS= read -r dep_version; do
  [[ -n "${dep_version}" ]] || continue
  # Workspace dependencies are exact-pinned (`=X.Y.Z`); the requirement
  # operator is stripped before the equality check. Exactness itself is
  # enforced by the release-prep topology tests.
  dep_version="${dep_version#=}"
  [[ "${dep_version}" == "${workspace_version}" ]] || fail "workspace dependency version ${dep_version} does not match workspace version ${workspace_version}"
done < <(read_workspace_dependency_versions)
log "internal workspace dependency versions match ${workspace_version}"

if grep -Eq "^## \\[${version//./\\.}\\]" CHANGELOG.md; then
  log "CHANGELOG.md contains section for ${version}"
else
  fail "CHANGELOG.md is missing a ## [${version}] section"
fi

release_record="docs/release/${version}.md"
github_notes="docs/release/github/v${version}.md"

if [[ "${DRY_RUN:-false}" == "true" ]]; then
  log "DRY_RUN=true; skipping release record checks for ${release_record} and ${github_notes}"
  exit 0
fi

[[ -f "${release_record}" ]] || fail "missing release record ${release_record}"
[[ -f "${github_notes}" ]] || fail "missing GitHub release notes ${github_notes}"
log "release record and GitHub notes exist for ${version}"
