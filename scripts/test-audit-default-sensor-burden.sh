#!/usr/bin/env bash
# Characterization checks for scripts/audit-default-sensor-burden.sh (#4360).
#
# Proves the CI-used harness fails closed without running the full audit:
# a missing cargo-allow binary must stop the harness before any fixture or
# receipt work, and a failing cargo-allow invocation must abort the harness
# instead of being swallowed into a partial receipt.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

expect_failure_with() {
  local label="$1" needle="$2"
  shift 2
  local output status
  output="$("$@" 2>&1)" && status=0 || status=$?
  if [[ "${status}" -eq 0 ]]; then
    printf 'fail %s (expected non-zero exit)\n' "${label}" >&2
    exit 1
  fi
  if ! grep -qF -- "${needle}" <<<"${output}"; then
    printf 'fail %s (exit %s, output missing %s)\n' "${label}" "${status}" "${needle}" >&2
    printf '%s\n' "${output}" >&2
    exit 1
  fi
  printf 'ok %s\n' "${label}"
}

# Case 1: missing binary. The harness must exit non-zero and name the
# missing binary plus the build remediation, so a scheduled run cannot
# produce a receipt without measuring anything.
expect_failure_with "missing cargo-allow binary fails closed" \
  "cargo-allow binary not found" \
  env CARGO_ALLOW_BIN="${work}/no-such-cargo-allow" \
    WORK_DIR="${work}/missing-bin" \
    bash scripts/audit-default-sensor-burden.sh

# Case 2: the harness honors WORK_DIR even on the fail-closed path; the
# work directory is created before the binary check.
if [[ -d "${work}/missing-bin" ]]; then
  printf 'ok WORK_DIR is honored on the fail-closed path\n'
else
  printf 'fail WORK_DIR is not honored on the fail-closed path\n' >&2
  exit 1
fi

# Case 3: a failing cargo-allow invocation aborts the harness. A stub
# binary that exits non-zero with a marker on stderr must surface both:
# no partial receipt and no swallowed failure.
stub="${work}/stub-cargo-allow"
cat > "${stub}" <<'EOF'
#!/usr/bin/env bash
printf 'stub-cargo-allow-failure-marker\n' >&2
exit 3
EOF
chmod +x "${stub}"
expect_failure_with "failing cargo-allow invocation aborts the harness" \
  "stub-cargo-allow-failure-marker" \
  env CARGO_ALLOW_BIN="${stub}" \
    WORK_DIR="${work}/stub-bin" \
    bash scripts/audit-default-sensor-burden.sh

printf 'all audit-default-sensor-burden characterization checks passed\n'
