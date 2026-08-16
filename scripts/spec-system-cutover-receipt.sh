#!/usr/bin/env bash
# SpecSystemCutoverReceiptV1 (#2568).
#
# Proves this repository enabled delegate_spec_system cutover and retired the
# embedded spec-system CI audit lane. Does not invoke cargo-intent audit surfaces
# that do not exist yet.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

work_dir="${WORK_DIR:-${ROOT}/target/spec-system-cutover}"
receipt="${work_dir}/spec-system-cutover.receipt.json"
schema_id="cargo-allow.spec-system-cutover-receipt.v1"
config_path="${ROOT}/.allow/compatibility/intent-delegation.toml"
cutover_fixture="${ROOT}/tests/compat/fixtures/intent-spec-system-cutover-v1.toml"
stage_receipt="${ROOT}/tests/compat/fixtures/cargo-allow-spec-system-cutover-receipt-v1.toml"

log() {
  printf 'spec-system-cutover: %s\n' "$*"
}

fail() {
  printf 'spec-system-cutover: error: %s\n' "$*" >&2
  exit 1
}

[[ -f "${config_path}" ]] || fail "missing delegation config ${config_path}"
[[ -f "${cutover_fixture}" ]] || fail "missing cutover fixture ${cutover_fixture}"
[[ -f "${stage_receipt}" ]] || fail "missing stage receipt ${stage_receipt}"
command -v python3 >/dev/null 2>&1 || fail "python3 is required"

mkdir -p "${work_dir}"

python3 - "${config_path}" "${cutover_fixture}" "${stage_receipt}" "${receipt}" "${schema_id}" "${ROOT}" <<'PY'
import json
import re
import sys
from pathlib import Path

config_path = Path(sys.argv[1])
cutover_fixture = Path(sys.argv[2])
stage_receipt = Path(sys.argv[3])
receipt_path = Path(sys.argv[4])
schema_id = sys.argv[5]
root = Path(sys.argv[6])

config = config_path.read_text(encoding="utf-8")
for flag in ["delegate_spec_system = true", "delegate_staged_precommit = true"]:
    if flag not in config:
        raise SystemExit(f"delegation config missing {flag}")
if 'schema_id = "cargo-allow.intent-delegation.v1"' not in config:
    raise SystemExit("delegation config missing schema_id")

fixture = cutover_fixture.read_text(encoding="utf-8")
for needle in [
    "delegate_spec_system",
    "forbidden_when_cutover_enabled",
    "cargo-intent",
]:
    if needle not in fixture:
        raise SystemExit(f"cutover fixture missing {needle}")

stage = stage_receipt.read_text(encoding="utf-8")
if "#2568" not in stage:
    raise SystemExit("stage receipt missing #2568 marker")

ci = (root / ".github/workflows/ci.yml").read_text(encoding="utf-8")
if "check --profile spec-system --mode audit" in ci:
    raise SystemExit("ci.yml still runs embedded spec-system audit")
if "spec-system-cutover-receipt.sh" not in ci:
    raise SystemExit("ci.yml missing spec-system cutover receipt step")

receipt = {
    "schema_version": 1,
    "schema_id": schema_id,
    "tool": "spec-system-cutover-receipt",
    "result": "Passed",
    "claim_boundary": [
        "delegate_spec_system_enabled_repo_wide",
        "embedded_spec_system_ci_audit_retired",
        "fail_closed_for_legacy_spec_system_commands",
        "no_cargo_intent_audit_vertical_claimed",
        "no_physical_repository_extraction",
    ],
    "delegation_config": {
        "path": str(config_path.relative_to(root)),
        "schema_id": "cargo-allow.intent-delegation.v1",
        "delegate_spec_system": True,
        "delegate_staged_precommit": True,
    },
    "limitations": [
        "spec_system_modules_remain_for_external_repos_without_cutover",
        "cargo_intent_audit_doctor_worklist_not_shipped",
        "rollback_via_disable_delegate_spec_system",
    ],
}

receipt_path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
print("cutover_ok")
PY

log "receipt: ${receipt}"
log "SpecSystemCutoverReceiptV1 Passed"
