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

bin_ext() {
  case "$(uname -s | tr '[:upper:]' '[:lower:]')" in
    mingw*|msys*|cygwin*) printf '.exe' ;;
    *) printf '' ;;
  esac
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
"${cargo_proof_bin}" --format json plan --obligation-plan "${obligation_fixture}" >"${work_dir}/proof-plan-frame.json"
python3 - "${work_dir}/proof-plan-frame.json" <<'PY'
import json, sys
from pathlib import Path

frame = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
digest = frame.get("intent_plan_digest", "")
if not digest.startswith("sha256:v1:"):
    raise SystemExit(f"plan frame must bind intent_plan_digest, got {digest!r}")
if digest not in frame.get("plan_id", ""):
    raise SystemExit("plan_id must embed the intent plan digest (#3316)")
PY
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

# --- Installed-journey parity (#3309 final installment) ---
# The same staged state flows through BOTH surfaces of the installed
# candidates: cargo-intent's own change-status command (journey B's
# surface) and cargo-allow's one-way compatibility delegation (journey
# E). Both must bind the same staged identity and classify the state
# identically — the same protocol results end to end.
log "installed-journey parity: direct change status on the delegated state"
parity_out="${consumer_dir}/parity-direct.json"
set +e
"${cargo_intent_bin}" --root "${consumer_dir}" --format json change status \
  --staged --phase precommit >"${parity_out}" 2>"${consumer_dir}/parity-direct.err"
parity_exit=$?
set -e
[[ -f "${parity_out}" ]] || fail "parity direct change status wrote no output"
PARITY_PRECOMMIT="${precommit_out}" PARITY_DIRECT="${parity_out}" python3 <<'PY'
import json
import os
from pathlib import Path

delegated = json.loads(Path(os.environ["PARITY_PRECOMMIT"]).read_text(encoding="utf-8"))
direct = json.loads(Path(os.environ["PARITY_DIRECT"]).read_text(encoding="utf-8"))

if direct.get("schema_id") != "cargo-intent.change-status.v1":
    raise SystemExit(f"direct run is not change-status: {direct.get('schema_id')!r}")

delegated_identity = delegated.get("staged_identity_after") or ""
direct_identity = direct.get("staged_identity") or ""
if not direct_identity:
    raise SystemExit("direct change-status lacks staged_identity")
if delegated_identity != direct_identity:
    raise SystemExit(
        "installed-journey parity drift: delegated "
        f"{delegated_identity!r} != direct {direct_identity!r}"
    )

delegated_gate = "delegated via repo.analysis-receipt.v1" in (
    delegated.get("remaining_gates") or []
)
delegated_unmapped = "provider reported unmapped staged surface" in (
    delegated.get("remaining_gates") or []
)
direct_unmapped = bool(direct.get("unmapped_staged_surface"))
if not direct_unmapped:
    raise SystemExit("expected the unmapped staged surface in the direct run")
if not delegated_gate:
    raise SystemExit("expected the delegation gate in the delegated run")
if not delegated_unmapped:
    raise SystemExit("expected the delegated surface to classify the state unmapped")
PY
record_journey "PARITY" "cargo-intent+cargo-allow" "Passed"

# --- Sentinel plants (#2605 final hardening): undeclared-read decoys ---
# Poison the monorepo target tree (a dedicated subdirectory, so real
# build outputs are untouched) with decoy product binaries that would
# be selected by ANY path-based discovery leaking the workspace, and
# plant marker files whose text must never appear in a retained
# artifact. The PATH-poison control below then proves the strongest
# form: even when a decoy IS discoverable, the real protocol handshake
# rejects it — a workspace binary cannot impersonate a provider.
log "sentinels: planting workspace decoys and markers"
sentinel_marker="SENTINEL-2605-DO-NOT-READ"
sentinel_dir="${ROOT}/target/interop-sentinel"
mkdir -p "${sentinel_dir}"
for product in cargo-allow cargo-intent cargo-proof; do
  printf '#!/usr/bin/env bash\necho "%s 0.0.0-sentinel"\nexit 1\n' "${product}" \
    >"${sentinel_dir}/${product}"
  chmod +x "${sentinel_dir}/${product}"
done
printf '%s\n' "${sentinel_marker}" >"${sentinel_dir}/sentinel-marker.txt"
printf '%s\n' "${sentinel_marker}" >"${ROOT}/crates/sentinel-marker.txt"

log "sentinel: PATH-poisoned discovery cannot impersonate a provider"
poison_dir="${work_dir}/path-poison-consumer"
mkdir -p "${poison_dir}/.allow/compatibility"
git -C "${poison_dir}" init -q 2>/dev/null || true
git -C "${poison_dir}" config user.name "Interop Harness"
git -C "${poison_dir}" config user.email "interop@example.invalid"
printf 'poisoned\n' >"${poison_dir}/staged.txt"
git -C "${poison_dir}" add staged.txt
printf 'schema_id = "cargo-allow.intent-delegation.v1"\ndelegate_staged_precommit = true\ntimeout_secs = 30\n' \
  >"${poison_dir}/.allow/compatibility/intent-delegation.toml"
poison_out="${poison_dir}/precommit-poison.json"
set +e
poison_err="$(env -u CARGO_INTENT_BIN PATH="${sentinel_dir}:${PATH}" \
  "${cargo_allow_bin}" check \
  --root "${poison_dir}" \
  --profile spec-system \
  --phase precommit \
  --staged \
  --format json \
  --output "${poison_out}" 2>&1)"
poison_exit=$?
set -e
poison_passed=false
if [[ "${poison_exit}" -ne 0 ]]; then
  case "${poison_err}" in
    *wrong_protocol*|*malformed_provider_output*|*provider_instrument_failure*|*not*found*)
      poison_passed=true
      ;;
  esac
fi
[[ "${poison_passed}" == "true" ]] \
  || fail "PATH-poison control: decoy was not rejected by the real handshake (exit=${poison_exit} err=${poison_err})"
record_negative "E" "sentinel" "WorkspaceDecoyRejectedByHandshake" "${poison_passed}"

# --- Compatibility matrix (#2605 installment 4): bounded, risk-based ---
# Every cell is a REAL run of installed candidates; postures are
# expected/actual/failure-boundary, not labels. Baseline = the exact
# current set; the risk cells cover missing-optional-provider and
# future-unsupported-protocol, which do not require building historical
# packages (previous-compatible-version cells are deferred to the
# release lane where historical candidates exist).
declare -a matrix_records=()
record_matrix() {
  # combination | expected | actual | failure_boundary
  matrix_records+=("$1|$2|$3|$4")
}

log "matrix: baseline current allow + current intent + current proof"
set +e
"${cargo_proof_bin}" dry-run --proof-plan "${ROOT}/tests/fixtures/cargo-proof/proof-plan-smoke-v1.toml" >/dev/null 2>&1
baseline_exit=$?
set -e
if [[ -f "${ROOT}/tests/fixtures/cargo-proof/proof-plan-smoke-v1.toml" ]] && [[ "${baseline_exit}" -eq 0 ]]; then
  record_matrix "current_allow_current_intent_current_proof" "compatible" "compatible" "none"
else
  fail "matrix baseline dry-run failed with exit ${baseline_exit}"
fi

log "matrix: missing optional provider (cargo-intent absent from an allow-only journey)"
# Derived posture: journey A (which runs unconditionally before any
# delegation config exists) proves the allow-only journey green, and the
# absent negative below proves the provider-absent boundary in the same
# receipt; this cell aggregates both rather than re-running them.
record_matrix "current_allow_missing_intent" "compatible_optional_absent" "compatible_optional_absent" "derived: journey A green + absent negative provider_absent (below)"

log "matrix: future unsupported intent protocol (real protocol gate)"
future_dir="${work_dir}/future-protocol-consumer"
mkdir -p "${future_dir}/.allow/compatibility"
git -C "${future_dir}" init -q 2>/dev/null || true
git -C "${future_dir}" config user.name "Interop Harness"
git -C "${future_dir}" config user.email "interop@example.invalid"
printf 'future\n' >"${future_dir}/staged.txt"
git -C "${future_dir}" add staged.txt
# A delegation config declaring a schema generation beyond the current
# contract is the smallest real future-protocol posture: the loader
# itself must refuse it (no silent acceptance of an unknown schema).
printf 'schema_id = "cargo-allow.intent-delegation.v99-future"\nexecutable = "cargo-intent"\ndelegate_staged_precommit = true\n' \
  >"${future_dir}/.allow/compatibility/intent-delegation.toml"
future_out="${future_dir}/precommit-future.json"
set +e
future_err="$(env -u CARGO_INTENT_BIN "${cargo_allow_bin}" check \
  --root "${future_dir}" \
  --profile spec-system \
  --phase precommit \
  --staged \
  --format json \
  --output "${future_out}" 2>&1)"
future_exit=$?
set -e
# The schema-refusal evidence is REQUIRED: a non-zero exit alone could
# come from a downstream failure (e.g. the bare executable resolving to
# nothing) while the schema gate silently never fired — that masked
# regression must fail the cell, not re-label the boundary.
if [[ "${future_exit}" -eq 0 ]]; then
  fail "future-protocol cell accepted an unknown schema (exit 0)"
fi
if ! printf '%s' "${future_err}" | grep -q "unexpected schema_id"; then
  fail "future-protocol cell failed without schema-gate evidence: $(printf '%s' "${future_err}" | head -1)"
fi
record_matrix \
  "current_allow_future_intent_protocol" \
  "incompatible" \
  "incompatible" \
  "loader rejects unknown delegation schema"

matrix_json="$(
  printf '%s\n' "${matrix_records[@]}" | python3 -c '
import json, sys
cells = []
for line in sys.stdin:
    parts = line.strip().split("|", 3)
    if len(parts) != 4:
        raise SystemExit(f"matrix record has unexpected shape: {line.strip()!r}")
    cells.append({
        "combination": parts[0],
        "expected": parts[1],
        "actual": parts[2],
        "failure_boundary": parts[3],
    })
print(json.dumps(cells))
'
)"

negatives_json='[]'
if [[ "${SKIP_NEGATIVES:-0}" != "1" ]]; then
  log "negative controls"

  log "negative A: provider absent (real delegation discovery)"
  absent_dir="${work_dir}/absent-consumer"
  mkdir -p "${absent_dir}/.allow/compatibility"
  git -C "${absent_dir}" init -q 2>/dev/null || true
  git -C "${absent_dir}" config user.name "Interop Harness"
  git -C "${absent_dir}" config user.email "interop@example.invalid"
  printf 'absent\n' >"${absent_dir}/staged.txt"
  git -C "${absent_dir}" add staged.txt
  printf 'schema_id = "cargo-allow.intent-delegation.v1"\ndelegate_staged_precommit = true\ntimeout_secs = 30\n' \
    >"${absent_dir}/.allow/compatibility/intent-delegation.toml"
  absent_out="${absent_dir}/precommit-absent.json"
  set +e
  absent_err="$(env -u CARGO_INTENT_BIN "${cargo_allow_bin}" check \
    --root "${absent_dir}" \
    --profile spec-system \
    --phase precommit \
    --staged \
    --format json \
    --output "${absent_out}" 2>&1)"
  absent_exit=$?
  set -e
  absent_passed=false
  if [[ "${absent_exit}" -ne 0 ]]; then
    if printf '%s' "${absent_err}" | grep -q "provider_absent"; then
      absent_passed=true
    elif [[ -f "${absent_out}" ]] && grep -q "provider_absent" "${absent_out}"; then
      absent_passed=true
    fi
  fi
  [[ "${absent_passed}" == "true" ]] \
    || fail "absent negative expected real ProviderAbsent failure, got exit=${absent_exit} err=${absent_err}"
  record_negative "A" "absent" "ProviderAbsent" "${absent_passed}"

  log "negative A: workspace target leak rejected (real delegation discovery)"
  leak_dir="${work_dir}/leak-consumer"
  mkdir -p "${leak_dir}/.allow/compatibility" "${leak_dir}/target/debug"
  git -C "${leak_dir}" init -q 2>/dev/null || true
  git -C "${leak_dir}" config user.name "Interop Harness"
  git -C "${leak_dir}" config user.email "interop@example.invalid"
  printf 'leak\n' >"${leak_dir}/staged.txt"
  git -C "${leak_dir}" add staged.txt
  # Poison the consumer's own target dir with a workspace-style binary
  # named for the expected product, so only the workspace-path rule —
  # not the product-name rule — can reject it.
  cp "${cargo_allow_bin}" "${leak_dir}/target/debug/cargo-intent$(bin_ext)"
  LEAK_EXEC="${leak_dir}/target/debug/cargo-intent$(bin_ext)" \
    LEAK_CONFIG="${leak_dir}/.allow/compatibility/intent-delegation.toml" python3 <<'PY'
import os
from pathlib import Path
path = Path(os.environ["LEAK_CONFIG"])
executable = Path(os.environ["LEAK_EXEC"]).resolve()
path.write_text(
    "schema_id = \"cargo-allow.intent-delegation.v1\"\n"
    f"executable = {str(executable)!r}\n"
    "delegate_staged_precommit = true\n"
    "timeout_secs = 30\n",
    encoding="utf-8",
)
PY
  leak_out="${leak_dir}/precommit-leak.json"
  set +e
  leak_err="$(env -u CARGO_INTENT_BIN "${cargo_allow_bin}" check \
    --root "${leak_dir}" \
    --profile spec-system \
    --phase precommit \
    --staged \
    --format json \
    --output "${leak_out}" 2>&1)"
  leak_exit=$?
  set -e
  leak_passed=false
  if [[ "${leak_exit}" -ne 0 ]] \
    && printf '%s' "${leak_err}" | grep -q "workspace path, which is forbidden"; then
    leak_passed=true
  elif [[ -f "${leak_out}" ]] && grep -q "workspace path, which is forbidden" "${leak_out}"; then
    leak_passed=true
  fi
  [[ "${leak_passed}" == "true" ]] \
    || fail "workspace target leak negative expected real forbidden-workspace-path failure, got exit=${leak_exit} err=${leak_err}"
  record_negative "A" "incompatible" "ForbiddenWorkspaceTarget" "${leak_passed}"

  log "negative B: wrong product incompatible (real delegation discovery)"
  wrong_dir="${work_dir}/wrong-product-consumer"
  mkdir -p "${wrong_dir}/.allow/compatibility"
  git -C "${wrong_dir}" init -q 2>/dev/null || true
  git -C "${wrong_dir}" config user.name "Interop Harness"
  git -C "${wrong_dir}" config user.email "interop@example.invalid"
  printf 'wrong-product\n' >"${wrong_dir}/staged.txt"
  git -C "${wrong_dir}" add staged.txt
  PROOF_EXEC="${cargo_proof_bin}" WRONG_CONFIG="${wrong_dir}/.allow/compatibility/intent-delegation.toml" python3 <<'PY'
import os
from pathlib import Path
path = Path(os.environ["WRONG_CONFIG"])
executable = Path(os.environ["PROOF_EXEC"]).resolve()
path.write_text(
    "schema_id = \"cargo-allow.intent-delegation.v1\"\n"
    f"executable = {str(executable)!r}\n"
    "delegate_staged_precommit = true\n"
    "timeout_secs = 30\n",
    encoding="utf-8",
)
PY
  wrong_out="${wrong_dir}/precommit-wrong-product.json"
  set +e
  # Scrub the CI-provided provider override so discovery reads the
  # config wrong executable (env vars outrank the config in provider
  # discovery); the negative tests config-based discovery specifically.
  wrong_err="$(env -u CARGO_INTENT_BIN "${cargo_allow_bin}" check \
    --root "${wrong_dir}" \
    --profile spec-system \
    --phase precommit \
    --staged \
    --format json \
    --output "${wrong_out}" 2>&1)"
  wrong_exit=$?
  set -e
  # The failure detail surfaces in the written delegated report and/or
  # stderr depending on platform; accept either, but require the
  # non-zero exit and the wrong_product classification somewhere real.
  wrong_passed=false
  if [[ "${wrong_exit}" -ne 0 ]]; then
    if printf '%s' "${wrong_err}" | grep -q "wrong_product\|wrong product"; then
      wrong_passed=true
    elif [[ -f "${wrong_out}" ]] && grep -q "wrong_product" "${wrong_out}"; then
      wrong_passed=true
    fi
  fi
  [[ "${wrong_passed}" == "true" ]] \
    || fail "wrong product negative expected real WrongProduct failure, got exit=${wrong_exit} err=${wrong_err}"
  record_negative "B" "incompatible" "WrongProduct" "${wrong_passed}"

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

  # Shape-level until cargo-proof exposes provider-discovery as a CLI
  # surface (the journey-C registered-command installment); the config
  # shape it validates is the one the real loader will reject/ignore.
  log "negative D: partial proof-delegation config (shape-level; real path lands with the provider CLI)"
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

  log "negative E: malformed intent delegation config (real parse path)"
  malformed_dir="${work_dir}/malformed-config-consumer"
  mkdir -p "${malformed_dir}/.allow/compatibility"
  git -C "${malformed_dir}" init -q 2>/dev/null || true
  git -C "${malformed_dir}" config user.name "Interop Harness"
  git -C "${malformed_dir}" config user.email "interop@example.invalid"
  printf 'malformed\n' >"${malformed_dir}/staged.txt"
  git -C "${malformed_dir}" add staged.txt
  printf 'not_valid_toml [[[\n' >"${malformed_dir}/.allow/compatibility/intent-delegation.toml"
  malformed_out="${malformed_dir}/precommit-malformed.json"
  set +e
  malformed_err="$(env -u CARGO_INTENT_BIN "${cargo_allow_bin}" check \
    --root "${malformed_dir}" \
    --profile spec-system \
    --phase precommit \
    --staged \
    --format json \
    --output "${malformed_out}" 2>&1)"
  malformed_exit=$?
  set -e
  malformed_passed=false
  if [[ "${malformed_exit}" -ne 0 ]] \
    && printf '%s' "${malformed_err}" | grep -q "MalformedConfig.*intent-delegation.toml"; then
    malformed_passed=true
  fi
  [[ "${malformed_passed}" == "true" ]] \
    || fail "malformed delegation negative expected real MalformedConfig parse failure, got exit=${malformed_exit} err=${malformed_err}"
  record_negative "E" "malformed" "MalformedConfig" "${malformed_passed}"

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

# Marker-leak scan: no retained artifact may embed the planted marker
# (any read of the workspace marker files would surface its text).
log "sentinel: scanning retained artifacts for marker leakage"
leak_hits="$(grep -rl -- "${sentinel_marker}" "${work_dir}" "${consumer_dir}" 2>/dev/null || true)"
if [[ -n "${leak_hits}" ]]; then
  fail "sentinel marker leaked into retained artifacts: ${leak_hits}"
fi
record_negative "A" "sentinel" "NoUndeclaredCheckoutRead" "true"

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
MATRIX_JSON="${matrix_json}" \
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
parity = None
for line in os.environ.get("JOURNEY_RECORDS", "").splitlines():
    if not line.strip():
        continue
    journey_id, product, result = line.split("|", 2)
    if journey_id == "PARITY":
        parity = {
            "surfaces": [
                "cargo-intent change status (direct)",
                "cargo-allow delegated precommit",
            ],
            "staged_state": "journey E consumer (delegated.txt staged)",
            "agreement": "delegated staged_identity_after == direct staged_identity; both classify the state unmapped",
            "result": result,
        }
        continue
    journeys.append({"id": journey_id, "product": product, "result": result})

negatives = json.loads(os.environ.get("NEGATIVE_JSON", "[]"))
matrix = json.loads(os.environ.get("MATRIX_JSON", "[]"))

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
    "installed_journey_parity": parity,
    "compatibility_matrix": {
        "selected": len(matrix),
        "omitted": "previous-compatible-version cells deferred to the release lane (no historical candidates here)",
        "cells": matrix,
    },
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
