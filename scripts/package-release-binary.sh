#!/usr/bin/env bash
# Build the reviewed Linux release archive envelope for cargo-allow (#2464).
# This script packages bytes; it does not attest or publish them.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

target="x86_64-unknown-linux-gnu"
version="${VERSION:-}"
output_dir="${OUTPUT_DIR:-${ROOT}/target/cargo-allow/release-assets}"

log() { printf 'package-release-binary: %s\n' "$*"; }
fail() { printf 'package-release-binary: error: %s\n' "$*" >&2; exit 1; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) [[ $# -ge 2 ]] || fail "--version requires a value"; version="$2"; shift 2 ;;
    --target) [[ $# -ge 2 ]] || fail "--target requires a value"; target="$2"; shift 2 ;;
    --output-dir) [[ $# -ge 2 ]] || fail "--output-dir requires a value"; output_dir="$2"; shift 2 ;;
    *) fail "unknown argument: $1" ;;
  esac
done

[[ "${target}" == "x86_64-unknown-linux-gnu" ]] \
  || fail "unsupported target ${target}; this lane is Linux-only"
for command_name in cargo tar gzip sha256sum python3; do
  command -v "${command_name}" >/dev/null 2>&1 || fail "${command_name} is required"
done

read_workspace_version() {
  awk '
    /^[[:space:]]*\[workspace\.package\]/ { in_ws = 1; next }
    /^[[:space:]]*\[/ { if (in_ws) exit }
    in_ws && /^[[:space:]]*version[[:space:]]*=/ {
      gsub(/^[[:space:]]*version[[:space:]]*=[[:space:]]*"/, "", $0)
      gsub(/".*/, "", $0)
      print $0
      exit
    }
  ' Cargo.toml
}

if [[ -z "${version}" ]]; then version="$(read_workspace_version)"; fi
[[ -n "${version}" ]] || fail "could not determine workspace version"
[[ "${version}" != *[[:space:]/\\]* ]] || fail "version contains whitespace or a path separator"

archive_root="cargo-allow-v${version}-${target}"
archive_name="${archive_root}.tar.gz"
mkdir -p "${output_dir}"

bin="${CARGO_ALLOW_BIN:-${ROOT}/target/${target}/release/cargo-allow}"
if [[ -z "${CARGO_ALLOW_BIN:-}" ]]; then
  log "building cargo-allow ${version} for ${target}"
  cargo build -p cargo-allow --bin cargo-allow --release --locked --target "${target}"
fi
[[ -f "${bin}" && -x "${bin}" ]] || fail "expected executable at ${bin}"
reported_version="$("${bin}" --version)" || fail "executable could not run --version"
[[ "${reported_version}" == "cargo-allow ${version}" ]] \
  || fail "executable reported ${reported_version}, expected cargo-allow ${version}"

stage="$(mktemp -d "${TMPDIR:-/tmp}/cargo-allow-release.XXXXXX")"
cleanup() { rm -rf "${stage:-}"; }
trap cleanup EXIT
archive_tree="${stage}/${archive_root}"
mkdir -p "${archive_tree}"
cp "${bin}" "${archive_tree}/cargo-allow"
cp LICENSE-APACHE LICENSE-MIT "${archive_tree}/"
cat >"${archive_tree}/README.md" <<EOF
# cargo-allow ${version}

Target: ${target}

This archive contains the cargo-allow executable for the exact target above.
Verify the archive checksum and the release attestation before extraction.
The source-build fallback is documented at:
https://github.com/EffortlessMetrics/cargo-allow/blob/main/docs/release/README.md
EOF
cat >"${archive_tree}/VERIFICATION.md" <<EOF
# Verification

Verify the sidecar before extraction:

    sha256sum --check ${archive_name}.sha256

Verify the exact GitHub attestation from a trusted checkout:

    gh attestation verify ${archive_name} --repo EffortlessMetrics/cargo-allow

After extraction, check the executable identity:

    ./cargo-allow --version

This archive does not claim universal Linux, musl, or CPU compatibility.
EOF
chmod 0755 "${archive_tree}/cargo-allow"
chmod 0755 "${archive_tree}"
chmod 0644 "${archive_tree}/LICENSE-APACHE" "${archive_tree}/LICENSE-MIT" \
  "${archive_tree}/README.md" "${archive_tree}/VERIFICATION.md"

archive_path="${output_dir}/${archive_name}"
archive_sha_path="${archive_path}.sha256"
executable_sha_path="${archive_path}.executable.sha256"
receipt_path="${output_dir}/release-binary.receipt.json"
log "creating deterministic archive ${archive_name}"
tar --sort=name --owner=0 --group=0 --numeric-owner \
  --mtime='UTC 1970-01-01' -cf - -C "${stage}" "${archive_root}" \
  | gzip -n -c >"${archive_path}"

archive_sha256="$(sha256sum "${archive_path}" | awk '{print $1}')"
executable_sha256="$(sha256sum "${bin}" | awk '{print $1}')"
printf '%s  %s\n' "${archive_sha256}" "${archive_name}" >"${archive_sha_path}"
printf '%s  %s\n' "${executable_sha256}" "cargo-allow" >"${executable_sha_path}"

python3 - "${receipt_path}" "${version}" "${target}" "${archive_name}" \
  "${archive_sha256}" "${executable_sha256}" <<'PY'
import json
import pathlib
import sys

receipt, version, target, archive_name, archive_sha256, executable_sha256 = sys.argv[1:]
payload = {
    "schema_id": "cargo-allow.release-binary-package.v1",
    "schema_version": 1,
    "result": "packaged",
    "version": version,
    "target_triple": target,
    "archive_name": archive_name,
    "archive_format": "tar.gz",
    "archive_sha256": f"sha256:{archive_sha256}",
    "executable_name": "cargo-allow",
    "executable_sha256": f"sha256:{executable_sha256}",
    "claim_boundary": "Bytes were built and packaged for one exact Linux target; attestation, clean-install proof, and publication are not claimed.",
}
pathlib.Path(receipt).write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
PY

log "archive: ${archive_path}"
log "archive sha256: ${archive_sha256}"
log "executable sha256: ${executable_sha256}"
log "receipt: ${receipt_path}"
