#!/usr/bin/env bash
# Operator-latency smoke for the supported cargo-allow command loop.
#
# The receipt is a performance observation tied to a real binary and verified
# command artifacts. It is intentionally a conservative catastrophic-regression
# gate, not a universal hardware-performance claim.
#
# Usage:
#   PROFILE=release scripts/perf-budget-smoke.sh
#
# Optional:
#   OUTPUT_DIR=<path>          receipt output directory
#   CARGO_ALLOW_BIN=<path>     use an already-built binary and skip the build
#   HARD_CEILING_MS=<integer>  per-command catastrophic ceiling (default: 60000)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

profile="${PROFILE:-debug}"
output_dir="${OUTPUT_DIR:-${ROOT}/target/perf-budget}"
artifact_dir="${output_dir}/artifacts"
receipt="${output_dir}/operator-latency.receipt.json"
metrics="${output_dir}/.operator-latency.samples.tsv"
hard_ceiling_ms="${HARD_CEILING_MS:-60000}"
failure_reason=""

mkdir -p "${artifact_dir}"
: >"${metrics}"
run_dir="$(mktemp -d "${TMPDIR:-/tmp}/cargo-allow-operator-latency.XXXXXX")"

log() {
  printf 'operator-latency: %s\n' "$*"
}

fail() {
  failure_reason="$1"
  printf 'operator-latency: %s\n' "${failure_reason}" >&2
  exit 1
}

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | cut -d ' ' -f 1
  else
    fail "no SHA-256 utility is available"
  fi
}

relative_path() {
  local path="$1"
  if [[ "${path}" == "${ROOT}/"* ]]; then
    printf '%s' "${path#"${ROOT}/"}"
  else
    printf '%s' "$(basename "${path}")"
  fi
}

write_receipt() {
  local result="$1" failure="$2"
  python3 - "${receipt}" "${metrics}" "${profile}" "${hard_ceiling_ms}" \
    "${result}" "${failure}" <<'PY'
import json
import os
import platform
import subprocess
import sys
from pathlib import Path

receipt_path, metrics_path, profile, ceiling, result, failure = sys.argv[1:]

argv_by_name = {
    "first_audit": ["cargo-allow", "audit", "--format", "json", "--output", "artifacts/first-audit.json"],
    "warm_check": ["cargo-allow", "check", "--mode", "no-new", "--format", "markdown", "--receipt", "artifacts/warm-check.receipt.json", "--output", "artifacts/warm-check.md"],
    "why_fast_path": ["cargo-allow", "why", "--kind", "non_rust_file", "--path", "scripts/release-install-smoke.sh", "--line", "1", "--format", "json", "--output", "artifacts/why-fast-path.json"],
    "worklist": ["cargo-allow", "worklist", "--format", "json", "--output", "artifacts/worklist.json"],
    "diff_base": ["cargo-allow", "diff", "--base", "HEAD~1", "--format", "markdown", "--output", "artifacts/diff-base.md"],
    "warm_audit": ["cargo-allow", "audit", "--format", "json", "--output", "artifacts/warm-audit.json"],
}

def version(command):
    try:
        return subprocess.check_output(command, text=True, stderr=subprocess.DEVNULL).strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"

def records():
    path = Path(metrics_path)
    if not path.is_file():
        return []
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        fields = line.split("\t")
        if len(fields) != 8 or fields[1] not in argv_by_name:
            continue
        phase, name, elapsed, artifact, digest, semantic, semantic_digest, status = fields
        rows.append({
            "name": name,
            "phase": phase,
            "argv": argv_by_name[name],
            "elapsed_ms": int(elapsed) if elapsed else None,
            "status": status,
            "artifact": {"path": artifact, "sha256": digest} if artifact else None,
            "semantic_artifact": {
                "path": semantic,
                "sha256": semantic_digest,
            } if semantic else None,
        })
    return rows

payload = {
    "schema_version": 1,
    "schema_id": "cargo-allow.operator-latency.v1",
    "tool": "cargo-allow",
    "command": "operator-latency",
    "result": result,
    "binary": {
        "path": os.environ.get("PERF_BINARY_REL", "unknown"),
        "sha256": os.environ.get("PERF_BINARY_SHA256", "unknown"),
        "profile": profile,
    },
    "host": {
        "os": platform.system(),
        "release": platform.release(),
        "machine": platform.machine(),
        "rustc": version(["rustc", "--version"]),
    },
    "repository": {
        "commit": version(["git", "rev-parse", "HEAD"]),
        "tracked_files": int(os.environ.get("PERF_TRACKED_FILES", "0")),
        "policy_entries": int(os.environ.get("PERF_POLICY_ENTRIES", "0")),
    },
    "sample_policy": {
        "cold_process_samples": 1,
        "warm_process_samples": 2,
        "targeted_samples": 3,
    },
    "budget": {
        "name": "operator_loop_hard_ceiling",
        "kind": "catastrophic_regression",
        "ceiling_ms": int(ceiling),
        "disposition": "passed" if result == "pass" else "failed",
    },
    "samples": records(),
    "claim_boundary": [
        "selected_repository_fixture",
        "end_to_end_wall_clock",
        "semantic_artifact_verified",
        "binary_and_profile_identified",
    ],
    "limitations": [
        "first_process_sample is not an operating-system-cold cache measurement",
        "advisory product targets are not blocking in this harness",
        "receipt does not establish latency on every repository or machine",
    ],
}
if failure:
    payload["failure"] = {"kind": "instrument_failure", "message": failure}
Path(receipt_path).write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
PY
}

finish() {
  local exit_code="$?"
  if [[ "${exit_code}" -ne 0 ]]; then
    write_receipt "failed" "${failure_reason:-instrument failure}"
  fi
  rm -rf "${run_dir}"
}
trap finish EXIT

[[ "${profile}" == "debug" || "${profile}" == "release" ]] || \
  fail "PROFILE must be debug or release"
[[ "${hard_ceiling_ms}" =~ ^[0-9]+$ ]] || \
  fail "HARD_CEILING_MS must be a non-negative integer"
command -v python3 >/dev/null 2>&1 || fail "python3 is required to write the JSON receipt"

binary="${CARGO_ALLOW_BIN:-}"
if [[ -z "${binary}" ]]; then
  log "building cargo-allow ${profile} binary"
  build_args=(-p cargo-allow --bin cargo-allow --locked)
  if [[ "${profile}" == "release" ]]; then
    build_args+=(--release)
  fi
  cargo build "${build_args[@]}" || fail "cargo build failed"
  binary="${ROOT}/target/${profile}/cargo-allow"
fi
if [[ ! -x "${binary}" && -x "${binary}.exe" ]]; then
  binary="${binary}.exe"
fi
[[ -x "${binary}" ]] || fail "cargo-allow binary is not executable: ${binary}"

export PERF_BINARY_REL="$(relative_path "${binary}")"
export PERF_BINARY_SHA256="$(sha256_file "${binary}")"
export PERF_TRACKED_FILES="$(git ls-files | wc -l | tr -d '[:space:]')"
export PERF_POLICY_ENTRIES="$(grep -c '^\[\[allow\]\]' policy/allow.toml || true)"

record_skipped() {
  local phase="$1" name="$2"
  printf '%s\t%s\t\t\t\t\t\tskipped\n' \
    "${phase}" "${name}" >>"${metrics}"
}

measure() {
  local phase="$1" name="$2" artifact_rel="$3" semantic_rel="$4" marker="$5"
  shift 5
  local artifact="${output_dir}/${artifact_rel}"
  local semantic="${output_dir}/${semantic_rel}"
  local stdout_path="${run_dir}/${name}.stdout"
  local stderr_path="${run_dir}/${name}.stderr"
  local start end elapsed digest semantic_digest

  rm -f "${artifact}" "${semantic}"
  mkdir -p "$(dirname "${artifact}")" "$(dirname "${semantic}")"
  start="$(date +%s%N)"
  if ! "${binary}" "$@" >"${stdout_path}" 2>"${stderr_path}"; then
    cat "${stdout_path}" >&2
    cat "${stderr_path}" >&2
    fail "${name} command failed"
  fi
  end="$(date +%s%N)"
  elapsed=$(( (end - start) / 1000000 ))
  [[ -s "${artifact}" ]] || fail "${name} did not produce ${artifact_rel}"
  [[ -s "${semantic}" ]] || fail "${name} did not produce ${semantic_rel}"
  grep -Fq "${marker}" "${semantic}" || \
    fail "${name} semantic result did not contain expected marker: ${marker}"
  if (( elapsed > hard_ceiling_ms )); then
    fail "${name} exceeded the ${hard_ceiling_ms}ms catastrophic ceiling (${elapsed}ms)"
  fi
  digest="$(sha256_file "${artifact}")"
  semantic_digest="$(sha256_file "${semantic}")"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\tpassed\n' \
    "${phase}" "${name}" "${elapsed}" "${artifact_rel}" "${digest}" \
    "${semantic_rel}" "${semantic_digest}" >>"${metrics}"
  log "${name}: ${elapsed}ms"
}

log "measuring first process audit"
measure "cold" "first_audit" \
  "artifacts/first-audit.json" "artifacts/first-audit.json" \
  '"status": "passed"' \
  audit --format json --output "${artifact_dir}/first-audit.json"

log "measuring warm no-new check"
measure "warm" "warm_check" \
  "artifacts/warm-check.md" "artifacts/warm-check.receipt.json" \
  '"failed": false' \
  check --mode no-new --format markdown \
  --receipt "${artifact_dir}/warm-check.receipt.json" \
  --output "${artifact_dir}/warm-check.md"

log "measuring targeted why"
measure "targeted" "why_fast_path" \
  "artifacts/why-fast-path.json" "artifacts/why-fast-path.json" \
  '"status": "matched"' \
  why --kind non_rust_file --path scripts/release-install-smoke.sh --line 1 \
  --format json --output "${artifact_dir}/why-fast-path.json"

log "measuring worklist"
measure "targeted" "worklist" \
  "artifacts/worklist.json" "artifacts/worklist.json" \
  '"schema_id": "cargo-allow.worklist.v1"' \
  worklist --format json --output "${artifact_dir}/worklist.json"

if git rev-parse --verify HEAD~1 >/dev/null 2>&1; then
  log "measuring diff against HEAD~1"
  measure "targeted" "diff_base" \
    "artifacts/diff-base.md" "artifacts/diff-base.md" \
    '**Result:** passed' \
    diff --base HEAD~1 --format markdown --output "${artifact_dir}/diff-base.md"
else
  log "skipping diff: HEAD~1 is unavailable"
  record_skipped "targeted" "diff_base"
fi

log "measuring warm audit"
measure "warm" "warm_audit" \
  "artifacts/warm-audit.json" "artifacts/warm-audit.json" \
  '"status": "passed"' \
  audit --format json --output "${artifact_dir}/warm-audit.json"

write_receipt "pass" ""

log "operator latency receipt: ${receipt}"
