#!/usr/bin/env bash
# Full-adapter characterization for #4165. Cargo, git, and gh are stubs;
# jq is real. No compilation, network, or live check publication occurs.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "${ROOT}/target"
work="$(mktemp -d "${ROOT}/target/review-readiness-fixture.XXXXXX")"
trap 'rm -rf "${work}"' EXIT
mkdir -p "${work}/bin" "${work}/repo/.allow/review-dispositions" "${work}/tmp"
export REAL_JQ REAL_MKTEMP
REAL_JQ="$(command -v jq)"
REAL_MKTEMP="$(command -v mktemp)"

# Native jq on Git Bash otherwise emits CRLF into shell variables.
cat >"${work}/bin/jq" <<'SH'
#!/usr/bin/env bash
case "${FIXTURE_JQ_FAILURE:-none}:$1:${2:-}" in
  validate:-se:* | conclusion:-er:.conclusion | summary:-er:.conclusion_reasons* | snapshot:-er:.baseRefName* | live:-n:* | candidate:-r:--arg | selected:-r:.head_sha*)
    printf 'private-jq-canary\n' >&2
    exit 27
    ;;
esac
if [[ "${OSTYPE}" == msys* ]]; then
  exec "${REAL_JQ}" -b "$@"
fi
exec "${REAL_JQ}" "$@"
SH

cat >"${work}/bin/mktemp" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
count=0
if [ -f "${FIXTURE_STATE}/mktemp-count" ]; then
  read -r count <"${FIXTURE_STATE}/mktemp-count"
fi
count=$((count + 1))
printf '%s\n' "${count}" >"${FIXTURE_STATE}/mktemp-count"
if [ "${FIXTURE_MKTEMP_FAILURE}" -eq "${count}" ]; then
  printf 'private-mktemp-canary\n' >&2
  exit 31
fi
exec "${REAL_MKTEMP}" "$@"
SH

cat >"${work}/bin/git" <<'SH'
#!/usr/bin/env bash
case "$1" in
  merge-base)
    if [ "$2" = --is-ancestor ]; then
      exit "${FIXTURE_ANCESTRY_STATUS}"
    fi
    printf '%040d\n' 1
    exit "${FIXTURE_MERGE_BASE_STATUS}"
    ;;
  diff)
    printf 'private-git-canary\n' >&2
    if [ "$2" = --name-only ]; then
      # Even a failed command can emit a plausible partial bootstrap
      # list. The adapter must check its status before consuming it.
      printf '.allow/review-dispositions/fixture.json\n'
      exit "${FIXTURE_DELTA_STATUS}"
    fi
    exit "${FIXTURE_DIFF_STATUS}"
    ;;
  *) exit 90 ;;
esac
SH

cat >"${work}/bin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
touch "${FIXTURE_STATE}/projector-called"
printf '%s\n' "$@" >"${FIXTURE_STATE}/projector-args"
live="" event=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --live) live="$2"; shift ;;
    --event) event="$2"; shift ;;
  esac
  shift
done
printf 'private-projector-canary\n' >&2
case "${FIXTURE_OUTPUT}" in
  empty) exit "${FIXTURE_CARGO_STATUS}" ;;
  invalid) printf '{private-invalid-json-canary\n'; exit "${FIXTURE_CARGO_STATUS}" ;;
esac
jq -n --slurpfile live "${live}" --arg event "${event}" --arg conclusion "${FIXTURE_CONCLUSION}" '
  {schema_id: "cargo-allow.review-readiness-check.v1", schema_version: 1,
   check_context: "review-readiness", repository: $live[0].repository,
   pr_number: $live[0].pr_number, event: $event, conclusion: $conclusion,
   conclusion_reasons: ["fixture typed result"],
   required_posture: (if $conclusion == "success" then "ready" else "draft" end),
   stale_green_invalidated: false, head_ledger_bootstrap: false,
   binding: ($live[0] | del(.review_protocol, .scope_claim_boundary) + {disposition_identity: ""}),
   claim_boundary: "mock transport fixture, no review performed"}' |
  jq "${FIXTURE_FILTER}"
if [ "${FIXTURE_OUTPUT}" = multiple ]; then
  printf '{}\n'
fi
exit "${FIXTURE_CARGO_STATUS}"
SH

cat >"${work}/bin/gh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
case "$1 $2" in
  'pr view')
    if [ "${FIXTURE_PR_STATUS}" -ne 0 ]; then
      printf 'private-snapshot-canary\n' >&2
      exit "${FIXTURE_PR_STATUS}"
    fi
    printf '{"baseRefName":"main","baseRefOid":"%040d","headRefName":"fixture","headRefOid":"%040d","isDraft":false}\n' 1 2
    ;;
  'pr list')
    printf 'private-enumeration-canary\n' >&2
    printf '4165\n'
    exit "${FIXTURE_ENUMERATION_STATUS}"
    ;;
  api\ *)
    printf '%s\n' "$2" >>"${FIXTURE_STATE}/api-calls"
    if [[ "$2" == */commits/* ]]; then
      if [ "${FIXTURE_LOOKUP_STATUS}" -ne 0 ]; then
        printf 'private-lookup-canary\n' >&2
        exit "${FIXTURE_LOOKUP_STATUS}"
      fi
      shift 2
      [ "$1" = --jq ] || exit 91
      # Apply the actual adapter query, so a broken name filter cannot
      # pass the update fixture by receiving an unconditional fake id.
      jq -r "$2" "${FIXTURE_STATE}/check-runs.json"
    else
      printf '%s\n' "$@" >"${FIXTURE_STATE}/mutation-args"
      printf 'private-publication-canary\n' >&2
      exit "${FIXTURE_WRITE_STATUS}"
    fi
    ;;
  *) exit 92 ;;
esac
SH
chmod +x "${work}/bin/"*
export PATH="${work}/bin:${PATH}" TMPDIR="${work}/tmp"
export GH_TOKEN=fixture-token GITHUB_REPOSITORY=EffortlessMetrics/cargo-allow
export PR_NUMBER=4165 PR_EVENT_ACTION=synchronize PR_HEAD_SHA
PR_HEAD_SHA="$(printf '%040d' 2)"
unset CHECK_NAME

fail() {
  printf 'FAIL %s: %s\n' "${case_name}" "$1" >&2
  # This is entirely mocked output, bounded for diagnosing fixture
  # failures; no real credentials or API responses enter this suite.
  head -c 2000 "${FIXTURE_STATE}/log" >&2
  exit 1
}

run_case() {
  local case_name="$1" expected_status="$2" expected_write="$3" expected_message="$4"
  local result=0
  export FIXTURE_STATE="${work}/${case_name}"
  mkdir -p "${FIXTURE_STATE}"
  printf '%s\n' "${check_runs}" >"${FIXTURE_STATE}/check-runs.json"
  if [ "${FIXTURE_LEDGER}" = ancestor ]; then
    printf '{"repository":"EffortlessMetrics/cargo-allow","pr_number":4165,"head_sha":"%040d"}\n' 3 \
      >"${work}/repo/.allow/review-dispositions/fixture.json"
  elif [ "${FIXTURE_LEDGER}" = array ]; then
    printf '[]\n' >"${work}/repo/.allow/review-dispositions/fixture.json"
  else
    rm -f "${work}/repo/.allow/review-dispositions/fixture.json"
  fi
  (
    cd "${work}/repo"
    bash "${ROOT}/scripts/project-review-readiness.sh"
  ) >"${FIXTURE_STATE}/log" 2>&1 || result=$?
  [ "${result}" -eq "${expected_status}" ] || fail "exit ${result}, expected ${expected_status}"
  grep -Fq "${expected_message}" "${FIXTURE_STATE}/log" || fail "missing expected diagnostic"
  if grep -Eq 'private-.*-canary' "${FIXTURE_STATE}/log"; then
    fail "raw tool payload leaked"
  fi
  if [ "${expected_write}" = none ]; then
    [ ! -e "${FIXTURE_STATE}/mutation-args" ] || fail "unexpected API mutation"
  else
    grep -Fxq "${expected_write}" "${FIXTURE_STATE}/mutation-args" || fail "wrong API method"
    grep -Fxq "conclusion=${FIXTURE_CONCLUSION}" "${FIXTURE_STATE}/mutation-args" || fail "wrong conclusion"
    grep -Fq 'output[summary]=pair: base=' "${FIXTURE_STATE}/mutation-args" || fail "missing bound summary"
    grep -Fxq 'fixture typed result' "${FIXTURE_STATE}/mutation-args" || fail "missing reason"
    if [ "${expected_write}" = PATCH ]; then
      grep -Fxq 'repos/EffortlessMetrics/cargo-allow/check-runs/42' "${FIXTURE_STATE}/mutation-args" || fail "wrong update id"
    else
      grep -Fxq 'name=review-readiness' "${FIXTURE_STATE}/mutation-args" || fail "wrong check identity"
    fi
  fi
  if [ "${expected_write}" = none ] || [ "${FIXTURE_WRITE_STATUS}" -ne 0 ]; then
    if grep -Eq 'review-readiness: (published|updated)' "${FIXTURE_STATE}/log"; then
      fail "failure printed a success message"
    fi
  fi
  if [[ "${expected_message}" == *'unusable projection'* ]]; then
    [ ! -e "${FIXTURE_STATE}/api-calls" ] || fail "invalid projection reached the API"
  fi
  if [[ "${case_name}" == input_* ]]; then
    [ ! -e "${FIXTURE_STATE}/projector-called" ] || fail "invalid input reached the projector"
    [ ! -e "${FIXTURE_STATE}/api-calls" ] || fail "invalid input reached the API"
  fi
  if [ "${case_name}" = ancestor_success ]; then
    grep -Fxq -- '--head-delta-path' "${FIXTURE_STATE}/projector-args" || fail "missing bootstrap delta argument"
    grep -Fxq '.allow/review-dispositions/fixture.json' "${FIXTURE_STATE}/projector-args" || fail "missing complete bootstrap path"
  fi
  [ -z "$(find "${work}/tmp" -type f -print -quit)" ] || fail "temporary projection leaked"
  printf 'ok %s\n' "${case_name}"
}

export FIXTURE_OUTPUT=json FIXTURE_CARGO_STATUS=0 FIXTURE_CONCLUSION=success FIXTURE_FILTER=.
export FIXTURE_LOOKUP_STATUS=0 FIXTURE_WRITE_STATUS=0 GITHUB_EVENT_NAME=pull_request
export FIXTURE_PR_STATUS=0 FIXTURE_MERGE_BASE_STATUS=0 FIXTURE_DIFF_STATUS=0 FIXTURE_MKTEMP_FAILURE=0
export FIXTURE_LEDGER=missing FIXTURE_ANCESTRY_STATUS=0 FIXTURE_DELTA_STATUS=0 FIXTURE_ENUMERATION_STATUS=0
check_runs='{"check_runs":[]}'
run_case create_success 0 POST 'published success'
FIXTURE_CONCLUSION=neutral
run_case create_neutral 0 POST 'published neutral'
FIXTURE_CONCLUSION=failure FIXTURE_CARGO_STATUS=1
run_case typed_failure 1 POST 'published failure'
FIXTURE_CONCLUSION=success FIXTURE_CARGO_STATUS=0
# An unrelated check must never receive the readiness update.
check_runs='{"check_runs":[{"name":"unrelated","id":17},{"name":"review-readiness","id":42}]}'
run_case update_success 0 PATCH 'updated run 42 -> success'
FIXTURE_WRITE_STATUS=22
run_case failed_patch 1 PATCH 'PATCH failed (exit 22)'
check_runs='{"check_runs":[]}'
run_case failed_post 1 POST 'POST failed (exit 22)'
FIXTURE_WRITE_STATUS=0 FIXTURE_LOOKUP_STATUS=23
run_case failed_lookup 1 none 'lookup failed (exit 23)'
FIXTURE_LOOKUP_STATUS=0
check_runs='{private-lookup-json-canary'
run_case malformed_lookup_json 1 none 'lookup failed'
check_runs='{"check_runs":[{"name":"review-readiness","id":"invalid"}]}'
run_case invalid_lookup_id 1 none 'lookup returned an invalid id'
check_runs='{"check_runs":[]}'
export FIXTURE_JQ_FAILURE=validate
run_case failed_jq_validation 1 none 'unusable projection'
FIXTURE_JQ_FAILURE=conclusion
run_case failed_jq_conclusion 1 none 'could not read validated projection'
FIXTURE_JQ_FAILURE=summary
run_case failed_jq_summary 1 none 'could not read validated projection'
FIXTURE_JQ_FAILURE=none
FIXTURE_PR_STATUS=28
run_case input_failed_pr_read 1 none 'PR snapshot read failed (exit 28)'
FIXTURE_PR_STATUS=0 FIXTURE_JQ_FAILURE=snapshot
run_case input_failed_snapshot_parse 1 none 'invalid PR snapshot'
FIXTURE_JQ_FAILURE=none FIXTURE_MERGE_BASE_STATUS=29
run_case input_failed_merge_base 1 none 'merge-base read failed (exit 29)'
FIXTURE_MERGE_BASE_STATUS=0 FIXTURE_DIFF_STATUS=29
run_case input_failed_diff 1 none 'diff digest read failed (exit 29)'
FIXTURE_DIFF_STATUS=0 FIXTURE_JQ_FAILURE=live
run_case input_failed_live_json 1 none 'could not create live input'
FIXTURE_JQ_FAILURE=none FIXTURE_MKTEMP_FAILURE=1
run_case input_failed_live_temp 1 none 'could not allocate live input'
FIXTURE_MKTEMP_FAILURE=2
run_case input_failed_projection_temp 1 none 'could not allocate projection output'
FIXTURE_MKTEMP_FAILURE=0
FIXTURE_LEDGER=ancestor
run_case ancestor_success 0 POST 'published success'
FIXTURE_ANCESTRY_STATUS=1 FIXTURE_CONCLUSION=neutral
run_case nonancestor_is_missing 0 POST 'published neutral'
FIXTURE_ANCESTRY_STATUS=2 FIXTURE_CONCLUSION=success
run_case input_failed_ancestry 1 none 'ancestry read failed (exit 2)'
FIXTURE_ANCESTRY_STATUS=0 FIXTURE_JQ_FAILURE=candidate
run_case input_failed_candidate_binding 1 none 'could not read candidate disposition binding'
FIXTURE_JQ_FAILURE=selected
run_case input_failed_selected_binding 1 none 'could not read selected disposition binding'
FIXTURE_JQ_FAILURE=none FIXTURE_DELTA_STATUS=29
run_case input_failed_partial_bootstrap_delta 1 none 'disposition delta read failed (exit 29)'
FIXTURE_DELTA_STATUS=0 FIXTURE_LEDGER=missing
FIXTURE_LEDGER=array
run_case input_invalid_candidate_array 1 none 'could not read candidate disposition binding'
FIXTURE_LEDGER=missing

FIXTURE_OUTPUT=empty FIXTURE_CARGO_STATUS=101
run_case failed_cargo_empty 1 none 'unusable projection (projector exit 101)'
FIXTURE_CARGO_STATUS=0
run_case empty_output 1 none 'unusable projection (projector exit 0)'
FIXTURE_OUTPUT=invalid
run_case invalid_json 1 none 'unusable projection'
FIXTURE_OUTPUT=multiple
run_case multiple_documents 1 none 'unusable projection'
FIXTURE_OUTPUT=json FIXTURE_CARGO_STATUS=1
run_case failed_cargo_success_json 1 none 'unusable projection (projector exit 1)'
FIXTURE_CARGO_STATUS=101 FIXTURE_CONCLUSION=failure
run_case crashed_cargo_failure_json 1 none 'unusable projection (projector exit 101)'
FIXTURE_CARGO_STATUS=0
run_case zero_exit_failure_json 1 none 'unusable projection'
FIXTURE_CONCLUSION=success

for mutation in \
  '.conclusion = ""' '.conclusion = "cancelled"' 'del(.conclusion)' \
  '.conclusion_reasons = []' '.conclusion_reasons = [null]' \
  '.conclusion_reasons = [" "]' 'del(.conclusion_reasons)' \
  '.schema_id = "unknown"' '.schema_version = 2' \
  '.check_context = "other"' 'del(.binding)' '.binding.head_sha = "stale"' \
  'del(.binding.disposition_identity)' '.event = "opened"' '.repository = "other"' \
  '.pr_number = 1' 'del(.required_posture)' 'del(.stale_green_invalidated)' \
  'del(.head_ledger_bootstrap)' 'del(.claim_boundary)' '[]' 'null'; do
  FIXTURE_FILTER="${mutation}"
  run_case "invalid_envelope_${mutation//[^a-zA-Z0-9]/_}" 1 none 'unusable projection'
done
FIXTURE_FILTER=.
GITHUB_EVENT_NAME=push FIXTURE_WRITE_STATUS=22
run_case push_publication_failure 1 POST 'POST failed (exit 22)'
FIXTURE_WRITE_STATUS=0
run_case push_success 0 POST 'published success'
FIXTURE_ENUMERATION_STATUS=26
run_case input_failed_push_enumeration 1 none 'open PR enumeration failed (exit 26)'

printf 'all review-readiness adapter characterization checks passed\n'
