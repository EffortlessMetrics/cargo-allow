#!/usr/bin/env bash
# Verify and clean-install-test a cargo-allow Linux release archive (#2464).
# This is consumer-shaped and does not verify GitHub attestations.
set -euo pipefail

archive=""
expected_version=""
receipt=""
log() { printf 'verify-release-binary: %s\n' "$*"; }
fail() { printf 'verify-release-binary: error: %s\n' "$*" >&2; exit 1; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) [[ $# -ge 2 ]] || fail "--version requires a value"; expected_version="$2"; shift 2 ;;
    --receipt) [[ $# -ge 2 ]] || fail "--receipt requires a value"; receipt="$2"; shift 2 ;;
    --) shift; [[ $# -eq 1 ]] || fail "one archive is required"; archive="$1"; shift ;;
    -*) fail "unknown argument: $1" ;;
    *) [[ -z "${archive}" ]] || fail "one archive is required"; archive="$1"; shift ;;
  esac
done

[[ -n "${archive}" ]] || fail "archive is required"
[[ -f "${archive}" ]] || fail "archive does not exist: ${archive}"
for command_name in tar gzip sha256sum python3 git; do
  command -v "${command_name}" >/dev/null 2>&1 || fail "${command_name} is required"
done

archive="$(cd "$(dirname "${archive}")" && pwd)/$(basename "${archive}")"
archive_name="$(basename "${archive}")"
prefix="cargo-allow-v"
suffix="-x86_64-unknown-linux-gnu.tar.gz"
[[ "${archive_name}" == ${prefix}*${suffix} ]] \
  || fail "archive name must be cargo-allow-v<VERSION>-x86_64-unknown-linux-gnu.tar.gz"
version="${archive_name#${prefix}}"
version="${version%${suffix}}"
[[ -n "${version}" && "${version}" != *[[:space:]/\\]* ]] \
  || fail "archive has no valid version in its filename"
if [[ -n "${expected_version}" && "${version}" != "${expected_version}" ]]; then
  fail "archive version ${version} does not match expected ${expected_version}"
fi

sidecar="${archive}.sha256"
[[ -f "${sidecar}" ]] || fail "missing archive checksum sidecar: ${sidecar}"
(
  cd "$(dirname "${archive}")"
  sha256sum --check "$(basename "${sidecar}")" >/dev/null
) || fail "archive checksum sidecar does not match"
executable_sidecar="${archive}.executable.sha256"
[[ -f "${executable_sidecar}" ]] || fail "missing executable checksum sidecar: ${executable_sidecar}"

entries="$(tar -tzf "${archive}")" || fail "archive is not a readable gzip tar"
archive_root="cargo-allow-v${version}-x86_64-unknown-linux-gnu"
expected_entries=(
  "${archive_root}/"
  "${archive_root}/cargo-allow"
  "${archive_root}/LICENSE-APACHE"
  "${archive_root}/LICENSE-MIT"
  "${archive_root}/README.md"
  "${archive_root}/VERIFICATION.md"
)
entry_count=0
while IFS= read -r entry; do
  [[ -n "${entry}" ]] || continue
  [[ "${entry}" != /* && "${entry}" != *'\\'* && "${entry}" != *'../'* && "${entry}" != *'/..'* ]] \
    || fail "archive contains an unsafe path: ${entry}"
  allowed=false
  for expected in "${expected_entries[@]}"; do
    if [[ "${entry}" == "${expected}" ]]; then allowed=true; break; fi
  done
  [[ "${allowed}" == true ]] || fail "archive contains an unexpected entry: ${entry}"
  entry_count=$((entry_count + 1))
done <<<"${entries}"
[[ "${entry_count}" -eq "${#expected_entries[@]}" ]] \
  || fail "archive must contain exactly ${#expected_entries[@]} entries, found ${entry_count}"

work="$(mktemp -d "${TMPDIR:-/tmp}/cargo-allow-release-verify.XXXXXX")"
cleanup() { rm -rf "${work}"; }
trap cleanup EXIT
tar -xzf "${archive}" -C "${work}" --no-same-owner --no-same-permissions
bin="${work}/${archive_root}/cargo-allow"
[[ -x "${bin}" ]] || fail "extracted cargo-allow is not executable"

reported_version="$("${bin}" --version)" || fail "extracted executable could not run --version"
[[ "${reported_version}" == "cargo-allow ${version}"* ]] \
  || fail "executable reported ${reported_version}, expected cargo-allow ${version}"
expected_executable_sha256="$(awk '$2 == "cargo-allow" { print $1; exit }' "${executable_sidecar}")"
[[ "${expected_executable_sha256}" =~ ^[0-9a-f]{64}$ ]] \
  || fail "executable checksum sidecar has no cargo-allow digest"
actual_executable_sha256="$(sha256sum "${bin}" | awk '{print $1}')"
[[ "${actual_executable_sha256}" == "${expected_executable_sha256}" ]] \
  || fail "executable checksum sidecar does not match"

clean_repo="${work}/clean-repository"
mkdir -p "${clean_repo}"
git -C "${clean_repo}" init -q
printf '# clean cargo-allow binary verification\n' >"${clean_repo}/README.md"
(
  cd "${clean_repo}"
  log "cargo-allow --version"; "${bin}" --version >/dev/null
  log "cargo-allow doctor"; "${bin}" doctor >/dev/null
  log "cargo-allow audit"; "${bin}" audit >/dev/null
  log "cargo-allow init --root ."; "${bin}" init --root . >/dev/null
  log "cargo-allow check --mode no-new"; "${bin}" check --mode no-new >/dev/null
  log "cargo-allow --help"; "${bin}" --help >/dev/null
)

archive_sha256="$(sha256sum "${archive}" | awk '{print $1}')"
executable_sha256="${actual_executable_sha256}"
if [[ -z "${receipt}" ]]; then receipt="${archive%.tar.gz}.receipt.json"; fi
mkdir -p "$(dirname "${receipt}")"
python3 - "${receipt}" "${version}" "${archive_name}" "${archive_sha256}" \
  "${executable_sha256}" <<'PY'
import json
import pathlib
import sys

receipt, version, archive_name, archive_sha256, executable_sha256 = sys.argv[1:]
payload = {
    "schema_id": "cargo-allow.release-binary-install.v1",
    "schema_version": 1,
    "result": "pass",
    "version": version,
    "target_triple": "x86_64-unknown-linux-gnu",
    "archive_name": archive_name,
    "archive_sha256": f"sha256:{archive_sha256}",
    "executable_name": "cargo-allow",
    "executable_sha256": f"sha256:{executable_sha256}",
    "clean_install_commands": [
        "cargo-allow --version", "cargo-allow doctor", "cargo-allow audit",
        "cargo-allow init --root .", "cargo-allow check --mode no-new",
        "cargo-allow --help",
    ],
    "attestation_verified": False,
    "claim_boundary": "The archive checksum, envelope, executable version, and clean-repository command surface passed; GitHub attestation and publication are not claimed.",
}
pathlib.Path(receipt).write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
PY

log "verified ${archive_name}"
log "receipt: ${receipt}"
