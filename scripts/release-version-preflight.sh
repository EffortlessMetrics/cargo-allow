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

# Pick the first interpreter that actually executes; on Windows the
# python3 name can resolve to a Store alias stub that exits non-zero.
PYTHON=""
for candidate in python3 python; do
  if command -v "${candidate}" >/dev/null 2>&1 \
    && "${candidate}" -c 'import json, sys' >/dev/null 2>&1; then
    PYTHON="${candidate}"
    break
  fi
done
[[ -n "${PYTHON}" ]] || fail "no usable python interpreter (tried python3, python)"

temp_projection=""
cleanup() {
  if [[ -n "${temp_projection}" ]]; then
    rm -f "${temp_projection}"
  fi
}
trap cleanup EXIT

# Validate the selected release identity through the typed Rust authority
# and consume one canonical JSON projection through the bounded reader
# (scripts/release_identity_projection.py). Every downstream identity field
# is read from the projection: the raw caller input is only bound to the
# projected canonical version, and on tag pushes the observed ref is only
# bound to the projected canonical tag. No shell code reconstructs a
# version from tag text or selects a fallback identity (#3787).
validate_typed_release_identity() {
  local release_version="$1"
  local -a command=(
    cargo run --quiet -p cargo-allow --locked --
    release-identity --version "${release_version}"
  )
  local observed_tag=""
  if [[ "${GITHUB_EVENT_NAME:-}" == "push" && "${GITHUB_REF_NAME:-}" == v* ]]; then
    observed_tag="${GITHUB_REF_NAME}"
    command+=(--tag "${observed_tag}")
  fi

  local projection_file="${RELEASE_IDENTITY_PROJECTION_FILE:-}"
  if [[ -z "${projection_file}" ]]; then
    temp_projection="$(mktemp "${TMPDIR:-/tmp}/release-identity-projection.XXXXXX")"
    projection_file="${temp_projection}"
  fi
  mkdir -p "$(dirname "${projection_file}")"
  if ! "${command[@]}" > "${projection_file}"; then
    fail "typed release identity rejected version/tag inputs"
  fi

  local -a reader=(
    "${PYTHON}" scripts/release_identity_projection.py "${projection_file}"
    --require version,tag,github_prerelease
    --expect "version=${release_version}"
  )
  if [[ -n "${observed_tag}" ]]; then
    reader+=(--expect "tag=${observed_tag}")
  fi
  # tr strips Windows console newlines from the reader's KEY=VALUE lines;
  # pipefail keeps the reader's exit status authoritative.
  if ! "${reader[@]}" | tr -d '\r'; then
    fail "typed release identity projection failed the bounded reader"
  fi
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
# released; the comparison target is the projected canonical identity, never
# reconstructed tag text.
projection="$(validate_typed_release_identity "${version}")"

projected_version=""
projected_tag=""
projected_prerelease=""
while IFS='=' read -r key value; do
  case "${key}" in
    version) projected_version="${value}" ;;
    tag) projected_tag="${value}" ;;
    github_prerelease) projected_prerelease="${value}" ;;
  esac
done <<< "${projection}"
[[ -n "${projected_version}" && -n "${projected_tag}" && -n "${projected_prerelease}" ]] \
  || fail "bounded reader did not return the required projected fields"

log "typed release identity accepted ${version} (projected ${projected_version}, tag ${projected_tag}, github_prerelease ${projected_prerelease})"

if [[ "${GITHUB_EVENT_NAME:-}" == "push" && "${GITHUB_REF_NAME:-}" == v* ]]; then
  [[ "${GITHUB_REF_NAME}" == "${projected_tag}" ]] \
    || fail "observed tag ${GITHUB_REF_NAME} differs from projected release tag ${projected_tag}"
  log "observed tag ${GITHUB_REF_NAME} matches projected release tag ${projected_tag}"
fi

[[ "${workspace_version}" == "${projected_version}" ]] \
  || fail "workspace version ${workspace_version} does not match projected release identity ${projected_version}"
log "workspace version matches projected release identity ${projected_version}"

while IFS= read -r dep_version; do
  [[ -n "${dep_version}" ]] || continue
  # Workspace dependencies are exact-pinned (`=X.Y.Z`); the requirement
  # operator is stripped before the equality check. Exactness itself is
  # enforced by the release-prep topology tests.
  dep_version="${dep_version#=}"
  [[ "${dep_version}" == "${projected_version}" ]] || fail "workspace dependency version ${dep_version} does not match projected release identity ${projected_version}"
done < <(read_workspace_dependency_versions)
log "internal workspace dependency versions match ${projected_version}"

if grep -Eq "^## \\[${projected_version//./\\.}\\]" CHANGELOG.md; then
  log "CHANGELOG.md contains section for ${projected_version}"
else
  fail "CHANGELOG.md is missing a ## [${projected_version}] section"
fi

# Release-record paths derive from the projected canonical identity; the
# script selects no alternative record source.
release_record="docs/release/${projected_version}.md"
github_notes="docs/release/github/v${projected_version}.md"

if [[ "${DRY_RUN:-false}" == "true" ]]; then
  log "DRY_RUN=true; release-record requirements for ${release_record} and ${github_notes} are not applicable"
  exit 0
fi

[[ -f "${release_record}" ]] || fail "missing release record ${release_record}"
[[ -f "${github_notes}" ]] || fail "missing GitHub release notes ${github_notes}"
log "release record and GitHub notes exist for ${projected_version}"
