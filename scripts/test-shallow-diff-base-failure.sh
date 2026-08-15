#!/usr/bin/env bash
# Deterministic characterization of the full-history failure artifact path.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work_dir="${ROOT}/target/shallow-diff-base-failure-test"
fake_dir="${ROOT}/target/shallow-diff-base-failure-bin"
rm -rf "${work_dir}" "${fake_dir}"
mkdir -p "${fake_dir}"
fake_bin="${fake_dir}/cargo-allow"
cat >"${fake_bin}" <<'FAKE'
#!/usr/bin/env bash
if [[ "${1:-}" == "--version" ]]; then
  printf 'cargo-allow 0.0.0-failure-probe\n'
  exit 0
fi
printf 'injected full-history diagnostic\n' >&2
exit 17
FAKE
chmod +x "${fake_bin}"

set +e
WORK_DIR="${work_dir}" CARGO_ALLOW_BIN="${fake_bin}" \
  bash "${ROOT}/scripts/shallow-diff-base-smoke.sh"
code=$?
set -e
[[ "${code}" -ne 0 ]] || { printf 'failure probe unexpectedly passed\n' >&2; exit 1; }

receipt="${work_dir}/shallow-diff-base-smoke.receipt.txt"
artifact_dir="${work_dir}/artifacts"
grep -q '^full_history_positive=fail$' "${receipt}"
grep -q '^result=fail$' "${receipt}"
stderr_artifact="$(sed -n 's/^full_history_error_artifact=//p' "${receipt}")"
[[ -f "${stderr_artifact}" ]]
grep -q 'injected full-history diagnostic' "${stderr_artifact}"
[[ -f "${artifact_dir}/full-history.stdout.txt" ]]

rm -rf "${work_dir}" "${fake_dir}"
printf 'ok shallow-diff-base-smoke failure characterization\n'
