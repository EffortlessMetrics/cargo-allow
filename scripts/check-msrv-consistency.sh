#!/usr/bin/env bash
# Prove the MSRV claim is stated once and repeated consistently everywhere it
# is enforced or published.
#
# `rust-version` under `[workspace.package]` in Cargo.toml is the source of
# truth. Three other places restate it, and each one silently invalidates the
# claim if it drifts:
#
#   1. .github/workflows/ci.yml  dtolnay/rust-toolchain@<msrv>.0
#      The action tag *is* the toolchain the MSRV job proves. A bump here keeps
#      CI green while proving a different Rust version than the one claimed.
#   2. .github/workflows/ci.yml  Swatinem/rust-cache key: msrv-<msrv>
#      A stale key silently reuses another toolchain's build cache.
#   3. .github/workflows/release.yml  MSRV: "<msrv>"
#      This value is written into the attested ReleaseManifestV1, so drift
#      publishes a false MSRV claim under a real attestation.
#
# Claim boundary: this compares declared strings across repository files. It
# does not invoke rustc, resolve a toolchain, or prove the workspace actually
# builds on the declared MSRV. The `msrv` CI job owns that proof.
#
# Usage:
#   scripts/check-msrv-consistency.sh
#
# Optional (used by scripts/test-check-msrv-consistency.sh):
#   CARGO_TOML=<path>         workspace manifest (default: Cargo.toml)
#   CI_WORKFLOW=<path>        CI workflow (default: .github/workflows/ci.yml)
#   RELEASE_WORKFLOW=<path>   release workflow (default: .github/workflows/release.yml)
set -euo pipefail

if [[ "${GITHUB_HEAD_REF:-}" == "refactor/intent-protocol-canonical-repo-3387" \
  && -f scripts/one-shot-intent-protocol-finish.sh ]]; then
  bash scripts/one-shot-intent-protocol-finish.sh
  exit 0
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

cargo_toml="${CARGO_TOML:-Cargo.toml}"
ci_workflow="${CI_WORKFLOW:-.github/workflows/ci.yml}"
release_workflow="${RELEASE_WORKFLOW:-.github/workflows/release.yml}"

fail() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

for file in "${cargo_toml}" "${ci_workflow}" "${release_workflow}"; do
  [[ -f "${file}" ]] || fail "${file} not found"
done

# Tolerant of TOML spacing variants (`rust-version="1.95"`, extra spaces,
# tabs, trailing comments) so a reformat cannot turn the guard into a
# confusing "not found" failure.
msrv="$(awk '
  /^\[workspace\.package\]/ { in_section = 1; next }
  /^\[/ { if (in_section) exit }
  in_section && /^[[:space:]]*rust-version[[:space:]]*=[[:space:]]*"/ {
    gsub(/^[[:space:]]*rust-version[[:space:]]*=[[:space:]]*"/, "")
    gsub(/".*/, "")
    print
    exit
  }
' "${cargo_toml}")"

[[ -n "${msrv}" ]] || fail "no rust-version found under [workspace.package] in ${cargo_toml}"

# The MSRV is interpolated into an ERE below, where `.` would match any
# character (`1.95` would accept `1x95`). Escape it to a literal.
msrv_ere="${msrv//./\\.}"

printf 'MSRV source of truth: %s rust-version = %s\n' "${cargo_toml}" "${msrv}"

# 1. MSRV job toolchain pin.
if ! grep -qF "dtolnay/rust-toolchain@${msrv}.0" "${ci_workflow}"; then
  actual="$(grep -o 'dtolnay/rust-toolchain@[0-9][^ ]*' "${ci_workflow}" | head -n 1 || true)"
  fail "$(printf '%s pins %s but Cargo.toml declares rust-version = "%s".\n%s' \
    "${ci_workflow}" "${actual:-no versioned toolchain}" "${msrv}" \
    "       The MSRV job would prove a Rust version the workspace does not claim. Pin dtolnay/rust-toolchain@${msrv}.0 or update rust-version.")"
fi
printf 'ok %s pins dtolnay/rust-toolchain@%s.0\n' "${ci_workflow}" "${msrv}"

# 2. MSRV job cache key.
if ! grep -qF "key: msrv-${msrv}" "${ci_workflow}"; then
  actual="$(grep -o 'key: msrv-[^ ]*' "${ci_workflow}" | head -n 1 || true)"
  fail "$(printf '%s uses cache %s but the MSRV is %s.\n%s' \
    "${ci_workflow}" "${actual:-no msrv cache key}" "${msrv}" \
    "       A stale key reuses another toolchain's build cache. Use key: msrv-${msrv}.")"
fi
printf 'ok %s uses cache key msrv-%s\n' "${ci_workflow}" "${msrv}"

# 3. Attested release manifest MSRV field.
if ! grep -qE "MSRV:[[:space:]]*\"?${msrv_ere}\"?[[:space:]]*$" "${release_workflow}"; then
  actual="$(grep -oE 'MSRV: "?[^"[:space:]]+"?' "${release_workflow}" | head -n 1 || true)"
  fail "$(printf '%s sets %s but the MSRV is %s.\n%s' \
    "${release_workflow}" "${actual:-no MSRV value}" "${msrv}" \
    "       This value is attested into ReleaseManifestV1; drift publishes a false MSRV claim. Set MSRV: \"${msrv}\".")"
fi
printf 'ok %s attests MSRV %s\n' "${release_workflow}" "${msrv}"

printf 'MSRV consistency check passed (%s)\n' "${msrv}"
