#!/usr/bin/env bash
# Installed-binary first-hour journey smoke (#2278 Stage A+).
#
# Path-installs cargo-allow (or reuses CARGO_ALLOW_BIN), runs the brownfield
# first-hour journey in a temporary consumer repository outside this checkout,
# and emits cargo-allow.source-candidate-smoke-receipt.v1 JSON.
#
# Does not prove ExactCandidatePackageSetV1 local-registry isolation (#2277),
# crates.io published install, checkout denial, or diff/refresh/prune negatives.
#
# Usage:
#   bash scripts/source-candidate-smoke.sh
#
# Optional:
#   WORK_DIR=<path>          work root (default: target/source-candidate-smoke)
#   CARGO_ALLOW_BIN=<path>   prebuilt/path-installed binary (skips cargo install)
#   INSTALL_ROOT=<path>      cargo install --root when installing (default: WORK_DIR/install)
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
python3 <<'PY'
import json
import os

def code(name: str) -> int:
    return int(os.environ[name])

receipt = {
    "schema_version": 1,
    "schema_id": os.environ["SCHEMA_ID"],
    "tool": "cargo-allow",
    "result": "Passed",
    "claim_boundary": [
        "installed_binary_first_hour_journey",
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
        "steps_expected": [
            "version",
            "doctor_no_policy",
            "audit_with_finding",
            "bootstrap_propose_write",
            "check_no_new_pass",
            "list_explain_worklist",
        ],
        "steps_executed": [
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
        ],
    },
    "limitations": [
        "package_set_not_consumed_from_isolated_registry",
        "source_checkout_not_denied_during_install",
        "lifecycle_diff_refresh_prune_not_executed",
        "negative_controls_not_run",
        "published_registry_install_not_executed",
    ],
}

with open(os.environ["RECEIPT_PATH"], "w", encoding="utf-8") as handle:
    json.dump(receipt, handle, indent=2)
    handle.write("\n")
PY

log "SourceCandidateSmokeReceiptV1 passed for workspace ${version}"
log "receipt: ${receipt}"
