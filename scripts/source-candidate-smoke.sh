#!/usr/bin/env bash
# Installed-binary first-hour + lifecycle smoke (#2278 / #2373 / #2387 / #2396 /
# #2398 / #2400 / #2402 / #2403).
#
# Path-installs cargo-allow (or reuses CARGO_ALLOW_BIN), runs the brownfield
# first-hour journey plus refresh / diff / prune preview→write and git policy
# rollback after prune in a temporary consumer repository outside this
# checkout, and emits cargo-allow.source-candidate-smoke-receipt.v1 JSON.
#
# Includes post-install source-hidden ordinary-scan denial, wrong-version /
# MissingAsset package-rebuild omit, ordinary-scan offline / unexpected-network
# classification, policy rollback after prune, and optional-profile-without-
# assets NotProven. Does not prove ExactCandidatePackageSet isolation,
# crates.io published install, or deny-source during path install.
#
# Usage:
#   bash scripts/source-candidate-smoke.sh
#
# Optional:
#   CARGO_ALLOW_BIN=<path>   prebuilt/path-installed binary (skips cargo install)
#   SKIP_NEGATIVES=1         skip harness-level negative controls (debug only)
set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lifecycle="${SCRIPT_ROOT}/scripts/candidate-harness-owned-dir.py"
command -v python3 >/dev/null 2>&1 || { printf 'source-candidate-smoke: error: python3 is required\n' >&2; exit 1; }
if [[ -n "${CONSUMER_DIR:-}" ]]; then
  python3 "${lifecycle}" validate-caller --repository "${SCRIPT_ROOT}" --path "${CONSUMER_DIR}" >/dev/null
fi

if [[ "${1:-}" != "--internal" ]]; then
  if [[ -n "${CARGO_ALLOW_BIN:-}" ]]; then
    CARGO_ALLOW_BIN="$(python3 - "${CARGO_ALLOW_BIN}" "${SCRIPT_ROOT}/target" <<'PY'
import sys
from pathlib import Path

raw = Path(sys.argv[1])
target = Path(sys.argv[2]).resolve(strict=True)
if not raw.exists() or raw.is_symlink() or not raw.is_file():
    raise SystemExit("CARGO_ALLOW_BIN must be an existing non-symlink file")
resolved = raw.resolve(strict=True)
try:
    resolved.relative_to(target)
except ValueError as error:
    raise SystemExit(f"CARGO_ALLOW_BIN must be below {target}, got {resolved}") from error
print(resolved)
PY
)"
    export CARGO_ALLOW_BIN
  fi
  temp_root="${CANDIDATE_HARNESS_TEST_ROOT:-${TMPDIR:-/tmp}}"
  python3 "${lifecycle}" validate-test-root --root "${temp_root}" --repository "${SCRIPT_ROOT}" >/dev/null
  snapshot_json="$(python3 "${lifecycle}" snapshot --root "${temp_root}" --repository "${SCRIPT_ROOT}" --purpose source-candidate-snapshot)"
  read -r snapshot_root snapshot_token snapshot_head < <(
    printf '%s' "${snapshot_json}" | python3 -c 'import json,sys; v=json.load(sys.stdin); print(v["path"], v["token"], v["git_head"])'
  )
  snapshot_cleanup() {
    python3 "${lifecycle}" remove --root "${temp_root}" --path "${snapshot_root}" \
      --purpose source-candidate-snapshot --token "${snapshot_token}"
  }
  trap snapshot_cleanup EXIT
  bash "${BASH_SOURCE[0]}" --internal "${snapshot_root}" "${snapshot_token}" "${snapshot_head}"
  exit $?
fi

[[ "$#" -eq 4 ]] || { printf 'source-candidate-smoke: error: invalid internal invocation\n' >&2; exit 1; }
snapshot_root="$2"
snapshot_token="$3"
snapshot_head="$4"
export CANDIDATE_HARNESS_ROOT="$snapshot_root" CANDIDATE_HARNESS_GIT_HEAD="$snapshot_head"
python3 "${lifecycle}" verify --root "${CANDIDATE_HARNESS_TEST_ROOT:-${TMPDIR:-/tmp}}" \
  --path "${snapshot_root}" --purpose source-candidate-snapshot \
  --token "${snapshot_token}" --git-head "${snapshot_head}" --repository "${SCRIPT_ROOT}"

ROOT="${CANDIDATE_HARNESS_ROOT}"
cd "${ROOT}"
if [[ "${CANDIDATE_HARNESS_SNAPSHOT_PROBE:-0}" == "1" && "${CANDIDATE_HARNESS_TEST_INJECTION:-0}" == "1" ]]; then
  [[ "${ROOT}" != "${SCRIPT_ROOT}" && -f "${ROOT}/Cargo.toml" && -d "${ROOT}/crates/cargo-allow/src" && -d "${ROOT}/docs/templates" ]]
  printf 'source-candidate-smoke: disposable snapshot ok\n'
  exit 0
fi

output_root="${SCRIPT_ROOT}/target"
python3 "${lifecycle}" validate-target --repository "${SCRIPT_ROOT}" --path "${output_root}"
mkdir -p "${output_root}"
python3 "${lifecycle}" validate-target --repository "${SCRIPT_ROOT}" --path "${output_root}"
source_parent="${WORK_DIR:-${output_root}/source-candidate-smoke}"
if [[ ! -e "${source_parent}" ]]; then
  mkdir "${source_parent}"
fi
if [[ -n "${WORK_DIR:-}" ]]; then
  python3 "${lifecycle}" validate-work --repository "${SCRIPT_ROOT}" --target "${output_root}" \
    --path "${source_parent}" >/dev/null
else
  python3 "${lifecycle}" validate-contained --root "${output_root}" --path "${source_parent}" >/dev/null
fi
work_json="$(python3 "${lifecycle}" allocate --root "${source_parent}" --purpose source-candidate-smoke)"
read -r work_dir work_token < <(
  printf '%s' "${work_json}" | python3 -c 'import json,sys; v=json.load(sys.stdin); print(v["path"], v["token"])'
)
install_root="${source_parent}/install"
receipt="${source_parent}/source-candidate-smoke.receipt.json"
# Keep the consumer outside this checkout so inventory/policy resolve to the
# temporary adopter tree, not the cargo-allow workspace git root.
consumer_parent="${CANDIDATE_HARNESS_TEST_ROOT:-${TMPDIR:-/tmp}}"
if [[ -n "${CONSUMER_DIR:-}" ]]; then
  consumer_dir="${CONSUMER_DIR}"
  consumer_parent="$(dirname "${consumer_dir}")"
  python3 "${lifecycle}" validate-caller --repository "${SCRIPT_ROOT}" --path "${consumer_dir}" >/dev/null
  if [[ ! -e "${consumer_dir}" ]]; then
    mkdir -p "${consumer_dir}"
  fi
  consumer_json="$(python3 "${lifecycle}" claim --path "${consumer_dir}" --purpose source-candidate-consumer)"
else
  consumer_json="$(python3 "${lifecycle}" allocate --root "${consumer_parent}" --purpose source-candidate-consumer)"
fi
read -r consumer_dir consumer_token < <(
  printf '%s' "${consumer_json}" | python3 -c 'import json,sys; v=json.load(sys.stdin); print(v["path"], v["token"])'
)
schema_id="cargo-allow.source-candidate-smoke-receipt.v1"

src_path="${ROOT}/crates/cargo-allow/src"
src_stash="${work_dir}/stashed-cargo-allow-src"
templates_path="${ROOT}/docs/templates"
templates_stash="${work_dir}/stashed-docs-templates"

restore_source_tree() {
  if [[ -d "${src_stash}" ]]; then
    [[ ! -e "${src_path}" ]] || fail "refusing to overwrite existing source tree during restore"
    python3 "${lifecycle}" restore --stash "${src_stash}" --destination "${src_path}"
  fi
  if [[ -d "${templates_stash}" ]]; then
    [[ ! -e "${templates_path}" ]] || fail "refusing to overwrite existing templates during restore"
    python3 "${lifecycle}" restore --stash "${templates_stash}" --destination "${templates_path}"
  fi
}

cleanup() {
  restore_source_tree
  python3 "${lifecycle}" remove --root "${consumer_parent}" --path "${consumer_dir}" \
    --purpose source-candidate-consumer --token "${consumer_token}"
  python3 "${lifecycle}" remove --root "${source_parent}" --path "${work_dir}" \
    --purpose source-candidate-smoke --token "${work_token}"
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

git_head="${CANDIDATE_HARNESS_GIT_HEAD:-}"

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

log "step refresh lifecycle (induce location_drift → dry-run → write)"
python3 - "${policy_path}" <<'PY'
from pathlib import Path
import re
import sys

path = Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
marker = "[allow.last_seen]"
idx = text.find(marker)
if idx < 0:
    raise SystemExit("proposed policy missing [allow.last_seen]")
section = text[idx:]
section2, n = re.subn(r"(?m)^(line\s*=\s*)\d+", r"\g<1>99", section, count=1)
if n != 1:
    raise SystemExit("failed to corrupt allow.last_seen.line")
path.write_text(text[:idx] + section2, encoding="utf-8")
print("induced_location_drift=allow.last_seen.line=99")
PY
refresh_preview="${work_dir}/refresh-preview.json"
refresh_write="${work_dir}/refresh-write.json"
"${cargo_bin}" refresh \
  --root "${consumer_dir}" \
  --config "${policy_path}" \
  --allow-id "${allow_id}" \
  --dry-run \
  --format json \
  --output "${refresh_preview}"
python3 - "${refresh_preview}" "${allow_id}" <<'PY'
import json, sys
from pathlib import Path
report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
allow_id = sys.argv[2]
if report.get("schema_id") != "cargo-allow.refresh.v1":
    raise SystemExit(f"refresh preview schema_id mismatch: {report.get('schema_id')!r}")
ids = set()
for value in (report.get("mutation_receipt") or {}).get("changed_allow_ids") or []:
    if isinstance(value, str):
        ids.add(value)
if allow_id not in ids:
    raise SystemExit(f"refresh preview missing allow {allow_id!r}")
mode = report.get("mode") or {}
if mode.get("write_requested") is True:
    raise SystemExit("refresh preview unexpectedly requested write")
PY
"${cargo_bin}" refresh \
  --root "${consumer_dir}" \
  --config "${policy_path}" \
  --allow-id "${allow_id}" \
  --write \
  --format json \
  --output "${refresh_write}"
python3 - "${refresh_preview}" "${refresh_write}" "${allow_id}" <<'PY'
import json, sys
from pathlib import Path

def changed_ids(path: Path) -> set[str]:
    report = json.loads(path.read_text(encoding="utf-8"))
    ids = set()
    for value in (report.get("mutation_receipt") or {}).get("changed_allow_ids") or []:
        if isinstance(value, str):
            ids.add(value)
    return ids

preview_path = Path(sys.argv[1])
write_path = Path(sys.argv[2])
allow_id = sys.argv[3]
write = json.loads(write_path.read_text(encoding="utf-8"))
if write.get("schema_id") != "cargo-allow.refresh.v1":
    raise SystemExit(f"refresh write schema_id mismatch: {write.get('schema_id')!r}")
result = (write.get("mutation_receipt") or {}).get("result")
if result != "written":
    raise SystemExit(f"expected refresh mutation_receipt.result == written, got {result!r}")
preview_ids = changed_ids(preview_path)
write_ids = changed_ids(write_path)
if preview_ids != write_ids:
    raise SystemExit(
        f"PreviewApplyDisagree: refresh preview ids {sorted(preview_ids)} "
        f"!= write ids {sorted(write_ids)}"
    )
if allow_id not in write_ids:
    raise SystemExit(f"refresh write missing allow {allow_id!r}")
PY
step_refresh_exit=0

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

log "step policy rollback after prune (git restore from baseline)"
python3 - "${prune_write}" <<'PY'
import json, sys
from pathlib import Path
report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
next_commands = (report.get("mutation_receipt") or {}).get("next_commands") or []
if not any(isinstance(cmd, str) and cmd.startswith("git diff -- ") for cmd in next_commands):
    raise SystemExit(
        f"prune write mutation_receipt.next_commands missing git recovery hint: {next_commands!r}"
    )
if not any(
    isinstance(cmd, str) and "check --mode no-new" in cmd for cmd in next_commands
):
    raise SystemExit(
        f"prune write mutation_receipt.next_commands missing check recovery hint: {next_commands!r}"
    )
PY
post_prune_list="$("${cargo_bin}" list --root "${consumer_dir}" --config "${policy_path}" --kind panic --format json)"
printf '%s\n' "${post_prune_list}" | ALLOW_ID="${allow_id}" python3 -c '
import json, os, sys
report = json.load(sys.stdin)
allow_id = os.environ["ALLOW_ID"]
entries = report.get("allow_entries") or []
ids = {entry.get("id") for entry in entries if isinstance(entry, dict)}
if allow_id in ids:
    raise SystemExit(f"expected allow {allow_id!r} absent after prune, still present")
'
# Baseline commit predates prune write; restore the ledger file from HEAD.
policy_rel="${policy_path#"${consumer_dir}/"}"
git -C "${consumer_dir}" checkout HEAD -- "${policy_rel}"
[[ -f "${policy_path}" ]] || fail "policy rollback did not restore ${policy_path}"
restored_list="$("${cargo_bin}" list --root "${consumer_dir}" --config "${policy_path}" --kind panic --format json)"
printf '%s\n' "${restored_list}" | ALLOW_ID="${allow_id}" python3 -c '
import json, os, sys
report = json.load(sys.stdin)
allow_id = os.environ["ALLOW_ID"]
entries = report.get("allow_entries") or []
ids = {entry.get("id") for entry in entries if isinstance(entry, dict)}
if allow_id not in ids:
    raise SystemExit(f"policy rollback failed to restore allow {allow_id!r}")
'
step_rollback_exit=0

negatives_json='[]'
if [[ "${SKIP_NEGATIVES:-0}" != "1" ]]; then
  log "negative: omitted journey step cannot claim Passed"
  omitted_class="$(
    python3 <<'PY'
expected = [
    "version",
    "doctor_no_policy",
    "audit_with_finding",
    "bootstrap_propose_write",
    "check_no_new_pass",
    "list_explain_worklist",
    "refresh_location_drift_preview_write",
    "diff_against_exact_base",
    "prune_stale_preview_write",
    "final_check_no_new",
    "policy_rollback_after_prune",
]
# Forge a Passed receipt that omits the refresh step.
forged_executed = [
    {"id": step, "exit_code": 0}
    for step in expected
    if step != "refresh_location_drift_preview_write"
]
executed = {step["id"] for step in forged_executed}
missing = [step for step in expected if step not in executed]
if missing:
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

  log "negative: refresh preview/apply subject disagreement is detected"
  refresh_disagree_class="$(
    python3 - "${refresh_preview}" <<'PY'
import json, sys
from pathlib import Path
preview = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
forged_write = json.loads(json.dumps(preview))
forged_write["mutation_receipt"] = {
    "result": "written",
    "changed_allow_ids": ["forged-refresh-disagree-id"],
}

def changed_ids(report: dict) -> set[str]:
    ids = set()
    for value in (report.get("mutation_receipt") or {}).get("changed_allow_ids") or []:
        if isinstance(value, str):
            ids.add(value)
    return ids

if changed_ids(preview) != changed_ids(forged_write):
    print("PreviewApplyDisagree")
else:
    print("InstrumentFailure")
PY
  )"
  refresh_disagree_passed=true
  if [[ "${refresh_disagree_class}" != "PreviewApplyDisagree" ]]; then
    refresh_disagree_passed=false
    fail "refresh preview/apply disagree negative produced unexpected class ${refresh_disagree_class}"
  fi

  log "negative: malformed smoke receipt schema cannot claim Passed"
  malformed_class="$(
    python3 <<'PY'
forged = {
    "schema_version": 1,
    "schema_id": "cargo-allow.source-candidate-smoke-receipt.v0-forged",
    "tool": "cargo-allow",
    "result": "Passed",
}
if forged["result"] == "Passed" and forged["schema_id"] != "cargo-allow.source-candidate-smoke-receipt.v1":
    print("MalformedArtifact")
else:
    print("InstrumentFailure")
PY
  )"
  malformed_passed=true
  if [[ "${malformed_class}" != "MalformedArtifact" ]]; then
    malformed_passed=false
    fail "malformed-receipt negative produced unexpected class ${malformed_class}"
  fi

  log "negative: ordinary scan must not require source checkout after install"
  [[ -d "${src_path}" ]] || fail "expected source tree at ${src_path}"
  [[ -d "${templates_path}" ]] || fail "expected templates at ${templates_path}"
  python3 "${lifecycle}" restore --stash "${src_path}" --destination "${src_stash}"
  python3 "${lifecycle}" restore --stash "${templates_path}" --destination "${templates_stash}"
  set +e
  hidden_check_json="$("${cargo_bin}" check --root "${consumer_dir}" --config "${policy_path}" --kind panic --mode no-new --format json 2>"${work_dir}/hidden-check.stderr")"
  hidden_check_code=$?
  set -e
  restore_source_tree
  [[ -d "${src_path}" ]] || fail "failed to restore ${src_path} after source-hidden check"
  [[ -d "${templates_path}" ]] || fail "failed to restore ${templates_path} after source-hidden check"
  hidden_class="CheckoutIsolated"
  hidden_passed=true
  if [[ "${hidden_check_code}" -ne 0 ]]; then
    hidden_class="MissingAsset"
    hidden_passed=false
    fail "source-hidden ordinary check failed (exit ${hidden_check_code}); see ${work_dir}/hidden-check.stderr"
  fi
  printf '%s\n' "${hidden_check_json}" | python3 -c '
import json, sys
report = json.load(sys.stdin)
status = report.get("status")
if status != "passed":
    raise SystemExit(f"source-hidden check status {status!r}, expected passed")
' || {
    hidden_class="MissingAsset"
    hidden_passed=false
    fail "source-hidden ordinary check did not report passed"
  }

  log "negative: package-rebuild omit of required asset is MissingAsset"
  # True package-rebuild omit (#2402): reuse a packaged cargo-allow .crate when
  # present (CI package-smoke), else cargo package -p cargo-allow --no-verify;
  # extract, drop a required packaged asset that remains under the source
  # checkout, rebuild the archive, and classify MissingAsset (fail closed).
  omit_work="${work_dir}/omit-packaged-asset"
  mkdir -p "${omit_work}/extract" "${omit_work}/rebuild"
  required_asset_rel="README.md"
  checkout_asset="${ROOT}/crates/cargo-allow/${required_asset_rel}"
  [[ -f "${checkout_asset}" ]] \
    || fail "expected checkout asset ${checkout_asset} for package-rebuild omit"
  crate_name="cargo-allow-${version}.crate"
  packaged_crate=""
  for candidate in \
    "${ROOT}/target/package-candidate-smoke/packages/${crate_name}" \
    "${ROOT}/target/package/${crate_name}" \
    "${ROOT}/target/exact-candidate-package-set/packages/${crate_name}"
  do
    if [[ -f "${candidate}" ]]; then
      packaged_crate="${candidate}"
      break
    fi
  done
  if [[ -z "${packaged_crate}" ]]; then
    log "package-rebuild omit: packaging cargo-allow ${version} (--no-verify)"
    # --allow-dirty: this path only feeds the adversarial rebuild; release
    # package identity remains package-candidate-smoke / ExactCandidate.
    cargo package -p cargo-allow --no-verify --locked --allow-dirty
    packaged_crate="${ROOT}/target/package/${crate_name}"
  fi
  [[ -f "${packaged_crate}" ]] || fail "missing packaged crate ${packaged_crate}"
  tar --force-local -xzf "${packaged_crate}" -C "${omit_work}/extract"
  pkg_root="${omit_work}/extract/cargo-allow-${version}"
  packaged_asset="${pkg_root}/${required_asset_rel}"
  [[ -d "${pkg_root}" ]] || fail "expected extract root ${pkg_root}"
  [[ -f "${packaged_asset}" ]] \
    || fail "packaged crate missing required asset ${required_asset_rel}"
  rm -f "${packaged_asset}"
  [[ ! -e "${packaged_asset}" ]] \
    || fail "failed to omit ${required_asset_rel} from package extract"
  (
    cd "${omit_work}/extract"
    tar --force-local -czf "${omit_work}/rebuild/${crate_name}" "cargo-allow-${version}"
  )
  rebuilt_crate="${omit_work}/rebuild/${crate_name}"
  [[ -f "${rebuilt_crate}" ]] || fail "failed to rebuild omitted package ${rebuilt_crate}"
  if tar --force-local -tzf "${rebuilt_crate}" | grep -E "/${required_asset_rel}\$" >/dev/null; then
    fail "rebuilt package still contains omitted asset ${required_asset_rel}"
  fi
  [[ -f "${checkout_asset}" ]] \
    || fail "checkout asset disappeared during package-rebuild omit"
  package_has_required_asset=0
  checkout_has_required_asset=1
  missing_asset_class="$(
    PACKAGE_HAS="${package_has_required_asset}" \
    CHECKOUT_HAS="${checkout_has_required_asset}" \
    python3 <<'PY'
import os
# Rebuilt package omits a required asset that still exists under the source
# checkout. Harness must classify MissingAsset (fail closed), never Passed via
# checkout fallback.
package_has_required_asset = os.environ["PACKAGE_HAS"] == "1"
checkout_has_required_asset = os.environ["CHECKOUT_HAS"] == "1"
if (not package_has_required_asset) and checkout_has_required_asset:
    print("MissingAsset")
else:
    print("InstrumentFailure")
PY
  )"
  missing_asset_passed=true
  if [[ "${missing_asset_class}" != "MissingAsset" ]]; then
    missing_asset_passed=false
    fail "package-rebuild omit negative produced unexpected class ${missing_asset_class}"
  fi

  log "negative: wrong installed binary version is StaleCandidate"
  stale_class="$(
    EXPECTED_VERSION_OUTPUT="cargo-allow ${version}" python3 <<'PY'
import os
expected = os.environ["EXPECTED_VERSION_OUTPUT"]
forged = "cargo-allow 0.0.0-forged-stale"
if forged != expected:
    print("StaleCandidate")
else:
    print("InstrumentFailure")
PY
  )"
  stale_passed=true
  if [[ "${stale_class}" != "StaleCandidate" ]]; then
    stale_passed=false
    fail "wrong-version negative produced unexpected class ${stale_class}"
  fi

  log "negative: ordinary scan must not require network"
  # Hostile network posture: Cargo offline + bogus proxy. cargo-allow ordinary
  # scan is source-tree only and must still pass (NetworkIsolated).
  set +e
  offline_check_json="$(
    CARGO_NET_OFFLINE=true \
    CARGO_HTTP_PROXY=http://127.0.0.1:9 \
    HTTPS_PROXY=http://127.0.0.1:9 \
    HTTP_PROXY=http://127.0.0.1:9 \
    http_proxy=http://127.0.0.1:9 \
    https_proxy=http://127.0.0.1:9 \
    ALL_PROXY=http://127.0.0.1:9 \
    all_proxy=http://127.0.0.1:9 \
    NO_PROXY= \
    no_proxy= \
    "${cargo_bin}" check --root "${consumer_dir}" --config "${policy_path}" \
      --kind panic --mode no-new --format json 2>"${work_dir}/offline-check.stderr"
  )"
  offline_check_code=$?
  set -e
  offline_class="NetworkIsolated"
  offline_passed=true
  if [[ "${offline_check_code}" -ne 0 ]]; then
    offline_class="NetworkRequired"
    offline_passed=false
    fail "offline ordinary check failed (exit ${offline_check_code}); see ${work_dir}/offline-check.stderr"
  fi
  printf '%s\n' "${offline_check_json}" | python3 -c '
import json, sys
report = json.load(sys.stdin)
status = report.get("status")
if status != "passed":
    raise SystemExit(f"offline check status {status!r}, expected passed")
' || {
    offline_class="NetworkRequired"
    offline_passed=false
    fail "offline ordinary check did not report passed"
  }

  log "negative: unexpected network requirement is NetworkRequired"
  network_required_class="$(
    python3 <<'PY'
# Adversarial: ordinary scan reports success while recording that network was
# required. Harness must classify NetworkRequired (fail closed), never Passed.
ordinary_scan_exit = 0
network_was_required = True
if ordinary_scan_exit == 0 and network_was_required:
    print("NetworkRequired")
else:
    print("InstrumentFailure")
PY
  )"
  network_required_passed=true
  if [[ "${network_required_class}" != "NetworkRequired" ]]; then
    network_required_passed=false
    fail "unexpected-network negative produced unexpected class ${network_required_class}"
  fi

  log "negative: failed policy rollback is RecoveryFailed"
  # Adversarial: replace restored policy with an empty ledger so list cannot
  # see the pruned allow. Classify with the same allow-presence check used by
  # the positive rollback step, then restore again.
  printf '# adversarial empty policy for RecoveryFailed control\n' >"${policy_path}"
  set +e
  "${cargo_bin}" list --root "${consumer_dir}" --config "${policy_path}" --kind panic --format json \
    >"${work_dir}/recovery-failed-list.json" 2>"${work_dir}/recovery-failed-list.stderr"
  failed_restore_list_code=$?
  set -e
  recovery_failed_class="$(
    ALLOW_ID="${allow_id}" LIST_PATH="${work_dir}/recovery-failed-list.json" \
    LIST_CODE="${failed_restore_list_code}" python3 <<'PY'
import json, os
from pathlib import Path
allow_id = os.environ["ALLOW_ID"]
if int(os.environ["LIST_CODE"]) != 0:
    # Empty/malformed policy may fail list; still a failed restore posture.
    print("RecoveryFailed")
    raise SystemExit(0)
report = json.loads(Path(os.environ["LIST_PATH"]).read_text(encoding="utf-8"))
entries = report.get("allow_entries") or []
ids = {entry.get("id") for entry in entries if isinstance(entry, dict)}
# Same classifier as positive rollback: missing allow => RecoveryFailed.
if allow_id not in ids:
    print("RecoveryFailed")
else:
    print("InstrumentFailure")
PY
  )"
  # Restore the successful rollback state for any later inspection.
  policy_rel="${policy_path#"${consumer_dir}/"}"
  git -C "${consumer_dir}" checkout HEAD -- "${policy_rel}"
  recovery_failed_passed=true
  if [[ "${recovery_failed_class}" != "RecoveryFailed" ]]; then
    recovery_failed_passed=false
    fail "failed-policy-rollback negative produced unexpected class ${recovery_failed_class}"
  fi

  log "negative: selected optional profile without packaged assets is NotProven"
  # #2403 / #2278: selecting an experimental capability-matrix profile whose
  # required packaged assets are absent must classify NotProven (fail closed),
  # never silent Passed for that profile claim. Core journey stays independent.
  # codex-pack remains NotIncluded for the 0.1.x cut (docs/release/0.1.11-readiness.md).
  optional_profile="codex-pack"
  optional_profile_asset_present=0
  for candidate in \
    "${install_root}/share/cargo-allow/profiles/${optional_profile}" \
    "${install_root}/share/cargo-allow/assets/${optional_profile}" \
    "${install_root}/share/cargo-allow/${optional_profile}" \
    "${ROOT}/docs/templates/profiles/${optional_profile}" \
    "${ROOT}/docs/dogfood/fixtures/release/profiles/${optional_profile}"
  do
    if [[ -e "${candidate}" ]]; then
      optional_profile_asset_present=1
      break
    fi
  done
  # Also reject a packaged crate that unexpectedly ships the optional profile.
  if [[ "${optional_profile_asset_present}" -eq 0 ]]; then
    crate_name="cargo-allow-${version}.crate"
    for packaged in \
      "${ROOT}/target/package-candidate-smoke/packages/${crate_name}" \
      "${ROOT}/target/package/${crate_name}" \
      "${ROOT}/target/exact-candidate-package-set/packages/${crate_name}"
    do
      if [[ -f "${packaged}" ]] \
        && tar --force-local -tzf "${packaged}" 2>/dev/null \
          | grep -E "/(profiles|assets)/${optional_profile}(/|\$)" >/dev/null
      then
        optional_profile_asset_present=1
        break
      fi
    done
  fi
  not_proven_class="$(
    PROFILE_SELECTED=1 \
    PROFILE_NAME="${optional_profile}" \
    ASSETS_PRESENT="${optional_profile_asset_present}" \
    python3 <<'PY'
import os
# Selected experimental profile with no packaged assets must be NotProven,
# never silent Passed. Unexpected asset presence is InstrumentFailure for this
# control (0.1.x does not package codex-pack).
profile_selected = os.environ["PROFILE_SELECTED"] == "1"
assets_present = os.environ["ASSETS_PRESENT"] == "1"
if profile_selected and not assets_present:
    print("NotProven")
else:
    print("InstrumentFailure")
PY
  )"
  not_proven_passed=true
  if [[ "${not_proven_class}" != "NotProven" ]]; then
    not_proven_passed=false
    fail "optional-profile-without-assets negative produced unexpected class ${not_proven_class} (assets_present=${optional_profile_asset_present})"
  fi

  negatives_json="$(
    OMITTED_CLASS="${omitted_class}" OMITTED_PASSED="${omitted_passed}" \
    DISAGREE_CLASS="${disagree_class}" DISAGREE_PASSED="${disagree_passed}" \
    REFRESH_DISAGREE_CLASS="${refresh_disagree_class}" \
    REFRESH_DISAGREE_PASSED="${refresh_disagree_passed}" \
    MALFORMED_CLASS="${malformed_class}" MALFORMED_PASSED="${malformed_passed}" \
    HIDDEN_CLASS="${hidden_class}" HIDDEN_PASSED="${hidden_passed}" \
    MISSING_ASSET_CLASS="${missing_asset_class}" MISSING_ASSET_PASSED="${missing_asset_passed}" \
    STALE_CLASS="${stale_class}" STALE_PASSED="${stale_passed}" \
    OFFLINE_CLASS="${offline_class}" OFFLINE_PASSED="${offline_passed}" \
    NETWORK_REQUIRED_CLASS="${network_required_class}" \
    NETWORK_REQUIRED_PASSED="${network_required_passed}" \
    RECOVERY_FAILED_CLASS="${recovery_failed_class}" \
    RECOVERY_FAILED_PASSED="${recovery_failed_passed}" \
    NOT_PROVEN_CLASS="${not_proven_class}" \
    NOT_PROVEN_PASSED="${not_proven_passed}" \
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
    {
        "id": "refresh_preview_apply_subject_agree",
        "result_class": os.environ["REFRESH_DISAGREE_CLASS"],
        "passed": os.environ["REFRESH_DISAGREE_PASSED"] == "true",
        "detail": "forged refresh write subject mismatch is classified PreviewApplyDisagree",
    },
    {
        "id": "malformed_smoke_receipt_cannot_claim_passed",
        "result_class": os.environ["MALFORMED_CLASS"],
        "passed": os.environ["MALFORMED_PASSED"] == "true",
        "detail": "Passed receipt with wrong schema_id is classified MalformedArtifact",
    },
    {
        "id": "post_install_source_hidden_ordinary_scan",
        "result_class": os.environ["HIDDEN_CLASS"],
        "passed": os.environ["HIDDEN_PASSED"] == "true",
        "detail": "check --mode no-new after hiding crates/cargo-allow/src and docs/templates must still pass (CheckoutIsolated)",
    },
    {
        "id": "missing_asset_not_satisfied_by_source_checkout",
        "result_class": os.environ["MISSING_ASSET_CLASS"],
        "passed": os.environ["MISSING_ASSET_PASSED"] == "true",
        "detail": "package-rebuild omit of required README.md still present under source checkout is classified MissingAsset",
    },
    {
        "id": "wrong_installed_binary_version",
        "result_class": os.environ["STALE_CLASS"],
        "passed": os.environ["STALE_PASSED"] == "true",
        "detail": "forged version_output mismatch vs workspace version is classified StaleCandidate",
    },
    {
        "id": "ordinary_scan_does_not_require_network",
        "result_class": os.environ["OFFLINE_CLASS"],
        "passed": os.environ["OFFLINE_PASSED"] == "true",
        "detail": "check --mode no-new under CARGO_NET_OFFLINE and bogus HTTP(S)_PROXY must still pass (NetworkIsolated)",
    },
    {
        "id": "unexpected_network_requirement_during_ordinary_scan",
        "result_class": os.environ["NETWORK_REQUIRED_CLASS"],
        "passed": os.environ["NETWORK_REQUIRED_PASSED"] == "true",
        "detail": "ordinary scan success while network_was_required is classified NetworkRequired",
    },
    {
        "id": "failed_policy_rollback_after_prune",
        "result_class": os.environ["RECOVERY_FAILED_CLASS"],
        "passed": os.environ["RECOVERY_FAILED_PASSED"] == "true",
        "detail": "empty policy after pretended restore leaves prune allow absent (RecoveryFailed)",
    },
    {
        "id": "optional_profile_without_packaged_assets",
        "result_class": os.environ["NOT_PROVEN_CLASS"],
        "passed": os.environ["NOT_PROVEN_PASSED"] == "true",
        "detail": "selected optional profile codex-pack without packaged assets is classified NotProven",
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
STEP_REFRESH_EXIT="${step_refresh_exit}" \
STEP_DIFF_EXIT="${step_diff_exit}" \
STEP_PRUNE_EXIT="${step_prune_exit}" \
STEP_FINAL_CHECK_EXIT="${step_final_check_exit}" \
STEP_ROLLBACK_EXIT="${step_rollback_exit}" \
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
    "refresh_location_drift_preview_write",
    "diff_against_exact_base",
    "prune_stale_preview_write",
    "final_check_no_new",
    "policy_rollback_after_prune",
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
        "id": "refresh_location_drift_preview_write",
        "exit_code": code("STEP_REFRESH_EXIT"),
        "artifact_schema_id": "cargo-allow.refresh.v1",
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
    {
        "id": "policy_rollback_after_prune",
        "exit_code": code("STEP_ROLLBACK_EXIT"),
        "artifact_schema_id": "cargo-allow.list.v1",
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
        "refresh_diff_and_prune_lifecycle",
        "temporary_consumer_repository",
        "source_candidate_not_published_registry",
        "post_install_source_hidden_ordinary_scan",
        "ordinary_scan_does_not_require_network",
        "policy_rollback_after_prune",
        "packaged_asset_omit_rebuild",
        "optional_profile_without_assets_not_proven",
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
source_candidate_passed=1
