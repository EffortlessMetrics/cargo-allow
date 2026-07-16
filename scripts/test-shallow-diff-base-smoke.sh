#!/usr/bin/env bash
# Characterization wrapper for scripts/shallow-diff-base-smoke.sh (#2366).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

work_dir="${ROOT}/target/shallow-diff-base-smoke-test"
rm -rf "${work_dir}"

WORK_DIR="${work_dir}" bash scripts/shallow-diff-base-smoke.sh

receipt="${work_dir}/shallow-diff-base-smoke.receipt.txt"
[[ -f "${receipt}" ]] || {
  printf 'missing receipt %s\n' "${receipt}" >&2
  exit 1
}
grep -q 'schema_id=cargo-allow.shallow-diff-base-smoke.v1' "${receipt}"
grep -q 'shallow_negative=pass' "${receipt}"
grep -q 'full_history_positive=pass' "${receipt}"
grep -q 'result=pass' "${receipt}"

printf 'ok shallow-diff-base-smoke characterization\n'
