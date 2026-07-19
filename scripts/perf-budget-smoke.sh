#!/usr/bin/env bash
# Performance budget smoke for cargo-allow operator-loop commands.
#
# Measures wall-clock elapsed time for the critical operator-loop commands
# against the cargo-allow repo itself. Produces a structured receipt so
# budgets can be tracked over time and asserted in CI when targets are set.
#
# Usage:
#   scripts/perf-budget-smoke.sh
#
# Optional:
#   OUTPUT_DIR=<path>   receipt output dir (default: target/perf-budget)
#   SKIP_WARMUP=1       skip the warmup audit (reuse existing target/)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

output_dir="${OUTPUT_DIR:-${ROOT}/target/perf-budget}"
receipt="${output_dir}/perf-budget.receipt.txt"

log() {
  printf 'perf-budget: %s\n' "$*"
}

# Measure a command and return elapsed seconds (wall-clock).
# Usage: elapsed=$(time_cmd "label" command args...)
time_cmd() {
  local label="$1"; shift
  local start end
  start=$(date +%s%N)
  "$@" >/dev/null 2>&1
  end=$(date +%s%N)
  # Convert nanoseconds to milliseconds
  local ms=$(( (end - start) / 1000000 ))
  printf '%s' "${ms}"
}

mkdir -p "${output_dir}"
: >"${receipt}"
{
  echo "repo=$(basename "${ROOT}")"
  echo "started_unix=$(date +%s)"
  echo "host=$(hostname 2>/dev/null || echo unknown)"
} >>"${receipt}"

log "building cargo-allow debug binary"
cargo build -p cargo-allow --bins --locked 2>/dev/null

BIN="target/debug/cargo-allow"

# 1. Cold audit (full inventory scan)
log "measuring cold audit (full scan)"
ms=$(time_cmd "cold_audit" "${BIN}" audit --format json --output /dev/null)
echo "cold_audit_ms=${ms}" >>"${receipt}"
log "cold audit: ${ms}ms"

# 2. Warm check --mode no-new (repeat, cache warm)
log "measuring warm check --mode no-new"
ms=$(time_cmd "warm_check" "${BIN}" check --mode no-new --format markdown --output /dev/null)
echo "warm_check_ms=${ms}" >>"${receipt}"
log "warm check: ${ms}ms"

# 3. why on one finding (single-file fast path)
log "measuring why (single-file fast path)"
ms=$(time_cmd "why_fast_path" "${BIN}" why --kind non_rust_file --path scripts/release-install-smoke.sh --line 1 --format json --output /dev/null)
echo "why_ms=${ms}" >>"${receipt}"
log "why: ${ms}ms"

# 4. diff --base (PR posture) — use HEAD~1 as base if available
if git rev-parse --verify HEAD~1 >/dev/null 2>&1; then
  log "measuring diff --base HEAD~1"
  ms=$(time_cmd "diff_base" "${BIN}" diff --base HEAD~1 --format markdown --output /dev/null)
  echo "diff_ms=${ms}" >>"${receipt}"
  log "diff: ${ms}ms"
else
  log "skipping diff (no HEAD~1 available)"
  echo "diff_ms=skipped" >>"${receipt}"
fi

# 5. Warm audit (repeat for warm comparison)
log "measuring warm audit"
ms=$(time_cmd "warm_audit" "${BIN}" audit --format json --output /dev/null)
echo "warm_audit_ms=${ms}" >>"${receipt}"
log "warm audit: ${ms}ms"

{
  echo "completed_unix=$(date +%s)"
  echo "result=pass"
} >>"${receipt}"

log "performance budget receipt: ${receipt}"
