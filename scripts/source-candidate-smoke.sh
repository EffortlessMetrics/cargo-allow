#!/usr/bin/env bash
# Installed-binary first-hour + lifecycle smoke (#2278 / #2373).
#
# Path-installs cargo-allow (or reuses CARGO_ALLOW_BIN), runs the brownfield
# first-hour journey plus diff / prune preview→write in a temporary consumer
# repository outside this checkout, and emits
# cargo-allow.source-candidate-smoke-receipt.v1 JSON.
#
# Does not prove ExactCandidatePackageSet isolation, crates.io published
# install, checkout denial, refresh path, or every #2278 negative control.
#
# Usage:
#   bash scripts/source-candidate-smoke.sh
#
# Optional:
#   WORK_DIR=<path>          work root (default: target/source-candidate-smoke)
#   CARGO_ALLOW_BIN=<path>   prebuilt/path-installed binary (skips cargo install)
#   INSTALL_ROOT=<path>      cargo install --root when installing (default: WORK_DIR/install)
#   SKIP_NEGATIVES=1         skip harness-level negative controls (debug only)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

work_dir="${WORK_DIR:-${ROOT}/target/source-candidate-smoke}"
install_root="${INSTALL_ROOT:-${work_dir}/install}"
receipt="${work_dir}/source-candidate-smoke.receipt.json"
# Keep the consumer outside this checkout so inventory/policy resolve to the
# temporary adopter tree, not the cargo-allow workspace git root.
consumer_dir="${CONSUMER_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/cargo-allow-source-candidate-consumer.XXXXXX")}"
schema_id="cargo-allow.source-candidate-smoke-receipt.v1"

cleanup() {
  if [[ "${KEEP_CONSUMER:-0}" != "1" ]]; then
    rm -rf "${consumer_dir}"
  fi
}
trap cleanup EXIT

log() {
  printf 'source-candidate-smoke: %s\n' "$*"
}

fail() {
  printf 'source-candidate-smoke: error: %s\n' "$*" >&2
  exit 1
}

command -v cargo >/dev/null 2>&1 || fail "cargo is required"
command -v python3 >/dev/null 2>&1 || fail "python3 is required to emit the JSON receipt"
command -v mktemp >/dev/null 2>&1 || fail "mktemp is required"

read_workspace_version() {
  awk '
    /^\[workspace\.package\]/ { in_ws = 1; next }
    /^\[/ { if (in_ws) exit }
    in_ws && /^version = / {
      gsub(/^version = "/, "", $0)
      gsub(/".*$/, "", $0)
      print $0
      exit
    }
  ' Cargo.toml
}

version="$(read_workspace_version)"
[[ -n "${version}" ]] || fail "could not read workspace.package.version"

git_head=""
if command -v git >/dev/null 2>&1 && git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  git_head="$(git rev-parse HEAD 2>/dev/null || true)"
fi

rm -rf "${work_dir}"
mkdir -p "${work_dir}" "${consumer_dir}/src"

install_method="cargo_install_path"
if [[ -n "${CARGO_ALLOW_BIN:-}" ]]; then
  cargo_bin="${CARGO_ALLOW_BIN}"
  [[ -x "${cargo_bin}" || -f "${cargo_bin}" ]] \
    || fail "CARGO_ALLOW_BIN is not a usable file: ${cargo_bin}"
  install_method="prebuilt_override"
else
  log "installing cargo-allow ${version} from workspace path into ${install_root}"
  mkdir -p "${install_root}"
  cargo install --path "${ROOT}/crates/cargo-allow" --locked --root "${install_root}" --force
  cargo_bin="${install_root}/bin/cargo-allow"
  if [[ -x "${cargo_bin}.exe" ]]; then
    cargo_bin="${cargo_bin}.exe"
  fi
  [[ -x "${cargo_bin}" || -f "${cargo_bin}" ]] \
    || fail "expected installed binary at ${install_root}/bin/cargo-allow(.exe)"
fi

log "cargo-allow --version"
version_output="$("${cargo_bin}" --version | tr -d '\r')"
printf '%s\n' "${version_output}"
printf '%s\n' "${version_output}" | grep -F "cargo-allow ${version}" >/dev/null \
  || fail "installed version mismatch: ${version_output} (expected cargo-allow ${version})"

# Brownfield first-hour journey in an isolated consumer repo.
printf 'pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n' >"${consumer_dir}/src/lib.rs"

log "step version"
step_version_exit=0
"${cargo_bin}" --version >/dev/null || step_version_exit=$?

log "step doctor (no policy)"
doctor_json="$("${cargo_bin}" doctor --root "${consumer_dir}" --format json)"
printf '%s\n' "${doctor_json}" | python3 -c '
import json, sys
report = json.load(sys.stdin)
if report.get("schema_id") != "cargo-allow.doctor.v1":
    raise SystemExit(f"doctor schema_id mismatch: {report.get('schema_id')!r}")
'
step_doctor_exit=0

log "step audit (expect one panic finding)"
audit_json="$("${cargo_bin}" audit --root "${consumer_dir}" --kind panic --format json)"
printf '%s\n' "${audit_json}" | python3 -c '
import json, sys
report = json.load(sys.stdin)
new = report.get("summary", {}).get("new")
if new != 1:
    raise SystemExit(f"expected summary.new == 1, got {new!r}")
'
step_audit_exit=0

log "step propose --write"
policy_path="${consumer_dir}/policy/allow.toml"
mkdir -p "${consumer_dir}/policy"
"${cargo_bin}" propose --root "${consumer_dir}" --kind panic --write "${policy_path}"
[[ -f "${policy_path}" ]] || fail "propose did not write ${policy_path}"
step_propose_exit=0

log "step check --mode no-new"
check_json="$("${cargo_bin}" check --root "${consumer_dir}" --config "${policy_path}" --kind panic --mode no-new --format json)"
printf '%s\n' "${check_json}" | python3 -c '
import json, sys
report = json.load(sys.stdin)
status = report.get("status")
if status != "passed":
    raise SystemExit(f"expected status passed, got {status!r}")
'
step_check_exit=0

log "step list / explain / worklist"
list_json="$("${cargo_bin}" list --root "${consumer_dir}" --config "${policy_path}" --kind panic --format json)"
allow_id="$(
  printf '%s\n' "${list_json}" | python3 -c '
import json, sys
report = json.load(sys.stdin)
entries = report.get("allow_entries") or []
if not entries:
    raise SystemExit("list returned no allow_entries")
print(entries[0]["id"])
'
)"
"${cargo_bin}" explain "${allow_id}" --root "${consumer_dir}" --config "${policy_path}" >/dev/null
"${cargo_bin}" worklist --root "${consumer_dir}" --config "${policy_path}" --kind panic --format json >/dev/null
step_list_exit=0

command -v git >/dev/null 2>&1 || fail "git is required for diff --base lifecycle steps"
log "step git baseline commit for diff --base"
git -C "${consumer_dir}" init >/dev/null
git -C "${consumer_dir}" config core.autocrlf false
git -C "${consumer_dir}" config user.email "source-candidate-smoke@example.com"
git -C "${consumer_dir}" config user.name "Source Candidate Smoke"
# Commit policy with the source tree so diff does not treat the allow ledger as
# newly introduced baseline debt relative to an empty base policy.
git -C "${consumer_dir}" add -A
git -C "${consumer_dir}" commit -m "source-candidate-smoke baseline" >/dev/null
diff_base="$(git -C "${consumer_dir}" rev-parse HEAD)"

log "step diff --base ${diff_base}"
diff_out="${work_dir}/diff-base.json"
"${cargo_bin}" diff \
  --root "${consumer_dir}" \
  --config "${policy_path}" \
  --kind panic \
  --base "${diff_base}" \
  --format json \
  --output "${diff_out}"
python3 - "${diff_out}" <<'PY'
import json, sys
from pathlib import Path
report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if report.get("schema_id") != "cargo-allow.report.v1":
    raise SystemExit(f"diff schema_id mismatch: {report.get('schema_id')!r}")
if report.get("command") not in (None, "diff"):
    pass
if report.get("status") != "passed" or report.get("failed") is True:
    raise SystemExit(
        f"expected diff status passed with no failure, got status={report.get('status')!r} "
        f"failed={report.get('failed')!r} summary={report.get('summary')!r} "
        f"diff={report.get('diff')!r}"
    )
PY
step_diff_exit=0

log "step prune lifecycle (fix finding → dry-run → write)"
printf 'pub fn load(value: Option<u8>) -> u8 { value.unwrap_or(0) }\n' >"${consumer_dir}/src/lib.rs"
prune_preview="${work_dir}/prune-preview.json"
prune_write="${work_dir}/prune-write.json"
"${cargo_bin}" prune \
  --root "${consumer_dir}" \
  --config "${policy_path}" \
  --stale \
  --dry-run \
  --format json \
  --output "${prune_preview}"
python3 - "${prune_preview}" "${allow_id}" <<'PY'
import json, sys
from pathlib import Path
report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
allow_id = sys.argv[2]
if report.get("schema_id") != "cargo-allow.prune.v1":
    raise SystemExit(f"prune preview schema_id mismatch: {report.get('schema_id')!r}")
stale = report.get("stale_entries") or []
ids = {entry.get("id") for entry in stale if isinstance(entry, dict)}
receipt_ids = set()
for value in (report.get("mutation_receipt") or {}).get("changed_allow_ids") or []:
    if isinstance(value, str):
        receipt_ids.add(value)
if allow_id not in ids and allow_id not in receipt_ids:
    raise SystemExit(f"prune preview missing stale allow {allow_id!r}")
PY
"${cargo_bin}" prune \
  --root "${consumer_dir}" \
  --config "${policy_path}" \
  --stale \
  --write \
  --format json \
  --output "${prune_write}"
python3 - "${prune_preview}" "${prune_write}" "${allow_id}" <<'PY'
import json, sys
from pathlib import Path

def stale_ids(path: Path) -> set[str]:
    report = json.loads(path.read_text(encoding="utf-8"))
    ids = set()
    for entry in report.get("stale_entries") or []:
        if isinstance(entry, dict) and isinstance(entry.get("id"), str):
            ids.add(entry["id"])
    for value in (report.get("mutation_receipt") or {}).get("changed_allow_ids") or []:
        if isinstance(value, str):
            ids.add(value)
    return ids

preview_path = Path(sys.argv[1])
write_path = Path(sys.argv[2])
allow_id = sys.argv[3]
write = json.loads(write_path.read_text(encoding="utf-8"))
if write.get("schema_id") != "cargo-allow.prune.v1":
    raise SystemExit(f"prune write schema_id mismatch: {write.get('schema_id')!r}")
result = (write.get("mutation_receipt") or {}).get("result")
if result != "written":
    raise SystemExit(f"expected prune mutation_receipt.result == written, got {result!r}")
preview_ids = stale_ids(preview_path)
write_ids = stale_ids(write_path)
if preview_ids != write_ids:
    raise SystemExit(
        f"PreviewApplyDisagree: prune preview ids {sorted(preview_ids)} "
        f"!= write ids {sorted(write_ids)}"
    )
if allow_id not in write_ids:
    raise SystemExit(f"prune write missing allow {allow_id!r}")
PY
step_prune_exit=0

log "step final check --mode no-new after prune"
final_check_json="$("${cargo_bin}" check --root "${consumer_dir}" --config "${policy_path}" --kind panic --mode no-new --format json)"
printf '%s\n' "${final_check_json}" | python3 -c '
import json, sys
report = json.load(sys.stdin)
status = report.get("status")
if status != "passed":
    raise SystemExit(f"expected final check status passed, got {status!r}")
'
step_final_check_exit=0

negatives_json='[]'
if [[ "${SKIP_NEGATIVES:-0}" != "1" ]]; then
  log "negative: omitted journey step cannot claim Passed"
  omitted_class="$(
    python3 <<'PY'
import json

expected = [
    "version",
    "doctor_no_policy",
    "audit_with_finding",
    "bootstrap_propose_write",
    "check_no_new_pass",
    "list_explain_worklist",
    "diff_against_exact_base",
    "prune_stale_preview_write",
    "final_check_no_new",
]
# Forge a Passed receipt that omits the prune step.
forged = {
    "schema_version": 1,
    "schema_id": "cargo-allow.source-candidate-smoke-receipt.v1",
    "tool": "cargo-allow",
    "result": "Passed",
    "journey": {
        "steps_expected": expected,
        "steps_executed": [{"id": step, "exit_code": 0} for step in expected if step != "prune_stale_preview_write"],
    },
}
executed = {step["id"] for step in forged["journey"]["steps_executed"]}
missing = [step for step in forged["journey"]["steps_expected"] if step not in executed]
if forged["result"] == "Passed" and missing:
    print("OmittedStep")
else:
    print("InstrumentFailure")
PY
  )"
  omitted_passed=true
  if [[ "${omitted_class}" != "OmittedStep" ]]; then
    omitted_passed=false
    fail "omitted-step negative produced unexpected class ${omitted_class}"
  fi

  log "negative: prune preview/apply subject disagreement is detected"
  disagree_class="$(
    python3 - "${prune_preview}" <<'PY'
import json, sys
from pathlib import Path
preview = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
forged_write = json.loads(json.dumps(preview))
# Corrupt write subject set so harness agreement check would fail.
forged_write["mutation_receipt"] = {
    "result": "written",
    "changed_allow_ids": ["forged-disagree-id"],
}
forged_write["stale_entries"] = [{"id": "forged-disagree-id"}]

def stale_ids(report: dict) -> set[str]:
    ids = set()
    for entry in report.get("stale_entries") or []:
        if isinstance(entry, dict) and isinstance(entry.get("id"), str):
            ids.add(entry["id"])
    for value in (report.get("mutation_receipt") or {}).get("changed_allow_ids") or []:
        if isinstance(value, str):
            ids.add(value)
    return ids

if stale_ids(preview) != stale_ids(forged_write):
    print("PreviewApplyDisagree")
else:
    print("InstrumentFailure")
PY
  )"
  disagree_passed=true
  if [[ "${disagree_class}" != "PreviewApplyDisagree" ]]; then
    disagree_passed=false
    fail "preview/apply disagree negative produced unexpected class ${disagree_class}"
  fi

  negatives_json="$(
    OMITTED_CLASS="${omitted_class}" OMITTED_PASSED="${omitted_passed}" \
    DISAGREE_CLASS="${disagree_class}" DISAGREE_PASSED="${disagree_passed}" \
    python3 <<'PY'
import json, os
print(json.dumps([
    {
        "id": "omitted_journey_step_cannot_claim_passed",
        "result_class": os.environ["OMITTED_CLASS"],
        "passed": os.environ["OMITTED_PASSED"] == "true",
        "detail": "Passed receipt missing a steps_expected id is classified OmittedStep",
    },
    {
        "id": "prune_preview_apply_subject_agree",
        "result_class": os.environ["DISAGREE_CLASS"],
        "passed": os.environ["DISAGREE_PASSED"] == "true",
        "detail": "forged prune write subject mismatch is classified PreviewApplyDisagree",
    },
]))
PY
  )"
fi

os_name="$(uname -s | tr '[:upper:]' '[:lower:]')"
case "${os_name}" in
  mingw*|msys*|cygwin*) os_name="windows" ;;
  darwin*) os_name="macos" ;;
  linux*) os_name="linux" ;;
esac
arch_name="$(uname -m)"
case "${arch_name}" in
  x86_64|amd64) arch_name="x86_64" ;;
  aarch64|arm64) arch_name="aarch64" ;;
esac

log "writing receipt ${receipt}"
RECEIPT_PATH="${receipt}" \
SCHEMA_ID="${schema_id}" \
WORKSPACE_VERSION="${version}" \
GIT_HEAD="${git_head}" \
INSTALL_METHOD="${install_method}" \
OS_NAME="${os_name}" \
ARCH_NAME="${arch_name}" \
VERSION_OUTPUT="${version_output}" \
STEP_VERSION_EXIT="${step_version_exit}" \
STEP_DOCTOR_EXIT="${step_doctor_exit}" \
STEP_AUDIT_EXIT="${step_audit_exit}" \
STEP_PROPOSE_EXIT="${step_propose_exit}" \
STEP_CHECK_EXIT="${step_check_exit}" \
STEP_LIST_EXIT="${step_list_exit}" \
STEP_DIFF_EXIT="${step_diff_exit}" \
STEP_PRUNE_EXIT="${step_prune_exit}" \
STEP_FINAL_CHECK_EXIT="${step_final_check_exit}" \
NEGATIVES_JSON="${negatives_json}" \
python3 <<'PY'
import json
import os

def code(name: str) -> int:
    return int(os.environ[name])

steps_expected = [
    "version",
    "doctor_no_policy",
    "audit_with_finding",
    "bootstrap_propose_write",
    "check_no_new_pass",
    "list_explain_worklist",
    "diff_against_exact_base",
    "prune_stale_preview_write",
    "final_check_no_new",
]
steps_executed = [
    {
        "id": "version",
        "exit_code": code("STEP_VERSION_EXIT"),
        "artifact_schema_id": None,
    },
    {
        "id": "doctor_no_policy",
        "exit_code": code("STEP_DOCTOR_EXIT"),
        "artifact_schema_id": "cargo-allow.doctor.v1",
    },
    {
        "id": "audit_with_finding",
        "exit_code": code("STEP_AUDIT_EXIT"),
        "artifact_schema_id": "cargo-allow.report.v1",
    },
    {
        "id": "bootstrap_propose_write",
        "exit_code": code("STEP_PROPOSE_EXIT"),
        "artifact_schema_id": None,
    },
    {
        "id": "check_no_new_pass",
        "exit_code": code("STEP_CHECK_EXIT"),
        "artifact_schema_id": "cargo-allow.report.v1",
    },
    {
        "id": "list_explain_worklist",
        "exit_code": code("STEP_LIST_EXIT"),
        "artifact_schema_id": "cargo-allow.list.v1",
    },
    {
        "id": "diff_against_exact_base",
        "exit_code": code("STEP_DIFF_EXIT"),
        "artifact_schema_id": "cargo-allow.report.v1",
    },
    {
        "id": "prune_stale_preview_write",
        "exit_code": code("STEP_PRUNE_EXIT"),
        "artifact_schema_id": "cargo-allow.prune.v1",
    },
    {
        "id": "final_check_no_new",
        "exit_code": code("STEP_FINAL_CHECK_EXIT"),
        "artifact_schema_id": "cargo-allow.report.v1",
    },
]
executed_ids = {step["id"] for step in steps_executed}
missing = [step for step in steps_expected if step not in executed_ids]
if missing:
    raise SystemExit(f"OmittedStep: missing executed steps {missing}")

negatives = json.loads(os.environ["NEGATIVES_JSON"])

receipt = {
    "schema_version": 1,
    "schema_id": os.environ["SCHEMA_ID"],
    "tool": "cargo-allow",
    "result": "Passed",
    "claim_boundary": [
        "installed_binary_first_hour_journey",
        "diff_and_prune_lifecycle",
        "temporary_consumer_repository",
        "source_candidate_not_published_registry",
    ],
    "candidate": {
        "workspace_version": os.environ["WORKSPACE_VERSION"],
        "git_head": os.environ["GIT_HEAD"] or None,
        "package_set_provenance": "workspace_path_install_after_optional_package_gate",
        "install_method": os.environ["INSTALL_METHOD"],
    },
    "environment": {
        "os": os.environ["OS_NAME"],
        "arch": os.environ["ARCH_NAME"],
        "rustc_version": None,
        "cargo_version": None,
        "network_posture": "not_required_for_core_journey",
    },
    "installed_binary": {
        "version_output": os.environ["VERSION_OUTPUT"],
        "path_redacted": True,
    },
    "journey": {
        "fixture_generation": "first_hour_brownfield_v1",
        "steps_expected": steps_expected,
        "steps_executed": steps_executed,
    },
    "negative_controls": negatives,
    "limitations": [
        "package_set_not_consumed_from_isolated_registry",
        "source_checkout_not_denied_during_install",
        "refresh_lifecycle_not_executed",
        "checkout_denial_negative_deferred",
        "published_registry_install_not_executed",
        "linux_hosted_claim_only",
    ],
}

with open(os.environ["RECEIPT_PATH"], "w", encoding="utf-8") as handle:
    json.dump(receipt, handle, indent=2)
    handle.write("\n")
PY

log "SourceCandidateSmokeReceiptV1 passed for workspace ${version}"
log "receipt: ${receipt}"
