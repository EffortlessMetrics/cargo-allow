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
output_dir="${output_dir%/}"
artifact_dir="${output_dir}/artifacts"
receipt="${output_dir}/operator-latency.receipt.json"
metrics="${output_dir}/.operator-latency.samples.tsv"
hard_ceiling_ms="${HARD_CEILING_MS:-60000}"
failure_reason=""

mkdir -p "${ROOT}/target" "${artifact_dir}"
: >"${metrics}"
run_dir="$(mktemp -d "${ROOT}/target/cargo-allow-operator-latency.XXXXXX")"

log() {
  printf 'operator-latency: %s\n' "$*"
}

fail() {
  failure_reason="$1"
  printf 'operator-latency: %s\n' "${failure_reason}" >&2
  exit 1
}

# Same Python fallback pattern as scripts/audit-default-sensor-burden.sh:
# prefer python3, accept any Python >= 3 interpreter named python.
py="${PYTHON3:-python3}"
if ! command -v "${py}" >/dev/null 2>&1; then
  if command -v python >/dev/null 2>&1 && python -c 'import sys; sys.exit(0 if sys.version_info[0] >= 3 else 1)' >/dev/null 2>&1; then
    py=python
  else
    printf 'operator-latency: a Python 3 interpreter is required to write the JSON receipt\n' >&2
    exit 1
  fi
fi

# Windows-native Python cannot open POSIX-form paths handed over by this
# shell; convert to the mixed form when the bridge exists (CI Linux is a
# no-op). Same approach as scripts/audit-default-sensor-burden.sh.
py_path() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -m "$1"
  else
    printf '%s' "$1"
  fi
}

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | cut -d ' ' -f 1
  else
    printf 'operator-latency: no SHA-256 utility is available\n' >&2
    return 1
  fi
}

now_ms() {
  local value
  if value="$(date +%s%N 2>/dev/null)" && [[ "${value}" =~ ^[0-9]+$ ]]; then
    printf '%s' "$(( value / 1000000 ))"
  else
    "${py}" -c 'import time; print(time.time_ns() // 1_000_000)'
  fi
}

encode_argv() {
  "${py}" - "$@" <<'PY'
import json
import sys

print(json.dumps(sys.argv[1:], separators=(",", ":")))
PY
}

normalize_json() {
  local source="$1" destination="$2"
  "${py}" - "$(py_path "${source}")" "$(py_path "${destination}")" <<'PY'
import json
import sys
from pathlib import Path

source, destination = sys.argv[1:]
value = json.loads(Path(source).read_text(encoding="utf-8"))
if isinstance(value, dict):
    value.pop("run_id", None)
    value.pop("started_at", None)
Path(destination).write_text(json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n", encoding="utf-8")
PY
}

compare_semantic_json() {
  local left="$1" right="$2" label="$3"
  cmp -s "${left}" "${right}" || fail "${label} semantic JSON differs"
}

relative_path() {
  local path="$1"
  if [[ "${path}" == "${ROOT}/"* ]]; then
    printf '%s' "${path#"${ROOT}/"}"
  else
    printf '%s' "$(basename "${path}")"
  fi
}

native_path() {
  if [[ "${binary,,}" == *.exe ]] && command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$1"
  elif [[ "${binary,,}" == *.exe ]] && command -v wslpath >/dev/null 2>&1; then
    wslpath -w "$1" | tr -d '\r'
  else
    printf '%s' "$1"
  fi
}

# Catastrophic payload ceilings (bytes) for emitted machine artifacts, keyed
# by sample name as "<artifact-ceiling> <semantic-ceiling>"; '-' disables the
# assertion for that file. These mirror the payload table in
# docs/performance-budgets.md and are catastrophic-regression assertions,
# not the <= 65,536 B advisory agent-read target.
payload_ceilings() {
  case "$1" in
    first_audit | warm_audit) printf '%s\n' '8388608 8388608' ;;
    worklist | agent_loop_worklist) printf '%s\n' '524288 524288' ;;
    worklist_summary) printf '%s\n' '4096 4096' ;;
    check_summary) printf '%s\n' '4096 16384' ;;
    warm_check) printf '%s\n' '- 16384' ;;
    agent_loop_check) printf '%s\n' '16384 16384' ;;
    why_fast_path) printf '%s\n' '8192 8192' ;;
    agent_loop_why_plan) printf '%s\n' '8192 12288' ;;
    agent_loop_add) printf '%s\n' '4096 4096' ;;
    *) printf '%s\n' '- -' ;;
  esac
}

check_payload_ceiling() {
  local name="$1" file="$2" ceiling="$3" bytes
  bytes="$(wc -c <"${file}" | tr -d '[:space:]')"
  (( bytes <= ceiling )) || \
    fail "${name} payload $(basename "${file}") at ${bytes}B exceeded the ${ceiling}B catastrophic payload ceiling"
}

assert_payload_ceilings() {
  local name="$1" artifact="$2" semantic="$3"
  local ceilings
  ceilings="$(payload_ceilings "${name}")"
  if [[ "${ceilings%% *}" != '-' ]]; then
    check_payload_ceiling "${name}" "${artifact}" "${ceilings%% *}"
  fi
  if [[ "${ceilings##* }" != '-' ]]; then
    check_payload_ceiling "${name}" "${semantic}" "${ceilings##* }"
  fi
}

artifact_payload_bytes() {
  wc -c <"$1" | tr -d '[:space:]'
}

# Read one field of the most recent metrics row for a sample name.
sample_field() {
  awk -F '\t' -v name="$1" -v field="$2" '$2 == name { value = $field } END { print value }' "${metrics}"
}

write_receipt() {
  local result="$1" failure="$2"
  "${py}" - "$(py_path "${receipt}")" "$(py_path "${metrics}")" "${profile}" "${hard_ceiling_ms}" \
    "${result}" "${failure}" <<'PY'
import json
import os
import platform
import subprocess
import sys
from pathlib import Path

receipt_path, metrics_path, profile, ceiling, result, failure = sys.argv[1:]

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
        if len(fields) != 10:
            continue
        phase, name, elapsed, artifact, digest, semantic, semantic_digest, status, argv_json, payload_bytes = fields
        try:
            argv = json.loads(argv_json)
        except json.JSONDecodeError:
            continue
        if not isinstance(argv, list) or not all(isinstance(arg, str) for arg in argv):
            continue
        cache_mode = "not_applicable"
        if "--persistent-cache" in argv:
            try:
                cache_mode = argv[argv.index("--persistent-cache") + 1]
            except IndexError:
                cache_mode = "invalid"
        rows.append({
            "name": name,
            "phase": phase,
            "cache_mode": cache_mode,
            "argv": argv,
            "elapsed_ms": int(elapsed) if elapsed else None,
            "status": status,
            "payload_bytes": int(payload_bytes) if payload_bytes else None,
            "artifact": {"path": artifact, "sha256": digest} if artifact else None,
            "semantic_artifact": {
                "path": semantic,
                "sha256": semantic_digest,
            } if semantic else None,
        })
    return rows

sample_rows = records()
payload = {
    "schema_version": 3,
    "schema_id": "cargo-allow.operator-latency.v3",
    "tool": "cargo-allow",
    "command": "operator-latency",
    "result": result,
    "binary": {
        "path": os.environ.get("PERF_BINARY_REL", "unknown"),
        "sha256": os.environ.get("PERF_BINARY_SHA256"),
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
        "cold_process_samples": sum(row["phase"] == "cold" for row in sample_rows),
        "warm_process_samples": sum(row["phase"] == "warm" for row in sample_rows),
        "targeted_samples": sum(row["phase"] == "targeted" for row in sample_rows),
        "agent_loop_samples": sum(row["phase"] == "agent_loop" for row in sample_rows),
        "cache_mode_samples": {
            mode: sum(row["cache_mode"] == mode for row in sample_rows)
            for mode in ("on", "off", "not_applicable")
        },
    },
    "budget": {
        "name": "operator_loop_hard_ceiling",
        "kind": "catastrophic_regression",
        "ceiling_ms": int(ceiling),
        "disposition": "passed" if result == "pass" else "failed",
    },
    "samples": sample_rows,
    "claim_boundary": [
        "selected_repository_fixture",
        "end_to_end_wall_clock",
        "semantic_artifact_verified",
        "persistent_cache_phase_compared",
        "binary_and_profile_identified",
        "payload_bytes_recorded_per_sample",
        "agent_loop_composite_steps_attributable",
    ],
    "limitations": [
        "first_process_sample is not an operating-system-cold cache measurement",
        "advisory product targets are not blocking in this harness",
        "receipt does not establish latency on every repository or machine",
        "off phase verifies no persistent-store creation, not absence of filesystem reads",
        "payload ceilings are catastrophic artifact-size assertions, not agent-read advisory targets",
        "hooks_overhead_ms is a paired same-host difference and carries host noise",
    ],
}
agent_loop_path = os.environ.get("PERF_AGENT_LOOP_SUMMARY", "")
if agent_loop_path and Path(agent_loop_path).is_file():
    payload["agent_loop"] = json.loads(Path(agent_loop_path).read_text(encoding="utf-8"))
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
[[ -n "${py}" ]] && command -v "${py}" >/dev/null 2>&1 || \
  fail "a Python 3 interpreter is required to write the JSON receipt"
command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1 || \
  fail "no SHA-256 utility is available"

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
# Prefer the literal Windows executable name: MSYS resolves "cargo-allow"
# transparently for exec/stat, but Windows-native consumers of the path
# (hooks run --binary) need the exact on-disk file.
if [[ -e "${binary}.exe" ]]; then
  binary="${binary}.exe"
fi
[[ -e "${binary}" ]] || fail "cargo-allow binary is not executable: ${binary}"

PERF_BINARY_REL="$(relative_path "${binary}")"
PERF_BINARY_SHA256="$(sha256_file "${binary}")"
PERF_TRACKED_FILES="$(git ls-files | wc -l | tr -d '[:space:]')"
PERF_POLICY_ENTRIES="$(grep -c '^\[\[allow\]\]' policy/allow.toml 2>/dev/null || printf '0')"
export PERF_BINARY_REL PERF_BINARY_SHA256 PERF_TRACKED_FILES PERF_POLICY_ENTRIES

record_skipped() {
  local phase="$1" name="$2"
  shift 2
  local argv_json
  argv_json="$(encode_argv "$@")"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "${phase}" "${name}" "" "" "" "" "" "skipped" "${argv_json}" "" >>"${metrics}"
}

measure() {
  local phase="$1" name="$2" artifact_rel="$3" semantic_rel="$4" marker="$5"
  shift 5
  # POSTURE_OK=1 (via measure_posture) accepts exit 1 with a rendered
  # report: a diff against the parent may legitimately fail on
  # posture (this PR's own policy changes); the latency sample is
  # still valid. Infrastructure failures (other exits, missing
  # output) still fail.
  local posture_ok="${POSTURE_OK:-0}"
  local artifact="${output_dir}/${artifact_rel}"
  local semantic="${output_dir}/${semantic_rel}"
  local stdout_path="${run_dir}/${name}.stdout"
  local stderr_path="${run_dir}/${name}.stderr"
  local start end elapsed digest semantic_digest payload_bytes argv_json

  rm -f "${artifact}" "${semantic}"
  mkdir -p "$(dirname "${artifact}")" "$(dirname "${semantic}")"
  argv_json="$(encode_argv "$@")"
  start="$(now_ms)"
  local rc=0
  "${binary}" "$@" >"${stdout_path}" 2>"${stderr_path}" || rc=$?
  if (( rc != 0 )) && { (( rc != 1 )) || [[ "${posture_ok}" != "1" ]]; }; then
    cat "${stdout_path}" >&2
    cat "${stderr_path}" >&2
    fail "${name} command failed (exit ${rc})"
  fi
  end="$(now_ms)"
  elapsed=$(( end - start ))
  [[ -s "${artifact}" ]] || fail "${name} did not produce ${artifact_rel}"
  [[ -s "${semantic}" ]] || fail "${name} did not produce ${semantic_rel}"
  if (( rc == 0 )); then
    grep -Fq "${marker}" "${semantic}" || \
      fail "${name} semantic result did not contain expected marker: ${marker}"
  else
    grep -Fq "${marker%%passed*}" "${semantic}" || \
      fail "${name} posture-failed output still needs a rendered result marker"
  fi
  if (( elapsed > hard_ceiling_ms )); then
    fail "${name} exceeded the ${hard_ceiling_ms}ms catastrophic ceiling (${elapsed}ms)"
  fi
  digest="$(sha256_file "${artifact}")"
  semantic_digest="$(sha256_file "${semantic}")"
  assert_payload_ceilings "${name}" "${artifact}" "${semantic}"
  payload_bytes="$(artifact_payload_bytes "${artifact}")"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "${phase}" "${name}" "${elapsed}" "${artifact_rel}" "${digest}" \
    "${semantic_rel}" "${semantic_digest}" "passed" "${argv_json}" \
    "${payload_bytes}" >>"${metrics}"
  log "${name}: ${elapsed}ms"
}

measure_posture() {
  POSTURE_OK=1 measure "$@"
}

# One agent-loop composite step (#4366): same measurement contract as
# measure(), executed inside the writable full-scale fixture clone so the
# routed work item, plan artifact, applied entry, and green gate all bind to
# the clone's own ledger. Command artifacts are written under the clone's
# .measurement/ directory (check/summary outputs must stay inside the scan
# root) and copied out to the receipt artifact directory afterwards.
measure_agent_step() {
  local name="$1" artifact_rel="$2" semantic_rel="$3" marker="$4"
  shift 4
  local artifact="${output_dir}/${artifact_rel}"
  local semantic="${output_dir}/${semantic_rel}"
  local in_artifact="${agent_loop_root}/.measurement/$(basename "${artifact_rel}")"
  local in_semantic="${agent_loop_root}/.measurement/$(basename "${semantic_rel}")"
  local stdout_path="${run_dir}/${name}.stdout"
  local stderr_path="${run_dir}/${name}.stderr"
  local start end elapsed digest semantic_digest payload_bytes argv_json

  rm -f "${in_artifact}" "${in_semantic}" "${artifact}" "${semantic}"
  mkdir -p "${agent_loop_root}/.measurement" \
    "$(dirname "${artifact}")" "$(dirname "${semantic}")"
  argv_json="$(encode_argv "$@")"
  start="$(now_ms)"
  local rc=0
  (
    cd "${agent_loop_root}" &&
      env -u GIT_DIR -u GIT_WORK_TREE "${binary}" "$@"
  ) >"${stdout_path}" 2>"${stderr_path}" || rc=$?
  if (( rc != 0 )); then
    cat "${stdout_path}" >&2
    cat "${stderr_path}" >&2
    fail "${name} agent-loop step failed (exit ${rc})"
  fi
  end="$(now_ms)"
  elapsed=$(( end - start ))
  [[ -s "${in_artifact}" ]] || fail "${name} did not produce $(basename "${artifact_rel}")"
  [[ -s "${in_semantic}" ]] || fail "${name} did not produce $(basename "${semantic_rel}")"
  grep -Fq "${marker}" "${in_semantic}" || \
    fail "${name} semantic result did not contain expected marker: ${marker}"
  if (( elapsed > hard_ceiling_ms )); then
    fail "${name} exceeded the ${hard_ceiling_ms}ms catastrophic ceiling (${elapsed}ms)"
  fi
  cp "${in_artifact}" "${artifact}"
  cp "${in_semantic}" "${semantic}"
  digest="$(sha256_file "${artifact}")"
  semantic_digest="$(sha256_file "${semantic}")"
  assert_payload_ceilings "${name}" "${artifact}" "${semantic}"
  payload_bytes="$(artifact_payload_bytes "${artifact}")"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "agent_loop" "${name}" "${elapsed}" "${artifact_rel}" "${digest}" \
    "${semantic_rel}" "${semantic_digest}" "passed" "${argv_json}" \
    "${payload_bytes}" >>"${metrics}"
  log "${name}: ${elapsed}ms"
}

# Paired hooks-run sample (#4366): the wrapped child is the exact closed
# `check --mode no-new` command, so the sample's machine artifact is the
# forwarded child stdout captured in the receipt artifact directory.
measure_hooks_sample() {
  local name="$1" artifact_rel="$2" marker="$3"
  shift 3
  local artifact="${output_dir}/${artifact_rel}"
  local stderr_path="${run_dir}/${name}.stderr"
  local start end elapsed digest payload_bytes argv_json

  rm -f "${artifact}"
  mkdir -p "$(dirname "${artifact}")"
  argv_json="$(encode_argv "$@")"
  start="$(now_ms)"
  local rc=0
  (
    cd "${agent_loop_root}" &&
      env -u GIT_DIR -u GIT_WORK_TREE "${binary}" "$@"
  ) >"${artifact}" 2>"${stderr_path}" || rc=$?
  if (( rc != 0 )); then
    cat "${artifact}" >&2
    cat "${stderr_path}" >&2
    fail "${name} hooks sample failed (exit ${rc})"
  fi
  end="$(now_ms)"
  elapsed=$(( end - start ))
  [[ -s "${artifact}" ]] || fail "${name} did not produce ${artifact_rel}"
  grep -Fq "${marker}" "${artifact}" || \
    fail "${name} semantic result did not contain expected marker: ${marker}"
  if (( elapsed > hard_ceiling_ms )); then
    fail "${name} exceeded the ${hard_ceiling_ms}ms catastrophic ceiling (${elapsed}ms)"
  fi
  digest="$(sha256_file "${artifact}")"
  payload_bytes="$(artifact_payload_bytes "${artifact}")"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "agent_loop" "${name}" "${elapsed}" "${artifact_rel}" "${digest}" \
    "${artifact_rel}" "${digest}" "passed" "${argv_json}" \
    "${payload_bytes}" >>"${metrics}"
  log "${name}: ${elapsed}ms"
}

fixture_root="${run_dir}/cache-fixture"
mkdir -p "${fixture_root}/src" "${fixture_root}/policy"
printf '%s\n' 'fn measured(value: Option<u8>) -> u8 { value.unwrap() }' >"${fixture_root}/src/lib.rs"
cat >"${fixture_root}/policy/allow.toml" <<'EOF'
schema_version = 1

[workspace]
ignored = []
generated = []
EOF
env -u GIT_DIR -u GIT_WORK_TREE git -C "${fixture_root}" init -q || fail "cache fixture git init failed"
env -u GIT_DIR -u GIT_WORK_TREE git -C "${fixture_root}" config user.email cargo-allow@example.invalid || fail "cache fixture git email failed"
env -u GIT_DIR -u GIT_WORK_TREE git -C "${fixture_root}" config user.name cargo-allow || fail "cache fixture git name failed"
env -u GIT_DIR -u GIT_WORK_TREE git -C "${fixture_root}" add --all || fail "cache fixture git add failed"
env -u GIT_DIR -u GIT_WORK_TREE git -C "${fixture_root}" commit -qm measurement || fail "cache fixture git commit failed"

measure_cache_phase() {
  local phase="$1" name="$2" mode="$3"
  local report_rel="artifacts/${name}.report.json"
  local receipt_rel="artifacts/${name}.receipt.json"
  local semantic_report_rel="artifacts/${name}.semantic-report.json"
  local semantic_receipt_rel="artifacts/${name}.semantic-receipt.json"
  local report="${output_dir}/${report_rel}"
  local receipt="${output_dir}/${receipt_rel}"
  local semantic_report="${output_dir}/${semantic_report_rel}"
  local semantic_receipt="${output_dir}/${semantic_receipt_rel}"
  local stdout_path="${run_dir}/${name}.stdout"
  local stderr_path="${run_dir}/${name}.stderr"
  local fixture_artifact_dir="${fixture_root}/.measurement"
  local fixture_report="${fixture_artifact_dir}/${name}.report.json"
  local fixture_receipt="${fixture_artifact_dir}/${name}.receipt.json"
  local fixture_root_arg="$(native_path "${fixture_root}")"
  local fixture_report_arg="$(native_path "${fixture_report}")"
  local fixture_receipt_arg="$(native_path "${fixture_receipt}")"
  local start end elapsed rc=0 digest semantic_digest payload_bytes argv_json
  local -a argv=(check --root "${fixture_root_arg}" --config policy/allow.toml
    --persistent-cache "${mode}" --format json --receipt "${fixture_receipt_arg}"
    --output "${fixture_report_arg}")
  mkdir -p "${fixture_artifact_dir}"
  argv_json="$(encode_argv "${argv[@]}")"
  start="$(now_ms)"
  "${binary}" "${argv[@]}" >"${stdout_path}" 2>"${stderr_path}" || rc=$?
  if (( rc != 1 )); then
    cat "${stdout_path}" >&2
    cat "${stderr_path}" >&2
    fail "${name} cache phase failed (exit ${rc})"
  fi
  end="$(now_ms)"
  elapsed=$(( end - start ))
  [[ -s "${fixture_report}" && -s "${fixture_receipt}" ]] || fail "${name} cache phase omitted report/receipt"
  cp "${fixture_report}" "${report}"
  cp "${fixture_receipt}" "${receipt}"
  normalize_json "${report}" "${semantic_report}"
  normalize_json "${receipt}" "${semantic_receipt}"
  grep -Fq 'src/lib.rs' "${semantic_report}" || fail "${name} omitted the measured source path"
  grep -Fq '"family":"unwrap"' "${semantic_report}" || fail "${name} omitted the measured unwrap finding"
  (( elapsed <= hard_ceiling_ms )) || fail "${name} exceeded the ${hard_ceiling_ms}ms catastrophic ceiling"
  digest="$(sha256_file "${report}")"
  semantic_digest="$(sha256_file "${semantic_report}")"
  payload_bytes="$(artifact_payload_bytes "${report}")"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "${phase}" "${name}" "${elapsed}" "${report_rel}" "${digest}" \
    "${semantic_report_rel}" "${semantic_digest}" "passed" "${argv_json}" \
    "${payload_bytes}" >>"${metrics}"
}

cache_dir="${fixture_root}/target/cargo-allow/cache"
[[ ! -e "${cache_dir}" ]] || fail "cache fixture unexpectedly had a preexisting store"
measure_cache_phase "cold" "cache_cold_on" "on"
[[ -s "${cache_dir}/scan-cache.v2.bin" ]] || fail "cold-on did not create the persistent store"
measure_cache_phase "warm" "cache_warm_on" "on"
[[ -s "${cache_dir}/scan-cache.v2.bin" ]] || fail "warm-on lost the persistent store"
rm -rf "${cache_dir}"
measure_cache_phase "targeted" "cache_disabled_off" "off"
[[ ! -e "${cache_dir}" ]] || fail "off mode created a persistent store"
compare_semantic_json "${artifact_dir}/cache_cold_on.semantic-report.json" \
  "${artifact_dir}/cache_warm_on.semantic-report.json" "cold/warm report"
compare_semantic_json "${artifact_dir}/cache_cold_on.semantic-report.json" \
  "${artifact_dir}/cache_disabled_off.semantic-report.json" "on/off report"
compare_semantic_json "${artifact_dir}/cache_cold_on.semantic-receipt.json" \
  "${artifact_dir}/cache_warm_on.semantic-receipt.json" "cold/warm receipt"
compare_semantic_json "${artifact_dir}/cache_cold_on.semantic-receipt.json" \
  "${artifact_dir}/cache_disabled_off.semantic-receipt.json" "on/off receipt"

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
  measure_posture "targeted" "diff_base" \
    "artifacts/diff-base.md" "artifacts/diff-base.md" \
    '**Result:** passed' \
    diff --base HEAD~1 --format markdown --output "${artifact_dir}/diff-base.md"
else
  log "skipping diff: HEAD~1 is unavailable"
  record_skipped "targeted" "diff_base" \
    diff --base HEAD~1 --format markdown --output "${artifact_dir}/diff-base.md"
fi

log "measuring warm audit"
measure "warm" "warm_audit" \
  "artifacts/warm-audit.json" "artifacts/warm-audit.json" \
  '"status": "passed"' \
  audit --format json --output "${artifact_dir}/warm-audit.json"

log "measuring worklist command summary projection"
measure "targeted" "worklist_summary" \
  "artifacts/worklist-summary.json" "artifacts/worklist-summary.json" \
  '"schema_id": "cargo-allow.core-command-summary.v1"' \
  worklist --command-summary-output "${artifact_dir}/worklist-summary.json"

log "measuring check command summary projection"
measure "targeted" "check_summary" \
  "artifacts/check-summary.json" "artifacts/check-summary.receipt.json" \
  '"failed": false' \
  check --mode no-new --format markdown \
  --receipt "${artifact_dir}/check-summary.receipt.json" \
  --output "${artifact_dir}/check-summary.md" \
  --command-summary-output "${artifact_dir}/check-summary.json"
grep -Fq '"schema_id": "cargo-allow.core-command-summary.v1"' \
  "${artifact_dir}/check-summary.json" || \
  fail "check_summary did not produce the command summary schema"

# --- Agentic-surface suite (#4366) -----------------------------------------
#
# The composite fixture mirrors the measure_cache_phase fixture pattern at
# full repo scale: one writable `git clone --shared` copy of this repository
# inside run_dir, with a fresh unreceipted unwrap probe committed on top.
# The clone is torn down with run_dir on exit, so every harness run measures
# the 4-command repair sequence against a fixture reset to the same probe
# state, and the real repository tree is never modified.

agent_loop_root="${run_dir}/agent-loop-fixture"
log "preparing agent-loop fixture clone"
env -u GIT_DIR -u GIT_WORK_TREE git clone --shared --quiet "${ROOT}" "${agent_loop_root}" ||
  fail "agent-loop fixture clone failed"
printf '%s\n' \
  'pub fn agent_loop_probe(value: Option<u8>) -> u8 {' \
  '    value.unwrap()' \
  '}' >"${agent_loop_root}/crates/cargo-allow/src/agent_loop_probe.rs"
env -u GIT_DIR -u GIT_WORK_TREE git -C "${agent_loop_root}" \
  add crates/cargo-allow/src/agent_loop_probe.rs ||
  fail "agent-loop probe git add failed"
env -u GIT_DIR -u GIT_WORK_TREE git -C "${agent_loop_root}" \
  -c user.email=cargo-allow@example.invalid -c user.name=cargo-allow-perf \
  commit -qm agent-loop-probe || fail "agent-loop probe git commit failed"

log "measuring agent-loop composite (worklist -> why --plan -> add --from-plan --update -> check)"
composite_start="$(now_ms)"
measure_agent_step "agent_loop_worklist" \
  "artifacts/agent-loop-worklist.json" "artifacts/agent-loop-worklist.json" \
  'new_unreceipted_finding' \
  worklist --format json --output .measurement/agent-loop-worklist.json
grep -Fq 'agent_loop_probe.rs' "${artifact_dir}/agent-loop-worklist.json" || \
  fail "agent_loop_worklist did not route the probe finding"
measure_agent_step "agent_loop_why_plan" \
  "artifacts/agent-loop-why.json" "artifacts/agent-loop-plan.json" \
  'cargo-allow.add-finding-plan.v1' \
  why --kind panic --path crates/cargo-allow/src/agent_loop_probe.rs --line 2 \
  --format json --output .measurement/agent-loop-why.json \
  --plan .measurement/agent-loop-plan.json
grep -Fq 'agent_loop_probe.rs' "${artifact_dir}/agent-loop-plan.json" || \
  fail "agent_loop_why_plan did not bind the probe finding"
measure_agent_step "agent_loop_add" \
  "artifacts/agent-loop-add-summary.json" "artifacts/agent-loop-add-summary.json" \
  'add-plan-application' \
  add --from-plan .measurement/agent-loop-plan.json --update \
  --owner core --reason "agent-loop composite measurement probe" \
  --evidence issue:4366 --summary-format json \
  --summary-output .measurement/agent-loop-add-summary.json
grep -Fq 'added_allow_id' "${artifact_dir}/agent-loop-add-summary.json" || \
  fail "agent_loop_add summary omitted the applied entry id"
measure_agent_step "agent_loop_check" \
  "artifacts/agent-loop-check.receipt.json" "artifacts/agent-loop-check.receipt.json" \
  '"failed": false' \
  check --mode no-new --format markdown \
  --receipt .measurement/agent-loop-check.receipt.json \
  --output .measurement/agent-loop-check.md
composite_total=$(( $(now_ms) - composite_start ))
log "agent-loop composite total: ${composite_total}ms"

log "measuring paired hooks run wrapper overhead"
hooks_binary="$(native_path "${binary}")"
hooks_digest="$("${binary}" tool identity --format json |
  "${py}" -c 'import json, sys; print(json.load(sys.stdin)["executable_digest"])')"
[[ "${hooks_digest}" == sha256:v1:* ]] || \
  fail "tool identity did not return an executable digest for hooks run"
measure_hooks_sample "hooks_wrapped_check" "artifacts/hooks-wrapped-check.md" \
  'Result: passed' \
  hooks run --binary "${hooks_binary}" --digest "${hooks_digest}" \
  --mode explicit-tool-under-test -- check --mode no-new
measure_hooks_sample "hooks_bare_check" "artifacts/hooks-bare-check.md" \
  'Result: passed' \
  check --mode no-new

log "recording agent-loop attribution rows"
: >"${run_dir}/agent-loop.steps.tsv"
for step in agent_loop_worklist agent_loop_why_plan agent_loop_add agent_loop_check; do
  printf '%s\t%s\t%s\n' \
    "${step}" "$(sample_field "${step}" 3)" "$(sample_field "${step}" 10)" \
    >>"${run_dir}/agent-loop.steps.tsv"
done
added_allow_id="$("${py}" -c 'import json, sys; print(json.load(sys.stdin).get("added_allow_id") or "")' \
  <"$(py_path "${artifact_dir}/agent-loop-add-summary.json")")"
"${py}" - \
  "$(py_path "${output_dir}/.agent-loop.summary.json")" \
  "${added_allow_id}" "${composite_total}" \
  "$(sample_field hooks_wrapped_check 3)" "$(sample_field hooks_bare_check 3)" \
  "$(py_path "${run_dir}/agent-loop.steps.tsv")" <<'PY'
import json
import sys
from pathlib import Path

out_path, added_allow_id, total, wrapped, bare, steps_path = sys.argv[1:7]
steps = []
for line in Path(steps_path).read_text(encoding="utf-8").splitlines():
    name, elapsed, payload = line.split("\t")
    steps.append({
        "sample": name,
        "elapsed_ms": int(elapsed),
        "payload_bytes": int(payload) if payload else None,
    })
wrapped_ms = int(wrapped)
bare_ms = int(bare)
summary = {
    "composite": {
        "name": "agent_loop_composite",
        "added_allow_id": added_allow_id or None,
        "steps": steps,
        "total_elapsed_ms": int(total),
    },
    "hooks_overhead": {
        "wrapped_sample": "hooks_wrapped_check",
        "bare_sample": "hooks_bare_check",
        "wrapped_elapsed_ms": wrapped_ms,
        "bare_elapsed_ms": bare_ms,
        "overhead_ms": wrapped_ms - bare_ms,
    },
}
Path(out_path).write_text(json.dumps(summary, sort_keys=True, separators=(",", ":")) + "\n", encoding="utf-8")
PY
PERF_AGENT_LOOP_SUMMARY="$(py_path "${output_dir}/.agent-loop.summary.json")"
export PERF_AGENT_LOOP_SUMMARY

write_receipt "pass" ""

log "operator latency receipt: ${receipt}"
