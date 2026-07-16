#!/usr/bin/env bash
# Characterization checks for scripts/package-candidate-smoke.sh.
# Offline; does not require crates.io.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

package_dir="${ROOT}/target/package-candidate-smoke-test"
rm -rf "${package_dir}"

PACKAGE_DIR="${package_dir}" bash scripts/package-candidate-smoke.sh

receipt="${package_dir}/package-candidate-smoke.receipt.txt"
[[ -f "${receipt}" ]] || {
  printf 'missing receipt %s\n' "${receipt}" >&2
  exit 1
}
grep -q 'result=pass' "${receipt}"
grep -q 'no_path_deps=cargo-allow' "${receipt}"
grep -q 'packaged=cargo-allow-' "${receipt}"

printf 'ok package-candidate-smoke characterization\n'
