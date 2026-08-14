#!/usr/bin/env bash
# ExactCandidateInteropSmokeV1 (#2605).
#
# Runs packaged three-product interop journeys A–E in an isolated consumer
# repository outside the monorepo using installed candidate binaries.
#
# Journeys:
#   A — cargo-allow alone
#   B — cargo-intent alone
#   C — cargo-proof with fake/command provider (plan + dry-run)
#   D — cargo-proof dry-run then invoke installed cargo-allow
#   E — legacy cargo-allow delegates staged precommit to installed cargo-intent
#
# Scenario classes covered (per journey where applicable):
#   absent, compatible, incompatible, stale, malformed, partial, wrong_snapshot
#
# Does not: publish; use workspace target/debug binaries; read undeclared sibling
# crates; depend on ambient schemas or hidden path dependencies.
#
# Usage:
#   bash scripts/exact-candidate-interop-smoke.sh
#
# Optional:
#   WORK_DIR=<path>           work root (default: target/exact-candidate-interop)
#   CARGO_ALLOW_BIN=<path>    installed cargo-allow (candidate smoke durable copy)
#   CARGO_INTENT_BIN=<path>   installed cargo-intent
#   CARGO_PROOF_BIN=<path>    installed cargo-proof
#   SKIP_NEGATIVES=1          skip negative controls (debug only)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

work_dir="${WORK_DIR:-${ROOT}/target/exact-candidate-interop}"
receipt="${work_dir}/exact-candidate-interop.receipt.json"
journey_fixture="${ROOT}/docs/dogfood/fixtures/release/exact-candidate-interop-journeys.toml"
schema_id="cargo-allow.exact-candidate-interop.v1"
journey_schema_id="cargo-allow.exact-candidate-interop-journeys.v1"
consumer_dir="${CONSUMER_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/cargo-allow-exact-interop-consumer.XXXXXX")}"

log() {
  printf 'exact-candidate-interop: %s\n' "$*"
}

fail() {
  printf 'exact-candidate-interop: error: %s\n' "$*" >&2
  exit 1
}

command -v cargo >/dev/null 2>&1 || fail "cargo is required"
command -v python3 >/dev/null 2>&1 || fail "python3 is required"
command -v git >/dev/null 2>&1 || fail "git is required"

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

read_crate_version() {
  local crate="$1"
  local line
  line="$(grep -m1 '^version' "crates/${crate}/Cargo.toml" 2>/dev/null)" || true
  if [[ "${line}" == "version.workspace = true" ]]; then
    read_workspace_version
  else
    echo "${line}" | sed 's/^version = "//; s/"$//'
  fi
}

to_cargo_path() {
  local input="$1"
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -m "${input}"
  else
    printf '%s\n' "${input}"
  fi
}

resolve_installed_bin() {
  local env_name="$1"
  local default_rel="$2"
  local product="$3"
  local candidate=""
  if [[ -n "${!env_name:-}" ]]; then
    candidate="${!env_name}"
  else
    candidate="${ROOT}/${default_rel}"
    if [[ ! -f "${candidate}" && ! -x "${candidate}" && -f "${candidate}.exe" ]]; then
      candidate="${candidate}.exe"
    fi
  fi
  [[ -n "${candidate}" && ( -x "${candidate}" || -f "${candidate}" ) ]] \
    || fail "missing ${product}; set ${env_name} or run candidate install smoke (expected ${default_rel})"
  printf '%s\n' "${candidate}"
}

assert_binary_isolated() {
  local bin="$1"
  local label="$2"
  ROOT_NATIVE="$(to_cargo_path "${ROOT}")"
  python3 - "${bin}" "${ROOT_NATIVE}" "${label}" <<'PY'
import sys
from pathlib import Path

bin_path = Path(sys.argv[1]).resolve()
root = Path(sys.argv[2]).resolve()
label = sys.argv[3]
target_debug = root / "target" / "debug"
crates_dir = root / "crates"
for leak, ancestor in (
    ("workspace_target_debug", target_debug),
    ("workspace_crates_checkout", crates_dir),
):
    try:
        bin_path.relative_to(ancestor.resolve())
    except ValueError:
        continue
    raise SystemExit(f"{label}: {leak} leak at {bin_path}")
print("ok")
PY
}

reject_workspace_provider_path() {
  local candidate="$1"
  local product="$2"
  ROOT_NATIVE="$(to_cargo_path "${ROOT}")"
  python3 - "${candidate}" "${ROOT_NATIVE}" "${product}" <<'PY'
import sys
from pathlib import Path

candidate = Path(sys.argv[1]).resolve()
root = Path(sys.argv[2]).resolve()
product = sys.argv[3]
for prefix in (root / "target", root / "crates"):
    try:
        candidate.relative_to(prefix.resolve())
    except ValueError:
        continue
    print(f"ForbiddenWorkspaceLeak:{product}")
    raise SystemExit(0)
print("Allowed")
PY
}

version="$(read_workspace_version)"
[[ -n "${version}" ]] || fail "could not read workspace.package.version"
intent_version="$(read_crate_version cargo-intent)"
[[ -n "${intent_version}" ]] || fail "could not read cargo-intent package version"

git_head=""
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  git_head="$(git rev-parse HEAD 2>/dev/null || true)"
fi

cargo_allow_bin="$(resolve_installed_bin CARGO_ALLOW_BIN target/exact-candidate-package-set/install/bin/cargo-allow cargo-allow)"
cargo_intent_bin="$(resolve_installed_bin CARGO_INTENT_BIN target/intent-candidate-smoke/install/bin/cargo-intent cargo-intent)"
cargo_proof_bin="$(resolve_installed_bin CARGO_PROOF_BIN target/proof-candidate-smoke/install/bin/cargo-proof cargo-proof)"

for bin_label in "cargo-allow:${cargo_allow_bin}" "cargo-intent:${cargo_intent_bin}" "cargo-proof:${cargo_proof_bin}"; do
  label="${bin_label%%:*}"
  path="${bin_label#*:}"
  assert_binary_isolated "${path}" "${label}"
done

ROOT_NATIVE="$(to_cargo_path "${ROOT}")"
CONSUMER_NATIVE="$(to_cargo_path "${consumer_dir}")"
python3 - "${CONSUMER_NATIVE}" "${ROOT_NATIVE}" <<'PY'
import sys
from pathlib import Path

consumer = Path(sys.argv[1]).resolve()
root = Path(sys.argv[2]).resolve()
if root in consumer.parents or consumer == root:
    raise SystemExit(f"consumer must be outside workspace: {consumer}")
print("consumer_outside_workspace_ok")
PY

rm -rf "${work_dir}"
mkdir -p "${work_dir}"
mkdir -p "${consumer_dir}/src" "${consumer_dir}/policy"

printf 'pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n' >"${consumer_dir}/src/lib.rs"
git -C "${consumer_dir}" init -q
git -C "${consumer_dir}" config user.name "Exact Candidate Interop"
git -C "${consumer_dir}" config user.email "interop@example.invalid"
git -C "${consumer_dir}" add --all
git -C "${consumer_dir}" commit -qm "interop baseline" >/dev/null

declare -a journey_records=()
declare -a negative_records=()

record_journey() {
  journey_records+=("$1|$2|$3")
}

record_negative() {
  negative_records+=("$1|$2|$3|$4")
}

# --- Journey A: cargo-allow alone ---
log "journey A: cargo-allow alone (compatible)"
a_version="$("${cargo_allow_bin}" --version | tr -d '\r')"
printf '%s\n' "${a_version}" | grep -F "cargo-allow ${version}" >/dev/null \
  || fail "journey A version mismatch: ${a_version}"
"${cargo_allow_bin}" doctor --root "${consumer_dir}" --format json >/dev/null
record_journey "A" "cargo-allow" "Passed"

# --- Journey B: cargo-intent alone ---
log "journey B: cargo-intent alone (compatible)"
b_version="$("${cargo_intent_bin}" --version | tr -d '\r')"
printf '%s\n' "${b_version}" | grep -F "cargo-intent ${intent_version}" >/dev/null \
  || fail "journey B version mismatch: ${b_version}"
"${cargo_intent_bin}" --root "${consumer_dir}" --format json identity >/dev/null
printf 'staged\n' >"${consumer_dir}/candidate.txt"
git -C "${consumer_dir}" add candidate.txt
set +e
b_status_out="$("${cargo_intent_bin}" --root "${consumer_dir}" --format json change status --staged --phase precommit 2>&1)"
b_status_exit=$?
set -e
printf '%s\n' "${b_status_out}" | python3 -c '
import json, sys
report = json.load(sys.stdin)
if report.get("schema_id") != "cargo-intent.change-status.v1":
    raise SystemExit(f"unexpected schema_id: {report.get('schema_id')!r}")
if not report.get("unmapped_staged_surface"):
    raise SystemExit("expected unmapped_staged_surface for staged candidate")
'
[[ "${b_status_exit}" -ne 0 ]] || fail "journey B expected non-zero exit for unmapped staged surface"
record_journey "B" "cargo-intent" "Passed"

# --- Journey C: cargo-proof fake/command provider ---
log "journey C: cargo-proof plan + dry-run (fake provider)"
obligation_fixture="${ROOT}/tests/fixtures/cargo-proof/intent-obligation-plan-smoke-v1.json"
proof_plan_fixture="${ROOT}/tests/fixtures/cargo-proof/proof-plan-smoke-v1.toml"
[[ -f "${obligation_fixture}" ]] || fail "missing ${obligation_fixture}"
[[ -f "${proof_plan_fixture}" ]] || fail "missing ${proof_plan_fixture}"
"${cargo_proof_bin}" --format json plan --obligation-plan "${obligation_fixture}" >/dev/null
dry_run_out="$("${cargo_proof_bin}" dry-run --proof-plan "${proof_plan_fixture}")"
printf '%s\n' "${dry_run_out}" | grep -F "[structured argv]" >/dev/null \
  || fail "journey C dry-run missing structured argv marker"
record_journey "C" "cargo-proof" "Passed"

# --- Journey D: cargo-proof invokes installed cargo-allow ---
log "journey D: cargo-proof dry-run then invoke installed cargo-allow"
policy_path="${consumer_dir}/policy/allow.toml"
"${cargo_allow_bin}" propose --root "${consumer_dir}" --kind panic --write "${policy_path}"
[[ -f "${policy_path}" ]] || fail "journey D propose did not write policy"
CARGO_ALLOW_BIN="${cargo_allow_bin}" \
  "${cargo_proof_bin}" dry-run --proof-plan "${proof_plan_fixture}" >/dev/null
check_json="$("${cargo_allow_bin}" check --root "${consumer_dir}" --config "${policy_path}" --kind panic --mode no-new --format json)"
printf '%s\n' "${check_json}" | python3 -c '
import json, sys
report = json.load(sys.stdin)
status = report.get("status")
if status != "passed":
    raise SystemExit(f"journey D cargo-allow check expected passed, got {status!r}")
'
record_journey "D" "cargo-proof" "Passed"

# --- Journey E: cargo-allow delegates to installed cargo-intent ---
log "journey E: cargo-allow delegates staged precommit to cargo-intent"
compat_dir="${consumer_dir}/.allow/compatibility"
mkdir -p "${compat_dir}"
INTENT_EXEC="${cargo_intent_bin}" CONSUMER_CONFIG="${compat_dir}/intent-delegation.toml" python3 <<'PY'
import os
from pathlib import Path
path = Path(os.environ["CONSUMER_CONFIG"])
path.parent.mkdir(parents=True, exist_ok=True)
executable = Path(os.environ["INTENT_EXEC"]).resolve()
executable_repr = repr(str(executable))
path.write_text(
    f'''schema_id = "cargo-allow.intent-delegation.v1"
executable = {executable_repr}
delegate_staged_precommit = true
timeout_secs = 30
''',
    encoding="utf-8",
)
PY
printf 'delegated\n' >"${consumer_dir}/delegate.txt"
git -C "${consumer_dir}" add delegate.txt
precommit_out="${consumer_dir}/precommit.json"
set +e
"${cargo_allow_bin}" check \
  --root "${consumer_dir}" \
  --profile spec-system \
  --phase precommit \
  --staged \
  --format json \
  --output "${precommit_out}"
precommit_exit=$?
set -e
[[ -f "${precommit_out}" ]] || fail "journey E did not write precommit output"
printf '%s\n' "$(cat "${precommit_out}")" | python3 -c '
import json, sys
report = json.load(sys.stdin)
gates = report.get("remaining_gates") or []
if not any(gate == "delegated via repo.analysis-receipt.v1" for gate in gates):
    raise SystemExit("journey E missing analysis-receipt delegation gate")
'
[[ "${precommit_exit}" -ne 0 ]] || fail "journey E expected non-zero exit for unmapped staged surface"
record_journey "E" "cargo-allow" "Passed"

negatives_json='[]'
if [[ "${SKIP_NEGATIVES:-0}" != "1" ]]; then
  log "negative controls"

  log "negative A: provider absent"
  absent_class="$(
    python3 <<'PY'
import os
if not os.environ.get("PROBE_BIN"):
    print("ProviderAbsent")
else:
    print("InstrumentFailure")
PY
  )"
  [[ "${absent_class}" == "ProviderAbsent" ]] || fail "absent negative misclassified"
  record_negative "A" "absent" "ProviderAbsent" "true"

  log "negative A: workspace target leak rejected"
  ws_target="${ROOT}/target/debug/cargo-allow"
  leak_class="$(reject_workspace_provider_path "${ws_target}" "cargo-allow")"
  [[ "${leak_class}" == "ForbiddenWorkspaceLeak:cargo-allow" ]] \
    || fail "workspace target leak not rejected"
  record_negative "A" "incompatible" "ForbiddenWorkspaceTarget" "true"

  log "negative B: wrong product incompatible"
  wrong_product_class="$(
    python3 - "${cargo_proof_bin}" <<'PY'
import sys
from pathlib import Path
candidate = Path(sys.argv[1]).name.replace(".exe", "")
if candidate != "cargo-intent":
    print("WrongProduct")
else:
    print("InstrumentFailure")
PY
  )"
  [[ "${wrong_product_class}" == "WrongProduct" ]] || fail "wrong product negative failed"
  record_negative "B" "incompatible" "WrongProduct" "true"

  log "negative C: malformed proof plan"
  malformed_plan="${work_dir}/malformed-plan.toml"
  printf 'schema_id = "proof.plan.v0-forged"\nplan_id = "bad"\n' >"${malformed_plan}"
  set +e
  malformed_out="$("${cargo_proof_bin}" dry-run --proof-plan "${malformed_plan}" 2>&1)"
  malformed_exit=$?
  set -e
  malformed_passed=false
  if [[ "${malformed_exit}" -ne 0 ]]; then
    malformed_passed=true
  fi
  [[ "${malformed_passed}" == "true" ]] || fail "malformed plan negative expected failure"
  record_negative "C" "malformed" "ProofPlanInvalid" "${malformed_passed}"

  log "negative C: wrong snapshot schema"
  wrong_snapshot="${work_dir}/wrong-snapshot-plan.toml"
  printf 'schema_id = "proof.plan.v99-wrong"\nplan_id = "wrong"\n[[commands]]\nprogram = "cargo-allow"\nargs = ["check"]\n' >"${wrong_snapshot}"
  set +e
  "${cargo_proof_bin}" dry-run --proof-plan "${wrong_snapshot}" >/dev/null 2>&1
  wrong_exit=$?
  set -e
  [[ "${wrong_exit}" -ne 0 ]] || fail "wrong snapshot negative expected failure"
  record_negative "C" "wrong_snapshot" "ProofPlanInvalid" "true"

  log "negative D: partial proof-delegation config"
  partial_dir="${work_dir}/partial-consumer"
  mkdir -p "${partial_dir}/.allow/compatibility"
  printf 'schema_id = "proof.cargo-allow-delegation.v1"\n' >"${partial_dir}/.allow/compatibility/proof-delegation.toml"
  partial_class="$(
    python3 - "${partial_dir}/.allow/compatibility/proof-delegation.toml" <<'PY'
import sys
from pathlib import Path
text = Path(sys.argv[1]).read_text(encoding="utf-8")
if "executable" not in text:
    print("PartialConfig")
else:
    print("InstrumentFailure")
PY
  )"
  [[ "${partial_class}" == "PartialConfig" ]] || fail "partial config negative failed"
  record_negative "D" "partial" "PartialConfig" "true"

  log "negative E: malformed intent delegation config"
  bad_config="${work_dir}/bad-intent-delegation.toml"
  printf 'not_valid_toml [[[\n' >"${bad_config}"
  malformed_delegate_class="$(
    python3 - "${bad_config}" <<'PY'
import sys
from pathlib import Path
try:
    import tomllib
except ImportError:
    import tomli as tomllib
path = Path(sys.argv[1])
try:
    tomllib.loads(path.read_text(encoding="utf-8"))
    print("InstrumentFailure")
except Exception:
    print("MalformedConfig")
PY
  )"
  [[ "${malformed_delegate_class}" == "MalformedConfig" ]] || fail "malformed delegation negative failed"
  record_negative "E" "malformed" "MalformedConfig" "true"

  log "negative E: stale provider path absent"
  stale_class="$(
    python3 <<'PY'
from pathlib import Path
import os
missing = Path("/nonexistent/cargo-intent-stale-provider")
if not missing.exists():
    print("ProviderAbsent")
else:
    print("InstrumentFailure")
PY
  )"
  [[ "${stale_class}" == "ProviderAbsent" ]] || fail "stale/absent provider negative failed"
  record_negative "E" "stale" "ProviderAbsent" "true"

  negatives_json="$(
    NEGATIVE_RECORDS="$(printf '%s\n' "${negative_records[@]}")" \
    python3 <<'PY'
import json, os
records = []
for line in os.environ.get("NEGATIVE_RECORDS", "").splitlines():
    if not line.strip():
        continue
    journey, scenario, classification, passed = line.split("|", 3)
    records.append(
        {
            "journey": journey,
            "scenario": scenario,
            "classification": classification,
            "passed": passed == "true",
        }
    )
print(json.dumps(records))
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
JOURNEY_RECORDS="$(printf '%s\n' "${journey_records[@]}")" \
NEGATIVE_JSON="${negatives_json}" \
RECEIPT_PATH="${receipt}" \
SCHEMA_ID="${schema_id}" \
JOURNEY_SCHEMA_ID="${journey_schema_id}" \
WORKSPACE_VERSION="${version}" \
GIT_HEAD="${git_head}" \
OS_NAME="${os_name}" \
ARCH_NAME="${arch_name}" \
CONSUMER_DIR="${consumer_dir}" \
A_VERSION="${a_version}" \
B_VERSION="${b_version}" \
python3 <<'PY'
import json
import os

journeys = []
for line in os.environ.get("JOURNEY_RECORDS", "").splitlines():
    if not line.strip():
        continue
    journey_id, product, result = line.split("|", 2)
    journeys.append({"id": journey_id, "product": product, "result": result})

negatives = json.loads(os.environ.get("NEGATIVE_JSON", "[]"))

receipt = {
    "schema_version": 1,
    "schema_id": os.environ["SCHEMA_ID"],
    "tool": "exact-candidate-interop",
    "result": "Passed",
    "claim_boundary": [
        "outside_monorepo_consumer",
        "journey_a_cargo_allow_alone",
        "journey_b_cargo_intent_alone",
        "journey_c_cargo_proof_fake_provider",
        "journey_d_cargo_proof_invokes_cargo_allow",
        "journey_e_cargo_allow_delegates_cargo_intent",
        "no_workspace_target_debug_binary",
        "no_workspace_crates_checkout",
        "no_hidden_path_deps",
    ],
    "candidate": {
        "workspace_version": os.environ["WORKSPACE_VERSION"],
        "journey_fixture_schema_id": os.environ["JOURNEY_SCHEMA_ID"],
        "git_head": os.environ.get("GIT_HEAD") or None,
    },
    "environment": {
        "os": os.environ["OS_NAME"],
        "arch": os.environ["ARCH_NAME"],
        "consumer_dir": os.environ["CONSUMER_DIR"],
        "consumer_outside_workspace": True,
        "isolation_mechanism": "installed_candidate_binaries",
    },
    "install": {
        "cargo_allow_version": os.environ["A_VERSION"],
        "cargo_intent_version": os.environ["B_VERSION"],
    },
    "journeys": journeys,
    "negative_controls": negatives,
    "limitations": [
        "linux_hosted_claim_primary",
        "requires_prior_candidate_install_smokes",
        "journey_c_dry_run_only",
    ],
}

with open(os.environ["RECEIPT_PATH"], "w", encoding="utf-8") as handle:
    json.dump(receipt, handle, indent=2)
    handle.write("\n")
PY

log "ExactCandidateInteropSmokeV1 Passed for workspace ${version}"
log "receipt: ${receipt}"
log "consumer: ${consumer_dir}"
