#!/usr/bin/env bash
# Prove the MSRV claim is stated once and repeated consistently everywhere it
# is enforced or published.
#
# `rust-version` under `[workspace.package]` is the source of truth. This
# compares the declared value with the CI toolchain/cache and release
# attestation. It does not invoke rustc or prove a build on the MSRV.
set -euo pipefail

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
msrv_ere="${msrv//./\\.}"
printf 'MSRV source of truth: %s rust-version = %s\n' "${cargo_toml}" "${msrv}"

if ! grep -qF "dtolnay/rust-toolchain@${msrv}.0" "${ci_workflow}"; then
  actual="$(grep -o 'dtolnay/rust-toolchain@[0-9][^ ]*' "${ci_workflow}" | head -n 1 || true)"
  fail "$(printf '%s pins %s but Cargo.toml declares rust-version = \"%s\".\n%s' \
    "${ci_workflow}" "${actual:-no versioned toolchain}" "${msrv}" \
    "       Pin dtolnay/rust-toolchain@${msrv}.0 or update rust-version.")"
fi
printf 'ok %s pins dtolnay/rust-toolchain@%s.0\n' "${ci_workflow}" "${msrv}"

if ! grep -qF "key: msrv-${msrv}" "${ci_workflow}"; then
  actual="$(grep -o 'key: msrv-[^ ]*' "${ci_workflow}" | head -n 1 || true)"
  fail "$(printf '%s uses cache %s but the MSRV is %s.\n%s' \
    "${ci_workflow}" "${actual:-no msrv cache key}" "${msrv}" \
    "       Use key: msrv-${msrv}.")"
fi
printf 'ok %s uses cache key msrv-%s\n' "${ci_workflow}" "${msrv}"

if ! grep -qE "MSRV:[[:space:]]*\"?${msrv_ere}\"?[[:space:]]*$" "${release_workflow}"; then
  actual="$(grep -oE 'MSRV: "?[^"[:space:]]+"?' "${release_workflow}" | head -n 1 || true)"
  fail "$(printf '%s sets %s but the MSRV is %s.\n%s' \
    "${release_workflow}" "${actual:-no MSRV value}" "${msrv}" \
    "       Set MSRV: \"${msrv}\".")"
fi
printf 'ok %s attests MSRV %s\n' "${release_workflow}" "${msrv}"
printf 'MSRV consistency check passed (%s)\n' "${msrv}"
